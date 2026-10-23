use crate::{Error, Result};

/// 单个集合的本地资源策略，不参与协议转录或组织指纹。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsLimits {
    /// 单个属性集合或选择集合的最大键数。
    pub max_keys: usize,
    /// 单个键解码后的最大 UTF-8 字节数，至少为 1。
    pub max_key_bytes: usize,
    /// 单个值解码后的最大 UTF-8 字节数，可为 0。
    pub max_value_bytes: usize,
}

/// 默认部署策略；这些数值不是协议硬上限。
pub const DEFAULT_CLAIMS_LIMITS: ClaimsLimits = ClaimsLimits {
    max_keys: 128,
    max_key_bytes: 64,
    max_value_bytes: 1024,
};
static mut CLAIMS_LIMITS: ClaimsLimits = DEFAULT_CLAIMS_LIMITS;

/// 按值返回共享配置快照，不暴露全局变量的引用。
pub fn claims_limits() -> ClaimsLimits {
    // 安全性由设置函数的初始化契约保证：正常使用阶段只有读取。
    unsafe { CLAIMS_LIMITS }
}

/// 在 SDK 使用前设置所有角色共享的属性资源策略。
///
/// # Safety
/// 仅可在进程初始化、其他 SDK 调用开始之前设置。调用方必须保证没有
/// 并发 SDK 读取或配置写入，并在 SDK 开始使用后保持配置不变。
pub unsafe fn set_claims_limits(limits: ClaimsLimits) -> Result<()> {
    if limits.max_key_bytes == 0 {
        return Err(Error::InvalidClaimsLimits);
    }
    // 先完成全部验证，失败不会部分更新配置；不按配置值分配内存。
    unsafe {
        CLAIMS_LIMITS = limits;
    }
    Ok(())
}
