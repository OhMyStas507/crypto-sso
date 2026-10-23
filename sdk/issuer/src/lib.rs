//! 签发方：核对完整核准属性，验证申领证明后签发 BBS+ 盲凭证。
//!
//! 身份核验、属性授权、唯一性台账及事务持久化由签发服务负责。
//! 参考 org-anon-auth（Apache-2.0）；本实现增加属性签名及分角色接口。
#![warn(missing_docs)]
mod error;
use crypto_sso_core::{__private::*, PROTOCOL_VERSION};
pub use crypto_sso_core::{
    BlindCredential, Claims, ClaimsLimits, DEFAULT_CLAIMS_LIMITS, EnrollmentRequest,
    Error as CoreError, IssuerId, IssuerPublic, RequestHash, RequestId, Result as CoreResult,
    SecretBytes, claims_limits, set_claims_limits, wire,
};
pub use error::{Error, Result};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// 持有非零签发私钥的运行时对象，不提供克隆、日志或通用序列化。
pub struct Issuer {
    public: IssuerPublic,
    id: IssuerId,
    key: SecretKey<Fr>,
}

#[cfg(test)]
mod tests;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    version: u8,
    public: Json<IssuerPublic>,
    secret_key: Zeroizing<String>,
}
impl Issuer {
    /// 初始化组织及非零私钥；组织名最多 128 UTF-8 字节且无首尾空白或控制字符。
    pub fn generate<R: RngCore + CryptoRng>(organization_name: &str, rng: &mut R) -> Result<Self> {
        organization(organization_name)?;
        let key = SecretKey(nonzero(rng));
        let public = IssuerPublic {
            version: PROTOCOL_VERSION,
            organization: organization_name.to_owned(),
            public_key: encode(&PublicKey::generate_using_secret_key(&key, &parameters(0)?))?,
        };
        let id = public.issuer_id()?;
        Ok(Self { public, id, key })
    }
    /// 可通过可信渠道分发的组织公开信息。
    pub fn public(&self) -> &IssuerPublic {
        &self.public
    }
    /// 初始化时固定的可信组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.id
    }
    /// 导出未加密本地备份；调用方负责加密和安全持久化。
    pub fn export_secret(&self) -> Result<SecretBytes> {
        let backup = Backup {
            version: PROTOCOL_VERSION,
            public: Json(self.public.clone()),
            secret_key: Zeroizing::new(encode(&self.key)?),
        };
        Ok(SecretBytes::from_vec(
            serde_json::to_vec(&backup).map_err(json_error)?,
        ))
    }
    /// 恢复私钥，并验证备份版本、公私钥关系及预期完整信任源。
    pub fn import_secret(bytes: &[u8], expected_issuer: &IssuerPublic) -> Result<Self> {
        let id = expected_issuer.issuer_id()?;
        let backup: Backup = serde_json::from_slice(bytes).map_err(json_error)?;
        version(backup.version)?;
        if backup.public.0 != *expected_issuer {
            return Err(Error::IssuerMismatch);
        }
        let key = SecretKey(scalar(&backup.secret_key)?);
        if key.0.is_zero()
            || encode(&PublicKey::generate_using_secret_key(&key, &parameters(0)?))?
                != expected_issuer.public_key
        {
            return Err(CoreError::InvalidIssuer.into());
        }
        Ok(Self {
            public: expected_issuer.clone(),
            id,
            key,
        })
    }
    /// 验证申领证明并盲签完整核准集合；核准属性必须来自服务方的可信资料。
    ///
    /// 此方法不执行身份授权或幂等控制；相同请求可产生不同有效签名。
    pub fn issue_blind<R: RngCore + CryptoRng>(
        &self,
        request: &EnrollmentRequest,
        approved_claims: &Claims,
        rng: &mut R,
    ) -> Result<BlindCredential> {
        let limits = claims_limits();
        version(request.version)?;
        if request.issuer_id != self.id {
            return Err(Error::IssuerMismatch);
        }
        validate(request, limits)?;
        validate_claims(approved_claims, limits)?;
        let params = parameters(message_count(request.claims.len())?)?;
        if !verify_opening(request, &params)? {
            return Err(Error::InvalidEnrollmentProof);
        }
        if request.claims != *approved_claims {
            return Err(Error::ClaimsMismatch);
        }
        let values = messages(Fr::zero(), &self.id, approved_claims)?;
        let known = values.iter().enumerate().skip(1).collect();
        let signature = Signature::new_with_committed_messages(
            rng,
            &point(&request.commitment)?,
            known,
            &self.key,
            &params,
        )
        .map_err(|_| CoreError::Crypto)?;
        Ok(BlindCredential {
            version: PROTOCOL_VERSION,
            issuer_id: self.id.clone(),
            request_id: request.request_id.clone(),
            request_hash: request.request_hash()?,
            signature: encode(&signature)?,
            claims: approved_claims.clone(),
        })
    }
}
