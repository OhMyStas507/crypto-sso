//! 候选私钥以认证加密持久化；数据库只启用不可覆盖的文件引用，事务回滚不会替换密钥。
use crate::{
    config::Config,
    error::{Error, Result},
    security::valid_token,
};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use crypto_sso_issuer::{Issuer, IssuerPublic};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize)]
struct Material {
    public: String,
    secret: Zeroizing<Vec<u8>>,
}

fn unavailable() -> Error {
    Error::new(
        503,
        "KEY_STORE_UNAVAILABLE",
        "密钥存储不可用，请联系部署维护者后重试原操作",
    )
}

pub fn prepare_directory(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    protect_windows_directory(path)?;
    Ok(())
}

fn encryption_key(config: &Config) -> Result<Zeroizing<[u8; 32]>> {
    let root = &config.setup_storage_key;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(root).map_err(|_| unavailable())?;
    mac.update(b"crypto-sso/setup/key-encryption/v1");
    Ok(Zeroizing::new(mac.finalize().into_bytes().into()))
}

#[cfg(windows)]
fn protect_windows_directory(path: &Path) -> anyhow::Result<()> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, LocalFree},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            },
            DACL_SECURITY_INFORMATION, GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION,
            SetFileSecurityW, TOKEN_QUERY, TOKEN_USER, TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct Local(*mut core::ffi::c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    // 所有指针仅在对应所有者存活期间使用；Win32 返回的句柄和本地内存由守卫释放。
    unsafe {
        let mut token_handle = null_mut();
        anyhow::ensure!(
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle) != 0,
            "无法读取服务账号令牌"
        );
        let token_handle = Handle(token_handle);
        let mut length = 0;
        GetTokenInformation(token_handle.0, TokenUser, null_mut(), 0, &mut length);
        anyhow::ensure!(length > 0, "无法读取服务账号 SID");
        let mut buffer = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
        anyhow::ensure!(
            GetTokenInformation(
                token_handle.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                length,
                &mut length
            ) != 0,
            "无法读取服务账号 SID"
        );
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid_text = null_mut();
        anyhow::ensure!(
            ConvertSidToStringSidW(user.User.Sid, &mut sid_text) != 0,
            "无法转换服务账号 SID"
        );
        let sid_memory = Local(sid_text.cast());
        let mut count = 0;
        while *sid_text.add(count) != 0 {
            count += 1;
        }
        let sid = String::from_utf16(std::slice::from_raw_parts(sid_text, count))?;
        drop(sid_memory);
        let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;{sid})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = null_mut();
        anyhow::ensure!(
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut()
            ) != 0,
            "无法建立密钥目录访问控制"
        );
        let descriptor = Local(descriptor);
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        anyhow::ensure!(!wide[..wide.len() - 1].contains(&0), "密钥目录路径不合法");
        anyhow::ensure!(
            SetFileSecurityW(
                wide.as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor.0
            ) != 0,
            "无法限制密钥目录访问权限"
        );
    }
    Ok(())
}

fn path(config: &Config, id: &str) -> Result<PathBuf> {
    if !valid_token(id) {
        return Err(Error::invalid());
    }
    Ok(config.setup_dir.join(format!("key-{id}.enc")))
}

pub fn write_exclusive(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    let persisted = file.persist_noclobber(target).map_err(|e| e.error)?;
    persisted.sync_all()?;
    #[cfg(unix)]
    fs::File::open(target.parent().unwrap())?.sync_all()?;
    Ok(())
}

pub fn load(config: &Config, id: &str) -> Result<Issuer> {
    let bytes = fs::read(path(config, id)?).map_err(|_| unavailable())?;
    if bytes.len() < 13 || bytes.len() > 64 * 1024 {
        return Err(unavailable());
    }
    let key = encryption_key(config)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| unavailable())?;
    let plain = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(&bytes[..12]),
                Payload {
                    msg: &bytes[12..],
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| unavailable())?,
    );
    let material: Material = serde_json::from_slice(&plain).map_err(|_| unavailable())?;
    let public = crypto_sso_issuer::wire::from_json::<IssuerPublic>(material.public.as_bytes())
        .map_err(|_| unavailable())?;
    Issuer::import_secret(&material.secret, &public).map_err(|_| unavailable())
}

pub fn generate_or_load(config: &Config, id: &str, organization: &str) -> Result<Issuer> {
    let target = path(config, id)?;
    if !target.exists() {
        let issuer = Issuer::generate(organization, &mut OsRng).map_err(|_| {
            Error::new(
                422,
                "VALIDATION_FAILED",
                "组织身份必须为 1–128 字节且不含首尾空白或控制字符",
            )
        })?;
        let material = Material {
            public: String::from_utf8(
                crypto_sso_issuer::wire::to_json(issuer.public()).map_err(|_| unavailable())?,
            )
            .map_err(|_| unavailable())?,
            secret: Zeroizing::new(
                issuer
                    .export_secret()
                    .map_err(|_| unavailable())?
                    .expose_secret()
                    .to_vec(),
            ),
        };
        let plain = Zeroizing::new(serde_json::to_vec(&material).map_err(|_| unavailable())?);
        let key = encryption_key(config)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref()).map_err(|_| unavailable())?;
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let mut bytes = nonce.to_vec();
        bytes.extend(
            cipher
                .encrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: &plain,
                        aad: id.as_bytes(),
                    },
                )
                .map_err(|_| unavailable())?,
        );
        if let Err(error) = write_exclusive(&target, &bytes)
            && error.kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(unavailable());
        }
    }
    let issuer = load(config, id)?;
    if issuer.public().organization() != organization {
        return Err(unavailable());
    }
    Ok(issuer)
}

pub fn remove_candidate(config: &Config, id: &str) -> Result<()> {
    match fs::remove_file(path(config, id)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(unavailable()),
    }
}
