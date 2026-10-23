//! 钱包持有方：盲申领、解盲验签、显式选择披露及本地备份恢复。
//!
//! 应用 origin 和披露选择必须来自可信交互；本库不会自动扩大披露范围。
//! 参考 org-anon-auth（Apache-2.0）；本实现增加属性、多隐藏消息及严格备份校验。
#![warn(missing_docs)]
mod error;
#[cfg(test)]
mod tests;
use crypto_sso_core::{__private::*, PROTOCOL_VERSION};
pub use crypto_sso_core::{
    BlindCredential, ClaimKeys, Claims, ClaimsLimits, DEFAULT_CLAIMS_LIMITS, DisclosedClaim,
    EnrollmentRequest, Error as CoreError, IssuerId, IssuerPublic, LoginChallenge, Nonce, Origin,
    Presentation, RequestHash, RequestId, Result as CoreResult, SecretBytes, claims_limits,
    set_claims_limits, wire,
};
use dock_crypto_utils::signature::MessageOrBlinding;
pub use error::{Error, Result};
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// 一张已验签凭证及其完整属性、非零钱包秘密；不提供秘密读取接口。
pub struct Holder {
    issuer: IssuerPublic,
    id: IssuerId,
    key: PublicKey,
    secret: Zeroizing<Fr>,
    claims: Claims,
    signature: Signature,
}
/// 尚未完成的申领状态，必须先在钱包本地安全保存再发送请求。
///
/// `finish` 借用该对象；错误不会销毁重试所需的秘密和盲因子。
pub struct PendingEnrollment {
    issuer: IssuerPublic,
    id: IssuerId,
    request: EnrollmentRequest,
    hash: RequestHash,
    secret: Zeroizing<Fr>,
    blinding: Zeroizing<Fr>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingBackup {
    version: u8,
    issuer: Json<IssuerPublic>,
    request: Json<EnrollmentRequest>,
    request_hash: RequestHash,
    secret: Zeroizing<String>,
    blinding: Zeroizing<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HolderBackup {
    version: u8,
    issuer: Json<IssuerPublic>,
    secret: Zeroizing<String>,
    credential: Zeroizing<String>,
    claims: Claims,
}
fn time(c: &LoginChallenge, now: u64) -> Result<()> {
    check_time(c, now).map_err(|e| match e {
        TimeError::Invalid => Error::InvalidChallengeTime,
        TimeError::Expired => Error::ChallengeExpired,
    })
}

impl Holder {
    /// 生成新的秘密、盲因子和申领标识；重新申领会改变应用身份。
    pub fn begin_enrollment<R: RngCore + CryptoRng>(
        trusted_issuer: &IssuerPublic,
        claims: &Claims,
        rng: &mut R,
    ) -> Result<(PendingEnrollment, EnrollmentRequest)> {
        let limits = claims_limits();
        validate_claims(claims, limits)?;
        let id = trusted_issuer.issuer_id()?;
        let params = parameters(message_count(claims.len())?)?;
        let x = Zeroizing::new(nonzero(rng));
        let r = Zeroizing::new(nonzero(rng));
        let kx = Zeroizing::new(nonzero(rng));
        let kr = Zeroizing::new(nonzero(rng));
        let commitment = params
            .commit_to_messages([(0, &*x)], &r)
            .map_err(|_| CoreError::Crypto)?;
        let t = (params.h[0].mul_bigint(kx.into_bigint())
            + params.h_0.mul_bigint(kr.into_bigint()))
        .into_affine();
        let mut request = EnrollmentRequest {
            version: PROTOCOL_VERSION,
            issuer_id: id.clone(),
            request_id: RequestId::parse(&random_token(rng))?,
            commitment: encode(&commitment)?,
            proof_commitment: encode(&t)?,
            response_secret: String::new(),
            response_blinding: String::new(),
            claims: claims.clone(),
        };
        let c = enrollment_challenge(&request, &params)?;
        request.response_secret = encode(&(*kx + c * *x))?;
        request.response_blinding = encode(&(*kr + c * *r))?;
        let hash = request.request_hash()?;
        Ok((
            PendingEnrollment {
                issuer: trusted_issuer.clone(),
                id,
                request: request.clone(),
                hash,
                secret: x,
                blinding: r,
            },
            request,
        ))
    }
    /// 初始化或恢复时固定的可信组织信息。
    pub fn issuer(&self) -> &IssuerPublic {
        &self.issuer
    }
    /// 已验证并缓存的组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.id
    }
    /// 完整且已由本地凭证签名验证的属性。
    pub fn claims(&self) -> &Claims {
        &self.claims
    }
    /// 根据用户显式选择生成随机化组合证明，不自动补充必需属性。
    ///
    /// `trusted_origin` 必须来自可信浏览器或钱包交互；`now` 由设备时钟提供。
    pub fn present<R: RngCore + CryptoRng>(
        &self,
        challenge: &LoginChallenge,
        trusted_origin: &Origin,
        disclose_keys: &ClaimKeys,
        now: u64,
        rng: &mut R,
    ) -> Result<Presentation> {
        let limits = claims_limits();
        version(challenge.version)?;
        if challenge.issuer_id != self.id {
            return Err(Error::IssuerMismatch);
        }
        if challenge.audience != *trusted_origin {
            return Err(Error::OriginMismatch);
        }
        time(challenge, now)?;
        validate(challenge, limits)?;
        validate_keys(disclose_keys, limits)?;
        validate_claims(&self.claims, limits)?;
        if disclose_keys.iter().any(|k| self.claims.get(k).is_none()) {
            return Err(Error::InvalidDisclosure);
        }
        if challenge
            .required_claims
            .iter()
            .any(|k| !disclose_keys.contains(k))
        {
            return Err(Error::MissingRequiredClaims);
        }
        let params = parameters(message_count(self.claims.len())?)?;
        let values = messages(*self.secret, &self.id, &self.claims)?;
        let mut disclosed = Vec::new();
        for (i, (key, value)) in self.claims.iter().enumerate() {
            if disclose_keys.contains(key) {
                disclosed.push(DisclosedClaim {
                    index: u64::try_from(i).map_err(|_| CoreError::InvalidClaims)?,
                    key: key.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
        let n = u64::try_from(self.claims.len()).map_err(|_| CoreError::InvalidClaims)?;
        let revealed = revealed(&self.id, n, &disclosed)?;
        let base = pseudonym_base(&self.id, trusted_origin)?;
        let b = Zeroizing::new(nonzero(rng));
        let nym = base.mul_bigint(self.secret.into_bigint()).into_affine();
        let commitment = base.mul_bigint(b.into_bigint()).into_affine();
        // 索引 0 的盲化因子同时用于化名关系；其他隐藏属性独立随机化。
        let choices = values.iter().enumerate().map(|(i, m)| {
            if i == 0 {
                MessageOrBlinding::blind_message_with(m, *b)
            } else if revealed.contains_key(&i) {
                MessageOrBlinding::RevealMessage(m)
            } else {
                MessageOrBlinding::BlindMessageRandomly(m)
            }
        });
        let pok = PoKOfSignatureG1Protocol::init(rng, &self.signature, &params, choices)
            .map_err(|_| Error::ProofGeneration)?;
        let mut contribution = Vec::new();
        pok.challenge_contribution(&revealed, &params, &mut contribution)
            .map_err(|_| Error::ProofGeneration)?;
        let mut presentation = Presentation {
            version: PROTOCOL_VERSION,
            challenge: challenge.clone(),
            pseudonym: encode(&nym)?,
            pseudonym_commitment: encode(&commitment)?,
            credential_proof: String::new(),
            claim_count: n,
            disclosed_claims: disclosed,
        };
        let c = login_challenge(&presentation, &self.key, &base, &contribution)?;
        presentation.credential_proof =
            encode(&pok.gen_proof(&c).map_err(|_| Error::ProofGeneration)?)?;
        Ok(presentation)
    }
    /// 导出包含全部属性的未加密备份，与最近一次披露子集无关。
    pub fn export_secret(&self) -> Result<SecretBytes> {
        validate_claims(&self.claims, claims_limits())?;
        let backup = HolderBackup {
            version: PROTOCOL_VERSION,
            issuer: Json(self.issuer.clone()),
            secret: Zeroizing::new(encode(&*self.secret)?),
            credential: Zeroizing::new(encode(&self.signature)?),
            claims: self.claims.clone(),
        };
        Ok(SecretBytes::from_vec(
            serde_json::to_vec(&backup).map_err(json_error)?,
        ))
    }
    /// 校验完整属性和凭证签名后恢复；不会信任备份声称的组织或秘密。
    pub fn import_secret(bytes: &[u8], expected_issuer: &IssuerPublic) -> Result<Self> {
        let limits = claims_limits();
        let id = expected_issuer.issuer_id()?;
        let backup: HolderBackup = serde_json::from_slice(bytes).map_err(json_error)?;
        version(backup.version)?;
        if backup.issuer.0 != *expected_issuer {
            return Err(Error::IssuerMismatch);
        }
        validate_claims(&backup.claims, limits)?;
        let x = Zeroizing::new(scalar(&backup.secret)?);
        if x.is_zero() {
            return Err(Error::InvalidCredential);
        }
        let sig = signature(&backup.credential).map_err(|_| Error::InvalidCredential)?;
        let key = public_key(&expected_issuer.public_key)?;
        let values = messages(*x, &id, &backup.claims)?;
        sig.verify(&values, key.clone(), parameters(values.len())?)
            .map_err(|_| Error::InvalidCredential)?;
        Ok(Self {
            issuer: expected_issuer.clone(),
            id,
            key,
            secret: x,
            claims: backup.claims,
            signature: sig,
        })
    }
}
impl PendingEnrollment {
    /// 目标签发组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.id
    }
    /// 本次申领标识。
    pub fn request_id(&self) -> &RequestId {
        &self.request.request_id
    }
    /// 创建或恢复时校验的完整请求哈希。
    pub fn request_hash(&self) -> &RequestHash {
        &self.hash
    }
    /// 本次申领的完整属性。
    pub fn claims(&self) -> &Claims {
        &self.request.claims
    }
    /// 检查响应上下文、解盲并验证最终签名；失败后仍可重试。
    pub fn finish(&self, response: &BlindCredential) -> Result<Holder> {
        let limits = claims_limits();
        version(response.version)?;
        if response.issuer_id != self.id {
            return Err(Error::IssuerMismatch);
        }
        if response.request_id != self.request.request_id
            || response.request_hash != self.hash
            || response.claims != self.request.claims
        {
            return Err(Error::EnrollmentMismatch);
        }
        validate_claims(&response.claims, limits)?;
        validate(&self.request, limits)?;
        let sig = signature(&response.signature)
            .map_err(|_| Error::InvalidCredential)?
            .unblind(&self.blinding);
        let key = public_key(&self.issuer.public_key)?;
        let values = messages(*self.secret, &self.id, &self.request.claims)?;
        sig.verify(&values, key.clone(), parameters(values.len())?)
            .map_err(|_| Error::InvalidCredential)?;
        Ok(Holder {
            issuer: self.issuer.clone(),
            id: self.id.clone(),
            key,
            secret: Zeroizing::new(*self.secret),
            claims: self.request.claims.clone(),
            signature: sig,
        })
    }
    /// 导出包含完整原请求及开口秘密的未加密本地备份。
    pub fn export_secret(&self) -> Result<SecretBytes> {
        validate(&self.request, claims_limits())?;
        let backup = PendingBackup {
            version: PROTOCOL_VERSION,
            issuer: Json(self.issuer.clone()),
            request: Json(self.request.clone()),
            request_hash: self.hash.clone(),
            secret: Zeroizing::new(encode(&*self.secret)?),
            blinding: Zeroizing::new(encode(&*self.blinding)?),
        };
        Ok(SecretBytes::from_vec(
            serde_json::to_vec(&backup).map_err(json_error)?,
        ))
    }
    /// 验证原请求、哈希、知识证明和承诺开口后恢复；最终签名仍由 finish 验证。
    pub fn import_secret(bytes: &[u8], expected_issuer: &IssuerPublic) -> Result<Self> {
        let id = expected_issuer.issuer_id()?;
        let backup: PendingBackup = serde_json::from_slice(bytes).map_err(json_error)?;
        version(backup.version)?;
        if backup.issuer.0 != *expected_issuer || backup.request.0.issuer_id != id {
            return Err(Error::IssuerMismatch);
        }
        let request = backup.request.0;
        if request.request_hash()? != backup.request_hash {
            return Err(Error::EnrollmentMismatch);
        }
        let x = Zeroizing::new(scalar(&backup.secret)?);
        let r = Zeroizing::new(scalar(&backup.blinding)?);
        let params = parameters(message_count(request.claims.len())?)?;
        if x.is_zero()
            || r.is_zero()
            || !verify_opening(&request, &params)?
            || params
                .commit_to_messages([(0, &*x)], &r)
                .map_err(|_| CoreError::Crypto)?
                != point(&request.commitment)?
        {
            return Err(Error::InvalidCredential);
        }
        Ok(Self {
            issuer: expected_issuer.clone(),
            id,
            request,
            hash: backup.request_hash,
            secret: x,
            blinding: r,
        })
    }
}
