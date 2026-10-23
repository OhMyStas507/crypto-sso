//! 角色共享配置与跨进程协议一致性；每个子进程在首次 SDK 使用前初始化。
mod common;
use common::*;
use crypto_sso_core::__private as math;
use crypto_sso_holder::{Holder, PendingEnrollment};
use crypto_sso_issuer::{EnrollmentRequest, Issuer};
use crypto_sso_verifier::*;
use std::process::Command;

fn child(mode: &str) {
    let limits = if mode == "small" {
        ClaimsLimits {
            max_keys: 4,
            max_key_bytes: 8,
            max_value_bytes: 16,
        }
    } else {
        ClaimsLimits {
            max_keys: 256,
            max_key_bytes: 128,
            max_value_bytes: 2048,
        }
    };
    // SAFETY：独立进程入口，没有先前 SDK 调用或其他工作线程。
    unsafe {
        set_claims_limits(limits).unwrap();
    }
    assert_eq!(crypto_sso_issuer::claims_limits(), limits);
    assert_eq!(crypto_sso_holder::claims_limits(), limits);
    assert_eq!(claims_limits(), limits);
    let mut rng = rng(55);
    let issuer = Issuer::generate("config-org", &mut rng).unwrap();
    let values = Claims::new([("a", "one"), ("b", "two"), ("c", "three")]).unwrap();
    let (pending, request) = Holder::begin_enrollment(issuer.public(), &values, &mut rng).unwrap();
    let blind = issuer.issue_blind(&request, &values, &mut rng).unwrap();
    let holder = pending.finish(&blind).unwrap();
    let app = Verifier::new(issuer.public().clone(), origin()).unwrap();
    let (c, b) = app
        .create_challenge(&ClaimKeys::new(["a"]).unwrap(), NOW, &mut rng)
        .unwrap();
    let p = holder
        .present(
            &c,
            &origin(),
            &ClaimKeys::new(["a", "c"]).unwrap(),
            NOW,
            &mut rng,
        )
        .unwrap();
    let checked = app.verify_proof(&p, &c, &b, NOW).unwrap();
    assert_roundtrip(&request);
    assert_roundtrip(&blind);
    assert_roundtrip(&c);
    assert_roundtrip(&p);
    let pending_bytes = pending.export_secret().unwrap();
    assert!(
        PendingEnrollment::import_secret(pending_bytes.expose_secret(), issuer.public())
            .unwrap()
            .finish(&blind)
            .is_ok()
    );
    let holder_bytes = holder.export_secret().unwrap();
    assert_eq!(
        Holder::import_secret(holder_bytes.expose_secret(), issuer.public())
            .unwrap()
            .claims(),
        &values
    );
    assert!(
        Issuer::import_secret(
            issuer.export_secret().unwrap().expose_secret(),
            issuer.public()
        )
        .is_ok()
    );
    if mode == "small" {
        for variant in 0..3 {
            let oversized = match variant {
                0 => serde_json::json!({"a":"1","b":"2","c":"3","d":"4","e":"5"}),
                1 => serde_json::json!({"abcdefghi":"v"}),
                _ => serde_json::json!({"a":"0123456789abcdefg"}),
            };
            assert!(
                wire::from_json::<EnrollmentRequest>(&change_json(
                    &wire::to_json(&request).unwrap(),
                    |v| v["claims"] = oversized.clone()
                ))
                .is_err()
            );
            assert!(
                Holder::import_secret(
                    &change_json(holder_bytes.expose_secret(), |v| v["claims"] =
                        oversized.clone()),
                    issuer.public()
                )
                .is_err()
            );
            assert!(
                PendingEnrollment::import_secret(
                    &change_json(pending_bytes.expose_secret(), |v| v["request"]["claims"] =
                        oversized.clone()),
                    issuer.public()
                )
                .is_err()
            );
            let mut invalid = p.clone();
            match variant {
                0 => invalid.claim_count = 5,
                1 => invalid.disclosed_claims[0].key = "abcdefghi".into(),
                _ => invalid.disclosed_claims[0].value = "0123456789abcdefg".into(),
            };
            assert!(wire::to_json(&invalid).is_err());
            assert!(app.verify_proof(&invalid, &c, &b, NOW).is_err());
        }
        assert!(ClaimKeys::new(["a", "b", "c", "d", "e"]).is_err());
        // 小配置的键和值边界同样经过角色入口、秘密输出和恢复。
        for (key, value) in [("a", ""), ("abcdefgh", "中文中文abcd")] {
            let exact = Claims::new([(key, value)]).unwrap();
            let (pending, request) =
                Holder::begin_enrollment(issuer.public(), &exact, &mut rng).unwrap();
            let blind = issuer.issue_blind(&request, &exact, &mut rng).unwrap();
            let h = pending.finish(&blind).unwrap();
            assert!(
                Holder::import_secret(h.export_secret().unwrap().expose_secret(), issuer.public())
                    .is_ok()
            );
            assert!(
                PendingEnrollment::import_secret(
                    pending.export_secret().unwrap().expose_secret(),
                    issuer.public()
                )
                .is_ok()
            );
        }
    }
    let params = math::parameters(6).unwrap();
    // 同时比较规范消息、请求哈希、身份和固定基点；配置绝不进入协议。
    println!(
        "{}",
        serde_json::json!({"request":String::from_utf8(wire::to_json(&request).unwrap()).unwrap(),"hash":request.request_hash().unwrap().as_str(),"issuer":issuer.issuer_id().as_str(),"subject":checked.subject().as_str(),"presentation":String::from_utf8(wire::to_json(&p).unwrap()).unwrap(),"parameters":math::encode(&params).unwrap()})
    );
}
fn main() {
    if let Some(mode) = std::env::args().nth(1) {
        child(&mode);
        return;
    }
    let results: Vec<_> = ["small", "large"]
        .into_iter()
        .map(|mode| {
            let output = Command::new(std::env::current_exe().unwrap())
                .arg(mode)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        })
        .collect();
    assert_eq!(results[0], results[1], "CF-06：配置不得改变协议编码或身份");
    println!("CF-02、CF-06、CO-08、BK-05、PV-03：独立进程配置检查通过");
}
