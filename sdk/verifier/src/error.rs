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
    /// 应用来源不匹配。
    #[error("应用来源不匹配")]
    OriginMismatch,
    /// 完整挑战与存储不一致。
    #[error("完整挑战与存储不一致")]
    ChallengeMismatch,
    /// 挑战时间无效。
    #[error("挑战时间无效")]
    InvalidChallengeTime,
    /// 挑战已到期。
    #[error("挑战已到期")]
    ChallengeExpired,
    /// 客户端绑定不匹配。
    #[error("客户端绑定不匹配")]
    ClientBindingMismatch,
    /// 组合证明无效。
    #[error("组合证明无效")]
    InvalidPresentation,
    /// 缺少必需披露属性。
    #[error("缺少必需披露属性")]
    MissingRequiredClaims,
}

/// 本角色方法的结果。
pub type Result<T> = std::result::Result<T, Error>;
