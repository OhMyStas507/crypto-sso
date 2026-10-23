//! 应用验证方：固定组织和 origin，检查客户端绑定并验证组合证明。
//!
//! 验真是无状态的；应用须在成功后原子消费挑战并提交会话。
//! 参考 org-anon-auth（Apache-2.0）；本实现增加属性选择披露和共享秘密检查。
#![warn(missing_docs)]
mod error;
#[cfg(test)]
mod tests;
mod type_contracts;
use crypto_sso_core::{__private::*, MAX_CHALLENGE_TTL_SECS, PROTOCOL_VERSION};
pub use crypto_sso_core::{
    ClaimKeys, Claims, ClaimsLimits, DEFAULT_CLAIMS_LIMITS, DisclosedClaim, Error as CoreError,
    IssuerId, IssuerPublic, LoginChallenge, Nonce, Origin, Presentation, Result as CoreResult,
    Subject, claims_limits, set_claims_limits, wire,
};
pub use error::{Error, Result};
use rand::{CryptoRng, RngCore};
use zeroize::Zeroizing;

/// 预配置的验证上下文，不会从请求中动态切换信任源。
pub struct Verifier {
    id: IssuerId,
    origin: Origin,
    key: PublicKey,
    base: G1Affine,
}
/// 与公开挑战独立的预登录 Cookie 秘密，不是登录后的会话令牌。
pub struct ClientBindingSecret(Zeroizing<String>);
impl ClientBindingSecret {
    /// 从本次 HTTP 请求的独立 Cookie 导入固定 32 字节小写 hex 秘密。
    pub fn from_cookie_value(value: &str) -> Result<Self> {
        token(value)?;
        Ok(Self(Zeroizing::new(value.to_owned())))
    }
    /// 显式暴露 Cookie 值；生产由应用设置 HttpOnly、Secure、SameSite。
    pub fn expose_cookie_value(&self) -> &str {
        &self.0
    }
}
/// 唯有验真成功才能创建的身份与属性结果；调用方仍须执行值策略和原子消费。
#[must_use]
pub struct VerifiedProof {
    subject: Subject,
    challenge: LoginChallenge,
    claims: Claims,
}
impl VerifiedProof {
    /// 按组织和应用 origin 解释的可信化名。
    pub fn subject(&self) -> &Subject {
        &self.subject
    }
    /// 本次验真的完整挑战，供应用原子比较并消费。
    pub fn challenge(&self) -> &LoginChallenge {
        &self.challenge
    }
    /// 本次已验证的披露属性；未披露的键不出现在结果中。
    pub fn disclosed_claims(&self) -> &Claims {
        &self.claims
    }
}
impl Verifier {
    /// 固定来自可信渠道的组织信息和应用 origin，并缓存已验证公钥及化名基点。
    pub fn new(trusted_issuer: IssuerPublic, origin: Origin) -> Result<Self> {
        let id = trusted_issuer.issuer_id()?;
        let key = public_key(&trusted_issuer.public_key)?;
        let base = pseudonym_base(&id, &origin)?;
        Ok(Self {
            id,
            origin,
            key,
            base,
        })
    }
    /// 固定的组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.id
    }
    /// 固定的应用 origin。
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    /// 创建固定 120 秒挑战及独立客户端秘密；调用方保存完整挑战并下发 Cookie。
    pub fn create_challenge<R: RngCore + CryptoRng>(
        &self,
        required_claims: &ClaimKeys,
        now: u64,
        rng: &mut R,
    ) -> Result<(LoginChallenge, ClientBindingSecret)> {
        validate_keys(required_claims, claims_limits())?;
        let expires_at = now
            .checked_add(MAX_CHALLENGE_TTL_SECS)
            .ok_or(Error::InvalidChallengeTime)?;
        let nonce = Nonce::parse(&random_token(rng))?;
        let binding = ClientBindingSecret(Zeroizing::new(random_token(rng)));
        let challenge = LoginChallenge {
            version: PROTOCOL_VERSION,
            issuer_id: self.id.clone(),
            audience: self.origin.clone(),
            nonce,
            issued_at: now,
            expires_at,
            session_binding: binding_hash(binding.expose_cookie_value())?,
            required_claims: required_claims.clone(),
        };
        Ok((challenge, binding))
    }
    /// 按完整挑战、固定上下文、时效、客户端绑定、披露及组合证明的顺序验真。
    ///
    /// `stored_challenge` 必须来自应用存储；`client_binding` 来自独立 Cookie。
    /// 此方法可重复成功，不消费挑战；`now` 必须取自服务端时钟。
    pub fn verify_proof(
        &self,
        presentation: &Presentation,
        stored_challenge: &LoginChallenge,
        client_binding: &ClientBindingSecret,
        now: u64,
    ) -> Result<VerifiedProof> {
        if presentation.challenge != *stored_challenge {
            return Err(Error::ChallengeMismatch);
        }
        version(presentation.version)?;
        version(stored_challenge.version)?;
        if stored_challenge.issuer_id != self.id {
            return Err(Error::IssuerMismatch);
        }
        if stored_challenge.audience != self.origin {
            return Err(Error::OriginMismatch);
        }
        check_time(stored_challenge, now).map_err(|e| match e {
            TimeError::Invalid => Error::InvalidChallengeTime,
            TimeError::Expired => Error::ChallengeExpired,
        })?;
        let limits = claims_limits();
        validate(stored_challenge, limits)?;
        // 固定长度比较所有字节，不按首个不同字节提前退出。
        let expected = binding_hash(client_binding.expose_cookie_value())?;
        let difference = expected
            .bytes()
            .zip(stored_challenge.session_binding.bytes())
            .fold(0u8, |a, (x, y)| a | (x ^ y));
        if difference != 0 {
            return Err(Error::ClientBindingMismatch);
        }
        disclosures(
            presentation.claim_count,
            &presentation.disclosed_claims,
            limits,
        )
        .map_err(|_| Error::InvalidPresentation)?;
        if stored_challenge.required_claims.iter().any(|key| {
            !presentation
                .disclosed_claims
                .iter()
                .any(|item| item.key == key)
        }) {
            return Err(Error::MissingRequiredClaims);
        }
        let nym = point(&presentation.pseudonym)?;
        let commitment = point(&presentation.pseudonym_commitment)?;
        let proof = proof(
            &presentation.credential_proof,
            presentation.claim_count,
            presentation.disclosed_claims.len(),
            limits,
        )
        .map_err(|_| Error::InvalidPresentation)?;
        let params = parameters(message_count(count(presentation.claim_count, limits)?)?)?;
        let revealed = revealed(
            &self.id,
            presentation.claim_count,
            &presentation.disclosed_claims,
        )?;
        let mut contribution = Vec::new();
        proof
            .challenge_contribution(&revealed, &params, &mut contribution)
            .map_err(|_| Error::InvalidPresentation)?;
        let c = login_challenge(presentation, &self.key, &self.base, &contribution)?;
        proof
            .verify(&revealed, &c, self.key.clone(), params)
            .map_err(|_| Error::InvalidPresentation)?;
        // 必须按消息索引取 x 的响应，不能复用任意隐藏属性或另一个秘密的响应。
        let response = proof
            .get_resp_for_message(0, &revealed.keys().copied().collect())
            .map_err(|_| Error::InvalidPresentation)?;
        if self.base.mul_bigint(response.into_bigint())
            != commitment.into_group() + nym.mul_bigint(c.into_bigint())
        {
            return Err(Error::InvalidPresentation);
        }
        let claims = Claims::new(
            presentation
                .disclosed_claims
                .iter()
                .map(|item| (item.key.clone(), item.value.clone())),
        )?;
        Ok(VerifiedProof {
            subject: Subject::parse(&presentation.pseudonym)?,
            challenge: stored_challenge.clone(),
            claims,
        })
    }
}
