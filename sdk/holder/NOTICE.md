# 许可与归属

密码学流程参考并修改自 [org-anon-auth](https://github.com/yhw2003/org-anon-auth)，参考提交 `ebf343609d1ba05c9e5a6043e1ba95ca4efe0188`，原项目采用 Apache License 2.0；完整许可保存在同目录 `LICENSE`。

本 SDK 已修改原流程：按角色拆包，增加 key-value 属性签名、动态消息基点、选择披露、多隐藏消息共享秘密证明、严格线格式、资源配置及私密备份验证。协议使用 `crypto-sso/v1/` 域分离，与参考项目的原始线格式不互通。

底层密码学实现使用 arkworks、bbs_plus 与 dock_crypto_utils，各依赖保留其自身许可。
