//! 手动生成合成 v1 向量；只允许创建新文件，绝不在测试失败时重写期望值。
#[path = "../tests/common/mod.rs"]
mod common;
use common::*;
use crypto_sso_core::__private as math;
use crypto_sso_verifier::*;
use serde_json::json;
use std::io::Write;

fn main() {
    let target = std::env::args()
        .nth(1)
        .expect("请指定一个尚不存在的输出 JSON 文件");
    let mut f = Fixture::standard();
    let mut presentations = Vec::new();
    for keys in [
        ClaimKeys::empty(),
        ClaimKeys::new(["role"]).unwrap(),
        claims().keys(),
    ] {
        let p = f
            .holder
            .present(&f.challenge, &origin(), &keys, NOW, &mut f.rng)
            .unwrap();
        presentations.push(String::from_utf8(wire::to_json(&p).unwrap()).unwrap());
    }
    let mut invalid = f.presentation.clone();
    invalid.disclosed_claims[0].value = "tampered".into();
    let params = math::parameters(6).unwrap();
    let scalar_claims: serde_json::Map<String, serde_json::Value> = claims()
        .iter()
        .map(|(k, v)| {
            (
                k.to_owned(),
                json!(math::encode(&math::claim_message(k, v).unwrap()).unwrap()),
            )
        })
        .collect();
    let output = json!({
        "metadata":{"protocol_version":1,"sdk_version":"0.1.0","source":"合成测试数据；org-anon-auth 仅为数学流程参考，非互通向量","reference_commit":"ebf343609d1ba05c9e5a6043e1ba95ca4efe0188","rng":"rand_chacha 0.3 ChaCha20Rng","seed_hex":"0b".repeat(32),"now":NOW,"generator":"cargo run -p crypto-sso-verifier --example export_v1 -- NEW_FILE.json","valid_result":"成功","invalid_result":"InvalidPresentation"},
        "public":String::from_utf8(wire::to_json(f.issuer.public()).unwrap()).unwrap(),
        "request":String::from_utf8(wire::to_json(&f.request).unwrap()).unwrap(),
        "request_hash":f.request.request_hash().unwrap().as_str(),
        "blind":String::from_utf8(wire::to_json(&f.blind).unwrap()).unwrap(),
        "challenge":String::from_utf8(wire::to_json(&f.challenge).unwrap()).unwrap(),
        "presentation":String::from_utf8(wire::to_json(&f.presentation).unwrap()).unwrap(),
        "presentations":presentations,
        "invalid_presentation":String::from_utf8(wire::to_json(&invalid).unwrap()).unwrap(),
        "issuer_backup":std::str::from_utf8(f.issuer.export_secret().unwrap().expose_secret()).unwrap(),
        "pending_backup":std::str::from_utf8(f.pending.export_secret().unwrap().expose_secret()).unwrap(),
        "holder_backup":std::str::from_utf8(f.holder.export_secret().unwrap().expose_secret()).unwrap(),
        "cookie":f.binding.expose_cookie_value(),"subject":f.subject().as_str(),"issuer_id":f.issuer.issuer_id().as_str(),
        "parameters":{"rule":std::str::from_utf8(math::PARAMETER_RULE).unwrap(),"g1":math::encode(&params.g1).unwrap(),"g2":math::encode(&params.g2).unwrap(),"h0":math::encode(&params.h_0).unwrap(),"messages":params.h.iter().map(|p|math::encode(p).unwrap()).collect::<Vec<_>>()},
        "organization_message":math::encode(&math::organization_message(f.issuer.issuer_id()).unwrap()).unwrap(),
        "count_message":math::encode(&math::Fr::from(3u64)).unwrap(),"claim_messages":scalar_claims,
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .unwrap();
    file.write_all(serde_json::to_string_pretty(&output).unwrap().as_bytes())
        .unwrap();
    file.write_all(b"\n").unwrap();
}
