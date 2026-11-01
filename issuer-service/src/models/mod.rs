//! Model 层：按业务聚合划分实体、会话与领域规则。
mod audit;
pub mod domain;
mod enrollment;
mod session;
mod settings;

pub use audit::*;
pub use enrollment::*;
pub use session::*;
pub use settings::*;
