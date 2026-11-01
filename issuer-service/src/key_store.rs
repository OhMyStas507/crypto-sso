//! PostgreSQL 中保存公开身份与未经额外加密的 SDK 私钥备份，仅服务端读取。
use crate::{
    error::{Error, Result},
    models::InitializationKey,
};
use crypto_sso_issuer::{Issuer, IssuerPublic, wire};
use rand::rngs::OsRng;
use zeroize::Zeroizing;

fn unavailable() -> Error {
    Error::new(
        503,
        "KEY_STORE_UNAVAILABLE",
        "密钥存储不可用，请联系部署维护者后重试原操作",
    )
}

pub fn load(row: &InitializationKey) -> Result<Issuer> {
    let public = wire::from_json::<IssuerPublic>(
        row.public_json.as_ref().ok_or_else(unavailable)?.as_bytes(),
    )
    .map_err(|_| unavailable())?;
    let secret = Zeroizing::new(
        hex::decode(row.secret_hex.as_ref().ok_or_else(unavailable)?).map_err(|_| unavailable())?,
    );
    let issuer = Issuer::import_secret(&secret, &public).map_err(|_| unavailable())?;
    if issuer.public().organization() != row.organization {
        return Err(unavailable());
    }
    Ok(issuer)
}

/// 公私钥与 READY 状态在同一事务中保存。
pub fn generate(organization: &str) -> Result<(String, Zeroizing<String>)> {
    let issuer = Issuer::generate(organization, &mut OsRng).map_err(|_| {
        Error::new(
            422,
            "VALIDATION_FAILED",
            "组织身份必须为 1–128 字节且不含首尾空白或控制字符",
        )
    })?;
    let public = String::from_utf8(wire::to_json(issuer.public()).map_err(|_| unavailable())?)
        .map_err(|_| unavailable())?;
    let secret = issuer.export_secret().map_err(|_| unavailable())?;
    Issuer::import_secret(secret.expose_secret(), issuer.public()).map_err(|_| unavailable())?;
    Ok((public, Zeroizing::new(hex::encode(secret.expose_secret()))))
}
