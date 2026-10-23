# 协议 v1 固定向量

`vectors.json` 仅包含合成秘密。版本、参考提交、随机算法、32 字节固定种子、Unix 秒、预期结果和生成命令均记录在 `metadata` 中。参考实现不包含本项目属性布局，因此这些不是与原参考项目互通的向量。

向量包含五种消息的规范 JSON、组织指纹、请求哈希、固定用途基点、消息基点前缀、组织与属性标量、完整秘密备份、空/部分/全部披露以及格式合法但值被篡改的无效证明。

`tests/vectors.rs` 从已提交文件重验，并独立拼接 u64 小端 LP 编码计算 SHA-256、SHA-512 和模标量域约减；消息基点输入也独立按规范拼接。`verify_hashes.py` 提供只依赖 Python 标准库的第二种哈希核对实现。曲线运算由固定版本 arkworks 执行，向量测试不代表独立密码学审计。

生成新候选向量：

```sh
cargo run -p crypto-sso-verifier --example export_v1 -- NEW_FILE.json
python sdk/verifier/tests/fixtures/v1/verify_hashes.py
```

生成器使用 `create_new`，拒绝覆盖已有文件。修改规范或升级密码学依赖后，应审查候选差异；正常测试绝不重写期望值。此目录随 verifier 包一起发布，不依赖被忽略的 references 目录。
