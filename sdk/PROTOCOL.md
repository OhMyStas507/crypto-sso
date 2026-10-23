# v1 实现编码补充

本文将 API 设计中未逐字列出的套件标识与私密备份字段固定下来，供实现审阅和后续互通使用。公开 API 的规范仍以 [sdk-api.md](../docs/sdk-api.md) 为准。

## 数学套件和参数

曲线为 BLS12-381。参数和化名基点使用 arkworks 0.4 的 WBMap / SSWU、XMD SHA-256、128 位安全参数 hash-to-curve。G1 和 G2 的 DST 分别为：

```text
CRYPTO_SSO_V1_BLS12381G1_XMD:SHA-256_SSWU_RO_
CRYPTO_SSO_V1_BLS12381G2_XMD:SHA-256_SSWU_RO_
```

令 `D = "crypto-sso/v1/bbsplus/fixed-parameters"`，固定基点输入为 `LP(D) || LP("g1")`、`LP(D) || LP("g2")`、`LP(D) || LP("h0")`，其中 g2 映射到 G2，其他映射到 G1。消息基点 j 的输入严格为 `LP(D) || LP("message") || u64_le(j)`。只按实际消息数量派生；固定基点被缓存，现有基点不随数量或配置改变。

组织指纹的参数规则字符串固定为源码 `PARAMETER_RULE` 的字节，并同时保存在向量 `parameters.rule` 中。不能替换该字符串、套件、标签或规则而仍宣称使用相同 v1 指纹。

`LP` 使用 u64 小端字节长度。SHA-512 产生标量时，将摘要解释为小端整数并约减到 Fr。所有数学对象使用 arkworks 规范压缩格式再转小写 hex；G1 为 48 字节，G2 为 96 字节，Fr 为 32 字节，BBS+ 签名为 112 字节。标量解码不接受非规范域整数。

## 域分离与转录

下表各个字段都使用 LP 编码，特别注明的原始 u64 字节除外。

| 用途 | 标签及后续字段 |
|---|---|
| 组织指纹，SHA-256 | `crypto-sso/v1/issuer`；单字节版本、组织 UTF-8、公钥 hex ASCII、参数规则字符串 |
| 请求哈希，SHA-256 | `crypto-sso/v1/enrollment-request`；完整规范请求 JSON |
| 组织标量，SHA-512 → Fr | `crypto-sso/v1/organization-message`；组织指纹 hex ASCII |
| 属性标量，SHA-512 → Fr | `crypto-sso/v1/claim`；`utf8-string`、键 UTF-8、值 UTF-8 |
| 申领挑战，SHA-512 → Fr | `crypto-sso/v1/enrollment-proof`；版本、指纹、请求标识、本次完整参数压缩编码、秘密承诺、证明承诺、规范 Claims JSON |
| 化名基点，G1 hash-to-curve | `crypto-sso/v1/pseudonym-scope`；指纹、规范 origin |
| 客户端绑定，SHA-256 | `crypto-sso/v1/client-binding`；32 字节秘密的小写 hex ASCII |
| 登录挑战，SHA-512 → Fr | `crypto-sso/v1/login-proof`；版本、完整规范挑战 JSON、u64 小端属性数、规范披露数组 JSON、公钥压缩编码、化名基点、化名、化名承诺、BBS+ 完整挑战贡献 |

签名消息布局为 `[x, m_org, Fr(n), m_0, …]`。公开消息恰为 1、2 和所有 `3 + disclosed.index`。化名关系使用 `get_resp_for_message(0, revealed_indices)`，不接受独立秘密响应。

## 完整证明边界

固定依赖 `bbs_plus = 0.25.0`，其 G1 完整证明压缩布局为：3 个随机化签名 G1、第一关系的 G1 承诺及 2 个 Fr、第二关系 G1 承诺、`Some` 单字节标记、u64 小端响应数量、响应数组、`None` 单字节标记。

隐藏消息数量为 `1 + n - disclosed_count`，第二关系响应数量为 `3 + n - disclosed_count`，总字节数为 `314 + 32 * response_count`。SDK 在分配或调用底层容器解码前，先检查外层精确长度、两个 Option 标记和内嵌数量。部分响应、增删响应、截断、超限和计数溢出都拒绝。

## 私密备份

全部为封闭 JSON 对象，拒绝未知或重复字段，`version` 为 1。字段顺序如下：

| 备份 | 字段 |
|---|---|
| issuer | `version`、`public`、`secret_key` |
| pending | `version`、`issuer`、`request`、`request_hash`、`secret`、`blinding` |
| holder | `version`、`issuer`、`secret`、`credential`、`claims` |

嵌入的公共消息保持规范公共结构，私钥和秘密标量为规范 hex。holder 凭证为解盲后的完整签名。备份不会写入本地 ClaimsLimits，也不包含最近一次披露选择。导入始终检查完整预期组织信息；pending 还检查请求哈希、证明和开口，holder 重新验签全部属性。
