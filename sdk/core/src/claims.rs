use crate::{ClaimsLimits, Error, Result, claims_limits};
use serde::{
    Deserialize, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub(crate) fn key_valid(key: &str, limits: ClaimsLimits) -> bool {
    !key.is_empty()
        && key.len() <= limits.max_key_bytes
        && key.as_bytes()[0].is_ascii_lowercase()
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
}

/// 不可原地修改的字符串属性集合；按照键的 ASCII 字节排序。
#[derive(Clone, Eq, PartialEq, Default)]
pub struct Claims(BTreeMap<String, String>);
impl Claims {
    /// 从键值对构造；在插入映射之前检测重复，不覆盖已有值。
    pub fn new<I, K, V>(entries: I) -> Result<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let limits = claims_limits();
        let mut map = BTreeMap::new();
        for (key, value) in entries {
            let (key, value) = (key.into(), value.into());
            if map.len() >= limits.max_keys
                || !key_valid(&key, limits)
                || value.len() > limits.max_value_bytes
                || map.contains_key(&key)
            {
                return Err(Error::InvalidClaims);
            }
            map.insert(key, value);
        }
        Ok(Self(map))
    }
    /// 不包含属性的集合。
    pub fn empty() -> Self {
        Self::default()
    }
    /// 属性数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// 是否没有属性。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// 查找原始字符串值；未披露或缺失时为 `None`，空值为 `Some("")`。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
    /// 返回按键排序的完整键集合。
    pub fn keys(&self) -> ClaimKeys {
        ClaimKeys(self.0.keys().cloned().collect())
    }
    /// 按键排序迭代，不 trim 或规范化属性值。
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
    pub(crate) fn validate(&self, limits: ClaimsLimits) -> Result<()> {
        if self.len() > limits.max_keys
            || self
                .iter()
                .any(|(k, v)| !key_valid(k, limits) || v.len() > limits.max_value_bytes)
        {
            return Err(Error::InvalidClaims);
        }
        Ok(())
    }
}
impl fmt::Debug for Claims {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Claims")
            .field("count", &self.len())
            .finish_non_exhaustive()
    }
}
impl Serialize for Claims {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.validate(claims_limits())
            .map_err(serde::ser::Error::custom)?;
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Claims {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct ClaimsVisitor(ClaimsLimits);
        impl<'de> Visitor<'de> for ClaimsVisitor {
            type Value = Claims;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("字符串属性对象")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Claims, M::Error> {
                let mut values = BTreeMap::new();
                // 不使用不可信 size_hint 预分配；键一出现就检查重复和数量。
                while let Some(key) = map.next_key::<String>()? {
                    if values.len() >= self.0.max_keys
                        || !key_valid(&key, self.0)
                        || values.contains_key(&key)
                    {
                        return Err(serde::de::Error::custom(Error::InvalidClaims));
                    }
                    let value = map.next_value::<String>()?;
                    if value.len() > self.0.max_value_bytes {
                        return Err(serde::de::Error::custom(Error::InvalidClaims));
                    }
                    values.insert(key, value);
                }
                Ok(Claims(values))
            }
        }
        d.deserialize_map(ClaimsVisitor(claims_limits()))
    }
}

/// 不可原地修改的披露键集合；构造时排序，线格式必须已经有序。
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ClaimKeys(BTreeSet<String>);
impl ClaimKeys {
    /// 创建键集合；拒绝重复键和通配符。
    pub fn new<I, K>(keys: I) -> Result<Self>
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        let limits = claims_limits();
        let mut set = BTreeSet::new();
        for key in keys {
            let key = key.into();
            if set.len() >= limits.max_keys || !key_valid(&key, limits) || !set.insert(key) {
                return Err(Error::InvalidClaims);
            }
        }
        Ok(Self(set))
    }
    /// 空选择；不会自动扩展为全部属性。
    pub fn empty() -> Self {
        Self::default()
    }
    /// 选择的键数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// 是否没有选择任何键。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// 判断是否包含指定键。
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains(key)
    }
    /// 按键的 ASCII 字节顺序迭代。
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
    pub(crate) fn validate(&self, limits: ClaimsLimits) -> Result<()> {
        if self.len() > limits.max_keys || self.iter().any(|k| !key_valid(k, limits)) {
            Err(Error::InvalidClaims)
        } else {
            Ok(())
        }
    }
}
impl Serialize for ClaimKeys {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.validate(claims_limits())
            .map_err(serde::ser::Error::custom)?;
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for ClaimKeys {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct KeysVisitor(ClaimsLimits);
        impl<'de> Visitor<'de> for KeysVisitor {
            type Value = ClaimKeys;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("严格递增的属性键数组")
            }
            fn visit_seq<S: SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> std::result::Result<ClaimKeys, S::Error> {
                let mut keys = BTreeSet::new();
                while let Some(key) = seq.next_element::<String>()? {
                    if keys.len() >= self.0.max_keys
                        || !key_valid(&key, self.0)
                        || keys.last().is_some_and(|last| last >= &key)
                    {
                        return Err(serde::de::Error::custom(Error::InvalidClaims));
                    }
                    keys.insert(key);
                }
                Ok(ClaimKeys(keys))
            }
        }
        d.deserialize_seq(KeysVisitor(claims_limits()))
    }
}
