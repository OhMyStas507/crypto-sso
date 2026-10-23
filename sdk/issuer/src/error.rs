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
    /// 申领开口证明无效。
    #[error("申领开口证明无效")]
    InvalidEnrollmentProof,
    /// 核准属性与请求不匹配。
    #[error("核准属性与请求不匹配")]
    ClaimsMismatch,
}

/// 本角色方法的结果。
pub type Result<T> = std::result::Result<T, Error>;
