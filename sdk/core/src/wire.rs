//! 五种公共交换消息的封闭、严格 JSON 编解码。
use crate::{__private as math, *};
use serde::{Deserialize, Serialize};
use std::fmt;

mod private {
    use super::*;
    pub trait Sealed: Sized {
        fn validate(&self, limits: ClaimsLimits) -> Result<()>;
        fn encode(&self) -> Result<Vec<u8>>;
        fn decode(bytes: &[u8]) -> Result<Self>;
    }
}
/// 封闭消息标记，只有本 SDK 的五种顶层交换消息实现。
pub trait WireMessage: private::Sealed {}

/// 校验当前 DTO 内容，输出固定字段顺序的紧凑 JSON。
pub fn to_json<T: WireMessage>(value: &T) -> Result<Vec<u8>> {
    value.validate(claims_limits())?;
    value.encode()
}
/// 严格解析并检查当前资源限制；不执行角色签名或证明验真。
pub fn from_json<T: WireMessage>(bytes: &[u8]) -> Result<T> {
    let limits = claims_limits();
    let value = T::decode(bytes)?;
    value.validate(limits)?;
    Ok(value)
}
pub(crate) fn validate<T: WireMessage>(value: &T, limits: ClaimsLimits) -> Result<()> {
    value.validate(limits)
}
pub(crate) fn decode_nested<T: WireMessage>(bytes: &[u8]) -> Result<T> {
    T::decode(bytes)
}

// 公共 DTO 不实现 Deserialize；私有镜像只在严格入口内部使用。
macro_rules! message {
    ($name:ident, $raw:ident, $doc:literal, {$($(#[$meta:meta])* $field:ident : $ty:ty $(=> $adapter:literal)?),* $(,)?}) => {
        #[doc=$doc]
        #[derive(Clone, Eq, PartialEq)]
        pub struct $name { $($(#[$meta])* pub $field: $ty),* }
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $raw { $($(#[serde(with=$adapter)])? $field: $ty),* }
        impl private::Sealed for $name {
            fn validate(&self,limits:ClaimsLimits) -> Result<()> { self.check(limits) }
            fn encode(&self) -> Result<Vec<u8>> {
                serde_json::to_vec(&$raw { $($field:self.$field.clone()),* }).map_err(math::json_error)
            }
            fn decode(bytes:&[u8]) -> Result<Self> {
                let value: $raw = serde_json::from_slice(bytes).map_err(math::json_error)?;
                Ok(Self { $($field:value.$field),* })
            }
        }
        impl WireMessage for $name {}
        impl fmt::Debug for $name {
            fn fmt(&self,f:&mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct(stringify!($name)).finish_non_exhaustive() }
        }
    }
}
message!(IssuerPublic, PublicDto, "组织公开信任源；必须从可信渠道固定。", {
    /// 协议版本。
    version:u8,
    /// 组织名称。
    organization:String,
    /// G2 公钥的规范压缩 hex。
    public_key:String,
});
message!(EnrollmentRequest, EnrollmentDto, "钱包提交的承诺、开口证明和待核准属性。", {
    /// 协议版本。
    version:u8,
    /// 目标签发组织指纹。
    issuer_id:IssuerId,
    /// 随机申领标识。
    request_id:RequestId,
    /// 隐藏钱包秘密和盲因子的 G1 承诺。
    commitment:String,
    /// 开口知识证明的 G1 承诺。
    proof_commitment:String,
    /// 钱包秘密的 Schnorr 响应。
    response_secret:String,
    /// 盲因子的 Schnorr 响应。
    response_blinding:String,
    /// 完整的待核准明文属性。
    claims:Claims,
});
message!(BlindCredential, BlindDto, "绑定完整申领请求的盲签响应。", {
    /// 协议版本。
    version:u8,
    /// 签发组织指纹。
    issuer_id:IssuerId,
    /// 原申领标识。
    request_id:RequestId,
    /// 完整规范请求的哈希。
    request_hash:RequestHash,
    /// BBS+ 盲签名的规范压缩 hex。
    signature:String,
    /// 签发方核准的完整属性。
    claims:Claims,
});
message!(LoginChallenge, ChallengeDto, "应用保存并发送给钱包的公开登录挑战。", {
    /// 协议版本。
    version:u8,
    /// 预配置的签发组织指纹。
    issuer_id:IssuerId,
    /// 规范 origin，沿用协议字段名 audience。
    audience:Origin,
    /// 查询挑战存储的随机标识。
    nonce:Nonce,
    /// 签发 Unix 秒。
    issued_at:u64,
    /// 到期 Unix 秒，不包含宽限期。
    expires_at:u64,
    /// 独立客户端秘密的域分离哈希。
    session_binding:String,
    /// 本次必须披露的键，不约束其业务值。
    required_claims:ClaimKeys,
});
message!(Presentation, PresentationDto, "随机化凭证证明、应用化名及选中属性；验真前均不可信。", {
    /// 协议版本。
    version:u8,
    /// 完整挑战副本，必须与应用存储逐字段一致。
    challenge:LoginChallenge => "challenge_serde",
    /// 非零 G1 应用化名的规范压缩 hex。
    pseudonym:String,
    /// 化名关系证明承诺。
    pseudonym_commitment:String,
    /// 完整的 BBS+ 持有证明。
    credential_proof:String,
    /// 完整凭证中的属性总数。
    claim_count:u64,
    /// 按槽位严格递增的选择披露项。
    disclosed_claims:Vec<DisclosedClaim> => "disclosed_serde",
});

mod challenge_serde {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &LoginChallenge,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        math::Json(v.clone()).serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<LoginChallenge, D::Error> {
        Ok(math::Json::<LoginChallenge>::deserialize(d)?.0)
    }
}

mod disclosed_serde {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        v: &[DisclosedClaim],
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        v.serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<DisclosedClaim>, D::Error> {
        struct Items;
        impl<'de> serde::de::Visitor<'de> for Items {
            type Value = Vec<DisclosedClaim>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("有序披露数组")
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> std::result::Result<Self::Value, S::Error> {
                let limits = claims_limits();
                let mut items: Vec<DisclosedClaim> = Vec::new();
                while let Some(item) = seq.next_element::<DisclosedClaim>()? {
                    if items.len() >= limits.max_keys
                        || !crate::claims::key_valid(&item.key, limits)
                        || item.value.len() > limits.max_value_bytes
                        || items
                            .last()
                            .is_some_and(|last| last.index >= item.index || last.key >= item.key)
                    {
                        return Err(serde::de::Error::custom(Error::InvalidClaims));
                    }
                    items.push(item);
                }
                Ok(items)
            }
        }
        d.deserialize_seq(Items)
    }
}

/// 出示消息内嵌的披露项，索引为属性槽位而非 BBS+ 消息索引。
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisclosedClaim {
    /// 原排序属性集合中的零基槽位。
    pub index: u64,
    /// 已选择披露的键。
    pub key: String,
    /// 已选择披露的原始字符串值。
    pub value: String,
}
impl fmt::Debug for DisclosedClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisclosedClaim")
            .field("index", &self.index)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl IssuerPublic {
    fn check(&self, _: ClaimsLimits) -> Result<()> {
        math::version(self.version)?;
        math::organization(&self.organization)?;
        math::public_key(&self.public_key)?;
        Ok(())
    }
    /// 协议版本。
    pub fn version(&self) -> u8 {
        self.version
    }
    /// 组织名称。
    pub fn organization(&self) -> &str {
        &self.organization
    }
    /// 规范 G2 公钥编码。
    pub fn public_key_hex(&self) -> &str {
        &self.public_key
    }
    /// 根据当前 DTO 内容重新验证并计算指纹，不缓存信任判断。
    pub fn issuer_id(&self) -> Result<IssuerId> {
        self.check(claims_limits())?;
        IssuerId::parse(&math::hash_hex(
            b"crypto-sso/v1/issuer",
            &[
                &[self.version],
                self.organization.as_bytes(),
                self.public_key.as_bytes(),
                math::PARAMETER_RULE,
            ],
        )?)
    }
}
impl EnrollmentRequest {
    fn check(&self, limits: ClaimsLimits) -> Result<()> {
        math::version(self.version)?;
        self.claims.validate(limits)?;
        math::message_count(self.claims.len())?;
        let _ = math::point(&self.commitment)?;
        let _ = math::point(&self.proof_commitment)?;
        math::scalar(&self.response_secret)?;
        math::scalar(&self.response_blinding)?;
        Ok(())
    }
    /// 目标组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.issuer_id
    }
    /// 申领标识。
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    /// 当前完整请求的规范哈希；不代表其开口证明有效。
    pub fn request_hash(&self) -> Result<RequestHash> {
        RequestHash::parse(&math::hash_hex(
            b"crypto-sso/v1/enrollment-request",
            &[&to_json(self)?],
        )?)
    }
    /// 待签署的完整属性。
    pub fn claims(&self) -> &Claims {
        &self.claims
    }
}
impl BlindCredential {
    fn check(&self, limits: ClaimsLimits) -> Result<()> {
        math::version(self.version)?;
        self.claims.validate(limits)?;
        math::message_count(self.claims.len())?;
        math::signature(&self.signature)?;
        Ok(())
    }
    /// 组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.issuer_id
    }
    /// 原申领标识。
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    /// 原请求哈希。
    pub fn request_hash(&self) -> &RequestHash {
        &self.request_hash
    }
    /// 完整核准属性；解盲验签后才可信。
    pub fn claims(&self) -> &Claims {
        &self.claims
    }
}
impl LoginChallenge {
    fn check(&self, limits: ClaimsLimits) -> Result<()> {
        math::version(self.version)?;
        crate::values::token(&self.session_binding)?;
        self.required_claims.validate(limits)?;
        if math::time_structure(self).is_err() {
            return Err(Error::InvalidEncoding {
                field: "challenge_time",
            });
        }
        Ok(())
    }
    /// 协议版本。
    pub fn version(&self) -> u8 {
        self.version
    }
    /// 组织指纹。
    pub fn issuer_id(&self) -> &IssuerId {
        &self.issuer_id
    }
    /// 规范化应用 origin。
    pub fn origin(&self) -> &Origin {
        &self.audience
    }
    /// 随机挑战标识。
    pub fn nonce(&self) -> &Nonce {
        &self.nonce
    }
    /// 签发 Unix 秒。
    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }
    /// 到期 Unix 秒。
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
    /// 必须披露的键集合。
    pub fn required_claims(&self) -> &ClaimKeys {
        &self.required_claims
    }
}
impl Presentation {
    fn check(&self, limits: ClaimsLimits) -> Result<()> {
        math::version(self.version)?;
        self.challenge.check(limits)?;
        math::disclosures(self.claim_count, &self.disclosed_claims, limits)?;
        let _ = math::point(&self.pseudonym)?;
        let _ = math::point(&self.pseudonym_commitment)?;
        math::proof(
            &self.credential_proof,
            self.claim_count,
            self.disclosed_claims.len(),
            limits,
        )?;
        Ok(())
    }
    /// 请求携带的完整挑战副本。
    pub fn challenge(&self) -> &LoginChallenge {
        &self.challenge
    }
    /// 完整凭证的属性数。
    pub fn claim_count(&self) -> u64 {
        self.claim_count
    }
    /// 未经信任的披露项；业务应读取 VerifiedProof 中的属性。
    pub fn disclosed_claims(&self) -> &[DisclosedClaim] {
        &self.disclosed_claims
    }
}
