//! BK-06：秘密类型的编译失败契约。每个块分别验证一种能力，避免一次失败掩盖其他能力。
//!
//! `crypto_sso_issuer::Issuer` 不提供 `std::fmt::Debug`。
//! ```compile_fail
//! fn require<T: std::fmt::Debug>() {}
//! require::<crypto_sso_issuer::Issuer>();
//! ```
//!
//! `crypto_sso_issuer::Issuer` 不提供 `Clone`。
//! ```compile_fail
//! fn require<T: Clone>() {}
//! require::<crypto_sso_issuer::Issuer>();
//! ```
//!
//! `crypto_sso_issuer::Issuer` 不提供 `serde::Serialize`。
//! ```compile_fail
//! fn require<T: serde::Serialize>() {}
//! require::<crypto_sso_issuer::Issuer>();
//! ```
//!
//! `crypto_sso_issuer::Issuer` 不提供 `serde::de::DeserializeOwned`。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_issuer::Issuer>();
//! ```
//!
//! `crypto_sso_holder::Holder` 不提供 `std::fmt::Debug`。
//! ```compile_fail
//! fn require<T: std::fmt::Debug>() {}
//! require::<crypto_sso_holder::Holder>();
//! ```
//!
//! `crypto_sso_holder::Holder` 不提供 `Clone`。
//! ```compile_fail
//! fn require<T: Clone>() {}
//! require::<crypto_sso_holder::Holder>();
//! ```
//!
//! `crypto_sso_holder::Holder` 不提供 `serde::Serialize`。
//! ```compile_fail
//! fn require<T: serde::Serialize>() {}
//! require::<crypto_sso_holder::Holder>();
//! ```
//!
//! `crypto_sso_holder::Holder` 不提供 `serde::de::DeserializeOwned`。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_holder::Holder>();
//! ```
//!
//! `crypto_sso_holder::PendingEnrollment` 不提供 `std::fmt::Debug`。
//! ```compile_fail
//! fn require<T: std::fmt::Debug>() {}
//! require::<crypto_sso_holder::PendingEnrollment>();
//! ```
//!
//! `crypto_sso_holder::PendingEnrollment` 不提供 `Clone`。
//! ```compile_fail
//! fn require<T: Clone>() {}
//! require::<crypto_sso_holder::PendingEnrollment>();
//! ```
//!
//! `crypto_sso_holder::PendingEnrollment` 不提供 `serde::Serialize`。
//! ```compile_fail
//! fn require<T: serde::Serialize>() {}
//! require::<crypto_sso_holder::PendingEnrollment>();
//! ```
//!
//! `crypto_sso_holder::PendingEnrollment` 不提供 `serde::de::DeserializeOwned`。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_holder::PendingEnrollment>();
//! ```
//!
//! `crypto_sso_verifier::ClientBindingSecret` 不提供 `std::fmt::Debug`。
//! ```compile_fail
//! fn require<T: std::fmt::Debug>() {}
//! require::<crypto_sso_verifier::ClientBindingSecret>();
//! ```
//!
//! `crypto_sso_verifier::ClientBindingSecret` 不提供 `Clone`。
//! ```compile_fail
//! fn require<T: Clone>() {}
//! require::<crypto_sso_verifier::ClientBindingSecret>();
//! ```
//!
//! `crypto_sso_verifier::ClientBindingSecret` 不提供 `serde::Serialize`。
//! ```compile_fail
//! fn require<T: serde::Serialize>() {}
//! require::<crypto_sso_verifier::ClientBindingSecret>();
//! ```
//!
//! `crypto_sso_verifier::ClientBindingSecret` 不提供 `serde::de::DeserializeOwned`。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_verifier::ClientBindingSecret>();
//! ```
//!
//! `crypto_sso_core::SecretBytes` 不提供 `std::fmt::Debug`。
//! ```compile_fail
//! fn require<T: std::fmt::Debug>() {}
//! require::<crypto_sso_core::SecretBytes>();
//! ```
//!
//! `crypto_sso_core::SecretBytes` 不提供 `Clone`。
//! ```compile_fail
//! fn require<T: Clone>() {}
//! require::<crypto_sso_core::SecretBytes>();
//! ```
//!
//! `crypto_sso_core::SecretBytes` 不提供 `serde::Serialize`。
//! ```compile_fail
//! fn require<T: serde::Serialize>() {}
//! require::<crypto_sso_core::SecretBytes>();
//! ```
//!
//! `crypto_sso_core::SecretBytes` 不提供 `serde::de::DeserializeOwned`。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_core::SecretBytes>();
//! ```
//!
//! `Claims` 不是额外的顶层公共消息。
//! ```compile_fail
//! fn require<T: crypto_sso_core::wire::WireMessage>() {}
//! require::<crypto_sso_core::Claims>();
//! ```
//!
//! `ClaimKeys` 不是额外的顶层公共消息。
//! ```compile_fail
//! fn require<T: crypto_sso_core::wire::WireMessage>() {}
//! require::<crypto_sso_core::ClaimKeys>();
//! ```
//!
//! `DisclosedClaim` 不是额外的顶层公共消息。
//! ```compile_fail
//! fn require<T: crypto_sso_core::wire::WireMessage>() {}
//! require::<crypto_sso_core::DisclosedClaim>();
//! ```
//!
//! `IssuerPublic` 不能通过通用反序列化绕过 wire 校验。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_core::IssuerPublic>();
//! ```
//!
//! `EnrollmentRequest` 不能通过通用反序列化绕过 wire 校验。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_core::EnrollmentRequest>();
//! ```
//!
//! `BlindCredential` 不能通过通用反序列化绕过 wire 校验。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_core::BlindCredential>();
//! ```
//!
//! `LoginChallenge` 不能通过通用反序列化绕过 wire 校验。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_core::LoginChallenge>();
//! ```
//!
//! `Presentation` 不能通过通用反序列化绕过 wire 校验。
//! ```compile_fail
//! fn require<T: serde::de::DeserializeOwned>() {}
//! require::<crypto_sso_core::Presentation>();
//! ```
//!
//! 验真结果不能由应用直接构造。
//! ```compile_fail
//! let result = crypto_sso_verifier::VerifiedProof {};
//! ```
