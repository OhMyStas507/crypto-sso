/// 不包含原始输入、属性值或秘密的公共错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// 不支持的协议或备份版本。
    #[error("不支持的版本：{found}")]
    UnsupportedVersion {
        /// 收到的版本号。
        found: u8,
    },
    /// 字段超出本地资源策略。
    #[error("输入超过限制：{limit}")]
    InputTooLarge {
        /// 当前允许的上限。
        limit: usize,
    },
    /// 非规范编码或无效密码学结构。
    #[error("字段编码无效：{field}")]
    InvalidEncoding {
        /// 固定的字段名称，不包含原始输入。
        field: &'static str,
    },
    /// 组织名或公钥无效。
    #[error("签发组织无效")]
    InvalidIssuer,
    /// 来源不是允许的裸 origin。
    #[error("应用来源无效")]
    InvalidOrigin,
    /// 属性语法、重复、排序或资源限制不合规。
    #[error("属性集合无效")]
    InvalidClaims,
    /// 属性限制配置不合法。
    #[error("属性限制配置无效")]
    InvalidClaimsLimits,
    /// JSON 结构或类型无效。
    #[error("序列化结构无效")]
    Serialization,
    /// 基础数学运算失败。
    #[error("密码学运算失败")]
    Crypto,
}

/// 公共协议操作的结果。
pub type Result<T> = std::result::Result<T, Error>;
