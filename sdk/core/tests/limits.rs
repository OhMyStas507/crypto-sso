//! 每种全局配置启动独立进程；不修改测试线程数，也不在运行阶段热更新。
use crypto_sso_core::__private as math;
use crypto_sso_core::*;
use std::process::Command;

fn request(claims: Claims) -> EnrollmentRequest {
    let point = math::encode(&math::parameters(0).unwrap().g1).unwrap();
    let scalar = math::encode(&math::Fr::from(1u64)).unwrap();
    EnrollmentRequest {
        version: 1,
        issuer_id: IssuerId::parse(&"00".repeat(32)).unwrap(),
        request_id: RequestId::parse(&"01".repeat(32)).unwrap(),
        commitment: point.clone(),
        proof_commitment: point,
        response_secret: scalar.clone(),
        response_blinding: scalar,
        claims,
    }
}
fn roundtrip(claims: Claims) {
    let r = request(claims);
    let bytes = wire::to_json(&r).unwrap();
    assert_eq!(wire::from_json::<EnrollmentRequest>(&bytes).unwrap(), r);
}
fn run(scenario: &str) {
    let limits = match scenario {
        "small" => ClaimsLimits {
            max_keys: 4,
            max_key_bytes: 8,
            max_value_bytes: 16,
        },
        "large" => ClaimsLimits {
            max_keys: 256,
            max_key_bytes: 128,
            max_value_bytes: 2048,
        },
        "zero_keys" => ClaimsLimits {
            max_keys: 0,
            ..DEFAULT_CLAIMS_LIMITS
        },
        "zero_values" => ClaimsLimits {
            max_value_bytes: 0,
            ..DEFAULT_CLAIMS_LIMITS
        },
        "max" => ClaimsLimits {
            max_keys: usize::MAX,
            ..DEFAULT_CLAIMS_LIMITS
        },
        "invalid" => ClaimsLimits {
            max_keys: 4,
            max_key_bytes: 8,
            max_value_bytes: 16,
        },
        _ => panic!("未知配置场景"),
    };
    // SAFETY：子进程尚未执行任何其他 SDK 调用，也没有启动工作线程。
    unsafe {
        set_claims_limits(limits).unwrap();
    }
    if scenario == "invalid" {
        // SAFETY：仍处于初始化阶段，尚无 SDK 读取；失败须保留全部旧字段。
        assert!(matches!(
            unsafe {
                set_claims_limits(ClaimsLimits {
                    max_keys: 9,
                    max_key_bytes: 0,
                    max_value_bytes: 99,
                })
            },
            Err(Error::InvalidClaimsLimits)
        ));
    }
    assert_eq!(claims_limits(), limits);
    match scenario {
        "max" | "invalid" => {}
        "zero_keys" => {
            roundtrip(Claims::empty());
            assert!(Claims::new([("a", "")]).is_err());
            assert!(ClaimKeys::new(["a"]).is_err());
        }
        "zero_values" => {
            roundtrip(Claims::new([("a", "")]).unwrap());
            assert!(Claims::new([("a", "x")]).is_err());
        }
        "large" => {
            for claims in [
                Claims::new((0..33).map(|i| (format!("k{i:02}"), "v"))).unwrap(),
                Claims::new([("a".repeat(65), "v")]).unwrap(),
                Claims::new([("a", "v".repeat(1025))]).unwrap(),
            ] {
                roundtrip(claims);
            }
        }
        "small" => {
            for n in [0, 1, 4] {
                assert_eq!(math::count(n, limits).unwrap(), usize::try_from(n).unwrap());
            }
            assert!(math::count(5, limits).is_err());
            for n in [0, 1, 4] {
                roundtrip(Claims::new((0..n).map(|i| (format!("k{i}"), "v"))).unwrap());
                assert_eq!(
                    ClaimKeys::new((0..n).map(|i| format!("k{i}")))
                        .unwrap()
                        .len(),
                    n
                );
            }
            assert!(Claims::new((0..5).map(|i| (format!("k{i}"), "v"))).is_err());
            assert!(ClaimKeys::new((0..5).map(|i| format!("k{i}"))).is_err());
            for key in ["a", "abcdefgh"] {
                for value in [
                    "",
                    "0123456789abcdef",
                    "中文中文abcd",
                    "\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n",
                ] {
                    roundtrip(Claims::new([(key, value)]).unwrap());
                }
            }
            for (key, value) in [
                ("", ""),
                ("abcdefghi", ""),
                ("a", "0123456789abcdefg"),
                ("a", "中文中文abcde"),
            ] {
                assert!(Claims::new([(key, value)]).is_err());
            }
            let bytes = wire::to_json(&request(Claims::new([("a", "v")]).unwrap())).unwrap();
            for claims in [
                serde_json::json!({"a":"v","b":"v","c":"v","d":"v","e":"v"}),
                serde_json::json!({"abcdefghi":"v"}),
                serde_json::json!({"a":"0123456789abcdefg"}),
            ] {
                let mut raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                raw["claims"] = claims;
                assert!(matches!(
                    wire::from_json::<EnrollmentRequest>(&serde_json::to_vec(&raw).unwrap()),
                    Err(Error::InvalidClaims)
                ));
            }
            let mut raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            raw["claims"] = serde_json::json!({"a":"\n"});
            assert!(
                wire::from_json::<EnrollmentRequest>(&serde_json::to_vec(&raw).unwrap()).is_ok()
            );
        }
        _ => unreachable!(),
    }
}
fn main() {
    if let Some(scenario) = std::env::args().nth(1) {
        run(&scenario);
        return;
    }
    for scenario in [
        "small",
        "large",
        "invalid",
        "zero_keys",
        "zero_values",
        "max",
    ] {
        let status = Command::new(std::env::current_exe().unwrap())
            .arg(scenario)
            .status()
            .unwrap();
        assert!(status.success(), "配置场景失败：{scenario}");
        println!("CF-02/03/04/05、CO-08：{scenario} 通过");
    }
}
