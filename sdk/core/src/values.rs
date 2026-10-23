use crate::{Error, Result};

macro_rules! value_type {
    ($name:ident, $doc:literal, $validate:expr) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Hash)]
        pub struct $name(String);
        impl $name {
            /// 检查规范编码并创建值；不赋予业务身份可信性。
            pub fn parse(value: &str) -> Result<Self> {
                ($validate)(value)?;
                Ok(Self(value.to_owned()))
            }
            /// 返回规范的小写 hex。
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                s.serialize_str(&self.0)
            }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(
                d: D,
            ) -> std::result::Result<Self, D::Error> {
                Self::parse(&<String as serde::Deserialize>::deserialize(d)?)
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}
pub(crate) fn token(value: &str) -> Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(Error::InvalidEncoding { field: "token" })
    }
}
value_type!(IssuerId, "绑定组织、公钥及固定参数规则的指纹。", token);
value_type!(RequestId, "仅用于申领匹配的 256 位随机标识。", token);
value_type!(RequestHash, "完整规范申领请求的域分离哈希。", token);
value_type!(Nonce, "用于查询待处理挑战的 256 位随机标识。", token);
value_type!(
    Subject,
    "非零 G1 化名；只有验真结果中的值可信。",
    |s| crate::__private::point(s).map(|_| ())
);
