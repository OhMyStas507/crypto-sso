//! 跨角色共享的数学原语；不提供签发、出示或完整验真的角色流程。
//!
//! 修改自 org-anon-auth（Apache-2.0），参考提交 ebf343609d1ba05c9e5a6043e1ba95ca4efe0188。
//! 本实现重新定义 v1 参数规则、属性消息、可变隐藏响应及严格解码。
use crate::*;
pub use ark_bls12_381::{Bls12_381, Fr, G1Affine, G1Projective, G2Affine};
use ark_bls12_381::{G2Projective, g1, g2};
use ark_ec::hashing::{
    HashToCurve, curve_maps::wb::WBMap, map_to_curve_hasher::MapToCurveBasedHasher,
};
pub use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::field_hashers::DefaultFieldHasher;
pub use ark_ff::{PrimeField, Zero};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
pub use ark_std::UniformRand;
pub use bbs_plus::{
    proof::PoKOfSignatureG1Protocol,
    setup::{PublicKeyG2, SecretKey},
};
use rand::{CryptoRng, RngCore};
use sha2::{Digest, Sha256, Sha512};
use std::{collections::BTreeMap, sync::OnceLock};
use zeroize::Zeroizing;

/// 固定规则派生的 BBS+ 参数，仅供角色实现使用。
pub type Params = bbs_plus::setup::SignatureParamsG1<Bls12_381>;
/// 完整 BBS+ 签名；不得作为出示消息直接传输。
pub type Signature = bbs_plus::signature::SignatureG1<Bls12_381>;
/// BBS+ 完整持有证明，包含全部隐藏消息响应。
pub type Proof = bbs_plus::proof::PoKOfSignatureG1Proof<Bls12_381>;
/// 经过子群检查的 G2 组织公钥类型。
pub type PublicKey = PublicKeyG2<Bls12_381>;

/// 固定参数与消息基点的独立用途标签。
pub const PARAMETER_DOMAIN: &[u8] = b"crypto-sso/v1/bbsplus/fixed-parameters";
/// G1 hash-to-curve 的协议专用域分离标签。
pub const G1_DST: &[u8] = b"CRYPTO_SSO_V1_BLS12381G1_XMD:SHA-256_SSWU_RO_";
/// G2 hash-to-curve 的协议专用域分离标签。
pub const G2_DST: &[u8] = b"CRYPTO_SSO_V1_BLS12381G2_XMD:SHA-256_SSWU_RO_";
// 指纹固定整个派生规则，而不是枚举受本次属性数量影响的基点列表。
/// 进入组织指纹的完整派生规则标识，不包含本地限制或属性数量。
pub const PARAMETER_RULE:&[u8] = b"BLS12-381;G1,G2:XMD:SHA-256_SSWU_RO_;DST:CRYPTO_SSO_V1_BLS12381G{1,2}_XMD:SHA-256_SSWU_RO_;LP:u64le;domain:crypto-sso/v1/bbsplus/fixed-parameters;fixed:g1,g2,h0;message:LP(domain)||LP(message)||u64le(index)";

/// 检查协议或私密备份版本，保留实际收到的版本号。
pub fn version(v: u8) -> Result<()> {
    if v == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(Error::UnsupportedVersion { found: v })
    }
}
/// 校验组织名的 UTF-8 长度、空白和控制字符。
pub fn organization(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 128 || s.trim() != s || s.chars().any(char::is_control) {
        Err(Error::InvalidIssuer)
    } else {
        Ok(())
    }
}
/// 将内部 JSON 错误映射为不带原始输入的公共错误。
pub fn json_error(e: serde_json::Error) -> Error {
    // 只映射固定类别，不将 Serde 的原始输入片段挂到错误链上。
    if e.to_string().contains("属性集合无效") {
        Error::InvalidClaims
    } else {
        Error::Serialization
    }
}
/// 使用本次操作的配置快照重新验证可变公共 DTO。
pub fn validate<T: wire::WireMessage>(value: &T, limits: ClaimsLimits) -> Result<()> {
    wire::validate(value, limits)
}
/// 用指定快照检查不可变属性集合，供角色入口和备份调用。
pub fn validate_claims(value: &Claims, limits: ClaimsLimits) -> Result<()> {
    value.validate(limits)
}
/// 用指定快照检查选择键或挑战必需键。
pub fn validate_keys(value: &ClaimKeys, limits: ClaimsLimits) -> Result<()> {
    value.validate(limits)
}
/// 检查固定 32 字节小写 hex 标识。
pub fn token(value: &str) -> Result<()> {
    crate::values::token(value)
}

/// 仅供角色私有备份嵌入公共 DTO；保留原始 JSON，避免 Value 覆盖重复键。
#[derive(Clone, PartialEq, Eq)]
pub struct Json<T>(
    /// 嵌入私密备份的公共消息，导入方仍须验证角色上下文。
    pub T,
);
impl<T: wire::WireMessage> serde::Serialize for Json<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let bytes = wire::to_json(&self.0).map_err(serde::ser::Error::custom)?;
        let text = String::from_utf8(bytes).map_err(serde::ser::Error::custom)?;
        let raw =
            serde_json::value::RawValue::from_string(text).map_err(serde::ser::Error::custom)?;
        raw.serialize(s)
    }
}
impl<'de, T: wire::WireMessage> serde::Deserialize<'de> for Json<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = <Box<serde_json::value::RawValue> as serde::Deserialize>::deserialize(d)?;
        wire::decode_nested(raw.get().as_bytes())
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// 规范压缩编码为小写 hex，并擦除中间字节缓冲区。
pub fn encode<T: CanonicalSerialize>(value: &T) -> Result<String> {
    let mut bytes = Zeroizing::new(Vec::new());
    value
        .serialize_compressed(&mut *bytes)
        .map_err(|_| Error::Crypto)?;
    Ok(hex::encode(&*bytes))
}
/// 在分配前检查精确长度和小写 hex，再返回可擦除字节。
pub fn hex_bytes(value: &str, expected: usize, field: &'static str) -> Result<Zeroizing<Vec<u8>>> {
    if expected.checked_mul(2) != Some(value.len())
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::InvalidEncoding { field });
    }
    Ok(Zeroizing::new(
        hex::decode(value).map_err(|_| Error::InvalidEncoding { field })?,
    ))
}
fn decode_fixed<T: CanonicalDeserialize>(
    value: &str,
    size: usize,
    field: &'static str,
) -> Result<T> {
    let bytes = hex_bytes(value, size, field)?;
    let mut cursor = bytes.as_slice();
    let result =
        T::deserialize_compressed(&mut cursor).map_err(|_| Error::InvalidEncoding { field })?;
    if !cursor.is_empty() {
        return Err(Error::InvalidEncoding { field });
    }
    Ok(result)
}
/// 解码规范的 32 字节标量；拒绝超出标量域的整数。
pub fn scalar(value: &str) -> Result<Fr> {
    decode_fixed(value, 32, "scalar")
}
/// 解码非零 G1 点，并验证曲线和素数阶子群。
pub fn point(value: &str) -> Result<G1Affine> {
    let point: G1Affine = decode_fixed(value, 48, "G1")?;
    if point.is_zero() {
        return Err(Error::InvalidEncoding { field: "G1" });
    }
    Ok(point)
}
/// 解码非零 G2 公钥并验证其子群归属。
pub fn public_key(value: &str) -> Result<PublicKey> {
    let key: PublicKey = decode_fixed(value, 96, "public_key")?;
    if !key.is_valid() {
        return Err(Error::InvalidIssuer);
    }
    Ok(key)
}
/// 检查 BBS+ 签名的固定长度和规范群元素编码。
pub fn signature(value: &str) -> Result<Signature> {
    let signature: Signature = decode_fixed(value, 112, "signature")?;
    if signature.A.is_zero() {
        return Err(Error::InvalidEncoding { field: "signature" });
    }
    Ok(signature)
}

/// 检查属性数到消息数 3+n 的转换和溢出。
pub fn message_count(n: usize) -> Result<usize> {
    u64::try_from(n)
        .ok()
        .and_then(|n| n.checked_add(3))
        .and_then(|m| usize::try_from(m).ok())
        .ok_or(Error::InvalidClaims)
}
/// 检查公开属性计数的本地限制、平台可表示性和消息布局。
pub fn count(n: u64, limits: ClaimsLimits) -> Result<usize> {
    let n = usize::try_from(n).map_err(|_| Error::InvalidClaims)?;
    if n > limits.max_keys {
        return Err(Error::InvalidClaims);
    }
    message_count(n)?;
    Ok(n)
}
/// 检查披露项的数量、键值、槽位界限及两种严格升序关系。
pub fn disclosures(n: u64, items: &[DisclosedClaim], limits: ClaimsLimits) -> Result<()> {
    let n = count(n, limits)?;
    if items.len() > n {
        return Err(Error::InvalidClaims);
    }
    let mut previous: Option<&DisclosedClaim> = None;
    for item in items {
        if usize::try_from(item.index).map_or(true, |i| i >= n)
            || !crate::claims::key_valid(&item.key, limits)
            || item.value.len() > limits.max_value_bytes
            || previous.is_some_and(|p| p.index >= item.index || p.key >= item.key)
        {
            return Err(Error::InvalidClaims);
        }
        previous = Some(item);
    }
    Ok(())
}

/// 在反序列化任何可变长度容器前核对完整证明的精确布局。
pub fn proof(value: &str, n: u64, disclosed: usize, limits: ClaimsLimits) -> Result<Proof> {
    let responses = count(n, limits)?
        .checked_sub(disclosed)
        .and_then(|n| n.checked_add(3))
        .ok_or(Error::InvalidEncoding {
            field: "credential_proof",
        })?;
    // bbs_plus 0.25 的完整证明布局：5 个 G1、2 个标量、Some 标记、
    // u64 响应数量、响应标量数组、None 标记。先验证长度再触及 Vec 解码。
    let size = responses
        .checked_mul(32)
        .and_then(|n| n.checked_add(314))
        .ok_or(Error::InvalidEncoding {
            field: "credential_proof",
        })?;
    let bytes = hex_bytes(value, size, "credential_proof")?;
    let declared = u64::from_le_bytes(bytes[305..313].try_into().map_err(|_| Error::Crypto)?);
    if bytes[304] != 1
        || declared != u64::try_from(responses).map_err(|_| Error::Crypto)?
        || bytes[size - 1] != 0
    {
        return Err(Error::InvalidEncoding {
            field: "credential_proof",
        });
    }
    let mut cursor = bytes.as_slice();
    let proof = Proof::deserialize_compressed(&mut cursor).map_err(|_| Error::InvalidEncoding {
        field: "credential_proof",
    })?;
    if proof.A_prime.is_zero()
        || !cursor.is_empty()
        || proof.sc_partial_resp_2.is_some()
        || proof.sc_resp_2.as_ref().map(|r| r.0.len()) != Some(responses)
    {
        return Err(Error::InvalidEncoding {
            field: "credential_proof",
        });
    }
    Ok(proof)
}

/// 从调用方提供的安全随机源生成非零标量。
pub fn nonzero<R: RngCore + CryptoRng>(rng: &mut R) -> Fr {
    loop {
        let v = Fr::rand(rng);
        if !v.is_zero() {
            return v;
        }
    }
}
/// 生成 256 位随机标识并擦除临时字节。
pub fn random_token<R: RngCore + CryptoRng>(rng: &mut R) -> String {
    let mut bytes = Zeroizing::new([0u8; 32]);
    rng.fill_bytes(&mut *bytes);
    hex::encode(bytes.as_slice())
}

/// 用 u64 小端长度前缀消除字段拼接歧义的 Fiat–Shamir 转录。
pub struct Transcript(
    /// 已按协议顺序编码的字节，仅包含当前用途的转录内容。
    pub Vec<u8>,
);
impl Transcript {
    /// 先写入用途标签，禁止不同证明类型共用挑战域。
    pub fn new(domain: &[u8]) -> Result<Self> {
        let mut t = Self(Vec::new());
        t.append(domain)?;
        Ok(t)
    }
    /// 附加一个 LP 编码字段；原始 UTF-8 字节保持不变。
    pub fn append(&mut self, bytes: &[u8]) -> Result<()> {
        let length = u64::try_from(bytes.len()).map_err(|_| Error::Crypto)?;
        let additional = bytes.len().checked_add(8).ok_or(Error::Crypto)?;
        self.0.len().checked_add(additional).ok_or(Error::Crypto)?;
        self.0.try_reserve(additional).map_err(|_| Error::Crypto)?;
        self.0.extend_from_slice(&length.to_le_bytes());
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    /// 压缩编码数学对象后作为一个完整 LP 字段附加。
    pub fn point<T: CanonicalSerialize>(&mut self, v: &T) -> Result<()> {
        let mut bytes = Vec::new();
        v.serialize_compressed(&mut bytes)
            .map_err(|_| Error::Crypto)?;
        self.append(&bytes)
    }
    /// 将 SHA-512 输出按小端整数约减到标量域。
    pub fn scalar(&self) -> Fr {
        Fr::from_le_bytes_mod_order(&Sha512::digest(&self.0))
    }
}
/// 对带 LP 编码和用途标签的转录计算 SHA-256。
pub fn hash_hex(domain: &[u8], parts: &[&[u8]]) -> Result<String> {
    let mut t = Transcript::new(domain)?;
    for p in parts {
        t.append(p)?;
    }
    Ok(hex::encode(Sha256::digest(&t.0)))
}
/// 按协议固定套件对输入执行 G1 hash-to-curve。
pub fn hash_g1(bytes: &[u8]) -> Result<G1Affine> {
    type H =
        MapToCurveBasedHasher<G1Projective, DefaultFieldHasher<Sha256, 128>, WBMap<g1::Config>>;
    let p = H::new(G1_DST)
        .map_err(|_| Error::Crypto)?
        .hash(bytes)
        .map_err(|_| Error::Crypto)?;
    if p.is_zero() {
        return Err(Error::Crypto);
    }
    Ok(p)
}
fn hash_g2(bytes: &[u8]) -> Result<G2Affine> {
    type H =
        MapToCurveBasedHasher<G2Projective, DefaultFieldHasher<Sha256, 128>, WBMap<g2::Config>>;
    let p = H::new(G2_DST)
        .map_err(|_| Error::Crypto)?
        .hash(bytes)
        .map_err(|_| Error::Crypto)?;
    if p.is_zero() {
        return Err(Error::Crypto);
    }
    Ok(p)
}
/// 只派生实际需要的消息基点，固定用途基点由 OnceLock 缓存。
pub fn parameters(messages: usize) -> Result<Params> {
    static FIXED: OnceLock<Result<(G1Affine, G2Affine, G1Affine)>> = OnceLock::new();
    let fixed = FIXED
        .get_or_init(|| {
            let make = |label: &[u8]| -> Result<Vec<u8>> {
                let mut t = Transcript::new(PARAMETER_DOMAIN)?;
                t.append(label)?;
                Ok(t.0)
            };
            Ok((
                hash_g1(&make(b"g1")?)?,
                hash_g2(&make(b"g2")?)?,
                hash_g1(&make(b"h0")?)?,
            ))
        })
        .as_ref()
        .map_err(|_| Error::Crypto)?;
    let mut h = Vec::new();
    for i in 0..messages {
        let mut t = Transcript::new(PARAMETER_DOMAIN)?;
        t.append(b"message")?;
        t.0.extend_from_slice(&u64::try_from(i).map_err(|_| Error::Crypto)?.to_le_bytes());
        h.push(hash_g1(&t.0)?);
    }
    Ok(Params {
        g1: fixed.0,
        g2: fixed.1,
        h_0: fixed.2,
        h,
    })
}
/// 将组织指纹放入独立域，计算签名消息索引 1。
pub fn organization_message(id: &IssuerId) -> Result<Fr> {
    let mut t = Transcript::new(b"crypto-sso/v1/organization-message")?;
    t.append(id.as_str().as_bytes())?;
    Ok(t.scalar())
}
/// 联合绑定类型、键和值，计算一个属性的签名标量。
pub fn claim_message(key: &str, value: &str) -> Result<Fr> {
    let mut t = Transcript::new(b"crypto-sso/v1/claim")?;
    t.append(b"utf8-string")?;
    t.append(key.as_bytes())?;
    t.append(value.as_bytes())?;
    Ok(t.scalar())
}
/// 构造 [x, m_org, Fr(n), 属性标量...]，临时消息数组在释放时擦除。
pub fn messages(x: Fr, id: &IssuerId, claims: &Claims) -> Result<Zeroizing<Vec<Fr>>> {
    message_count(claims.len())?;
    let mut values = Zeroizing::new(vec![
        x,
        organization_message(id)?,
        Fr::from(u64::try_from(claims.len()).map_err(|_| Error::InvalidClaims)?),
    ]);
    for (k, v) in claims.iter() {
        values.push(claim_message(k, v)?);
    }
    Ok(values)
}
/// 将组织、计数及披露槽位映射到真实 BBS+ 消息索引。
pub fn revealed(id: &IssuerId, n: u64, items: &[DisclosedClaim]) -> Result<BTreeMap<usize, Fr>> {
    let mut values = BTreeMap::from([(1, organization_message(id)?), (2, Fr::from(n))]);
    for item in items {
        let i = usize::try_from(item.index)
            .ok()
            .and_then(|i| i.checked_add(3))
            .ok_or(Error::InvalidClaims)?;
        values.insert(i, claim_message(&item.key, &item.value)?);
    }
    Ok(values)
}
/// 将参数、请求标识、承诺和完整规范属性绑定到申领挑战。
pub fn enrollment_challenge(request: &EnrollmentRequest, params: &Params) -> Result<Fr> {
    let mut t = Transcript::new(b"crypto-sso/v1/enrollment-proof")?;
    t.append(&[request.version])?;
    t.append(request.issuer_id.as_str().as_bytes())?;
    t.append(request.request_id.as_str().as_bytes())?;
    t.point(params)?;
    t.point(&point(&request.commitment)?)?;
    t.point(&point(&request.proof_commitment)?)?;
    t.append(&serde_json::to_vec(&request.claims).map_err(json_error)?)?;
    Ok(t.scalar())
}
/// 验证两个秘密的承诺开口方程；不签名、不建立角色信任源。
pub fn verify_opening(request: &EnrollmentRequest, params: &Params) -> Result<bool> {
    let c = enrollment_challenge(request, params)?;
    Ok(
        params.h[0].mul_bigint(scalar(&request.response_secret)?.into_bigint())
            + params
                .h_0
                .mul_bigint(scalar(&request.response_blinding)?.into_bigint())
            == point(&request.proof_commitment)?.into_group()
                + point(&request.commitment)?.mul_bigint(c.into_bigint()),
    )
}
/// 根据组织指纹和可信规范 origin 派生应用化名基点。
pub fn pseudonym_base(id: &IssuerId, origin: &Origin) -> Result<G1Affine> {
    let mut t = Transcript::new(b"crypto-sso/v1/pseudonym-scope")?;
    t.append(id.as_str().as_bytes())?;
    t.append(origin.as_str().as_bytes())?;
    hash_g1(&t.0)
}
/// 绑定完整挑战、披露内容、公钥、化名和 BBS+ 挑战贡献。
pub fn login_challenge(
    p: &Presentation,
    key: &PublicKey,
    base: &G1Affine,
    contribution: &[u8],
) -> Result<Fr> {
    let mut t = Transcript::new(b"crypto-sso/v1/login-proof")?;
    t.append(&[p.version])?;
    t.append(&wire::to_json(&p.challenge)?)?;
    t.append(&p.claim_count.to_le_bytes())?;
    t.append(&serde_json::to_vec(&p.disclosed_claims).map_err(json_error)?)?;
    t.point(key)?;
    t.point(base)?;
    t.point(&point(&p.pseudonym)?)?;
    t.point(&point(&p.pseudonym_commitment)?)?;
    t.append(contribution)?;
    Ok(t.scalar())
}
/// 将独立客户端 Cookie 秘密哈希为公开挑战绑定值。
pub fn binding_hash(cookie: &str) -> Result<String> {
    hash_hex(b"crypto-sso/v1/client-binding", &[cookie.as_bytes()])
}
/// 由各角色映射到其公开错误枚举的时间校验结果。
pub enum TimeError {
    /// 时间顺序、TTL 或未来偏差不合法。
    Invalid,
    /// 当前时间已经达到或超过到期秒。
    Expired,
}
/// 检查严格正数且不超过 120 秒的挑战 TTL。
pub fn time_structure(c: &LoginChallenge) -> std::result::Result<(), TimeError> {
    let ttl = c
        .expires_at
        .checked_sub(c.issued_at)
        .ok_or(TimeError::Invalid)?;
    if ttl == 0 || ttl > MAX_CHALLENGE_TTL_SECS {
        return Err(TimeError::Invalid);
    }
    Ok(())
}
/// 检查 TTL、未来偏差和立即到期语义，不读取系统时钟。
pub fn check_time(c: &LoginChallenge, now: u64) -> std::result::Result<(), TimeError> {
    time_structure(c)?;
    // 用差值比较未来偏差，避免 now + 30 在 u64 上界溢出。
    if c.issued_at > now
        && c.issued_at.checked_sub(now).ok_or(TimeError::Invalid)? > MAX_FUTURE_SKEW_SECS
    {
        return Err(TimeError::Invalid);
    }
    if now >= c.expires_at {
        return Err(TimeError::Expired);
    }
    Ok(())
}
