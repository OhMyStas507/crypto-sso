//! 协议 v1 的值类型、严格消息编码和共享数学基础。
//!
//! 公共消息均为不可信 DTO；解析成功不等于签名或证明有效。
//! 本库不读取时钟、文件或网络，部署方负责输入总量及并发限制。
#![warn(missing_docs)]

#[path = "internal/mod.rs"]
#[doc(hidden)]
pub mod __private;
mod claims;
mod error;
mod limits;
mod origin;
mod secret;
mod values;
pub mod wire;

pub use claims::{ClaimKeys, Claims};
pub use error::{Error, Result};
pub use limits::{ClaimsLimits, DEFAULT_CLAIMS_LIMITS, claims_limits, set_claims_limits};
pub use origin::Origin;
pub use secret::SecretBytes;
pub use values::{IssuerId, Nonce, RequestHash, RequestId, Subject};
pub use wire::{
    BlindCredential, DisclosedClaim, EnrollmentRequest, IssuerPublic, LoginChallenge, Presentation,
};

/// 交换协议版本，与 crate 版本分别管理。
pub const PROTOCOL_VERSION: u8 = 1;
/// 创建挑战的固定有效秒数，也是接收挑战允许的最大 TTL。
pub const MAX_CHALLENGE_TTL_SECS: u64 = 120;
/// 允许挑战签发时间领先当前时钟的最大秒数。
pub const MAX_FUTURE_SKEW_SECS: u64 = 30;

#[cfg(test)]
mod tests;
