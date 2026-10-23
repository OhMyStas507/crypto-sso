/// 本角色的稳定错误类别；不保存原始秘密或敏感输入。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// 公共值类型、编码或数学错误。
    #[error("{0}")]
    Core(#[from] crypto_sso_core::Error),
    /// 签发组织不匹配。
    #[error("签发组织不匹配")]
    IssuerMismatch,
    /// 可信应用来源不匹配。
    #[error("可信应用来源不匹配")]
    OriginMismatch,
    /// 申领上下文不匹配。
    #[error("申领上下文不匹配")]
    EnrollmentMismatch,
    /// 凭证签名无效。
    #[error("凭证签名无效")]
    InvalidCredential,
    /// 选择了未知属性。
    #[error("选择了未知属性")]
    InvalidDisclosure,
    /// 缺少必需披露属性。
    #[error("缺少必需披露属性")]
    MissingRequiredClaims,
    /// 挑战时间无效。
    #[error("挑战时间无效")]
    InvalidChallengeTime,
    /// 挑战已到期。
    #[error("挑战已到期")]
    ChallengeExpired,
    /// 证明生成失败。
    #[error("证明生成失败")]
    ProofGeneration,
}

/// 本角色方法的结果。
pub type Result<T> = std::result::Result<T, Error>;
