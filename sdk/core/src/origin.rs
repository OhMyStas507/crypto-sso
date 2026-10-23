use crate::{Error, Result};
use url::{Host, Url};

/// 经过校验和规范化的应用 origin，业务路径不构成身份域。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Origin(String);
impl Origin {
    /// 接受 HTTPS 或 HTTP loopback；拒绝认证信息、非根路径、查询和片段。
    pub fn parse(value: &str) -> Result<Self> {
        // URL 库会消除 /a/.. 等路径；先检查原始输入，避免把非根路径误当裸 origin。
        let authority = value
            .split_once("://")
            .map(|(_, tail)| tail)
            .ok_or(Error::InvalidOrigin)?;
        if authority.contains('\\') || authority.find('/').is_some_and(|i| &authority[i..] != "/") {
            return Err(Error::InvalidOrigin);
        }
        let url = Url::parse(value).map_err(|_| Error::InvalidOrigin)?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host().is_none()
            || value.chars().any(char::is_control)
            || value.trim() != value
            || value
                .split_once("://")
                .is_none_or(|(_, tail)| tail.split('/').next().is_some_and(|a| a.contains('@')))
        {
            return Err(Error::InvalidOrigin);
        }
        let loopback = match url.host() {
            Some(Host::Domain("localhost")) => true,
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            return Err(Error::InvalidOrigin);
        }
        Ok(Self(url.origin().ascii_serialization()))
    }
    /// 返回规范的 ASCII origin。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl serde::Serialize for Origin {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}
impl<'de> serde::Deserialize<'de> for Origin {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let value = <String as serde::Deserialize>::deserialize(d)?;
        let origin = Self::parse(&value).map_err(serde::de::Error::custom)?;
        if origin.0 != value {
            return Err(serde::de::Error::custom("origin 必须已规范化"));
        }
        Ok(origin)
    }
}
