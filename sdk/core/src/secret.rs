use zeroize::Zeroizing;

/// 拥有缓冲区的未加密本地备份，释放时擦除 SDK 持有的字节。
///
/// 不实现日志、克隆或通用序列化能力。调用方负责加密存储及其副本清理。
pub struct SecretBytes(Zeroizing<Vec<u8>>);
impl SecretBytes {
    /// 接管缓冲区；此操作不验证秘密格式或凭证有效性。
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
    /// 显式暴露秘密，仅交给本地安全存储或角色导入接口。
    pub fn expose_secret(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    #[test]
    fn bk_06_secret_container_uses_erasable_owned_storage() {
        let mut secret = SecretBytes::from_vec(vec![42; 32]);
        assert_eq!(secret.expose_secret(), &[42; 32]);
        // 检查容器自身仍存活的缓冲区，释放后的内存不再访问。
        secret.0.zeroize();
        assert!(secret.expose_secret().is_empty());
    }
}
