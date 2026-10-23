# 命令行工具

`utils` 是 workspace 中的独立 crate，命令放在 `src/bin/` 下。

## 生成 issuer 私钥

在仓库根目录运行：

```sh
cargo run -p utils --bin generate-issuer-key -- --organization "Example Org" --output issuer-secret.json
```

- `--organization`（`-n`）：组织名称，必填。遵循 SDK 的校验规则：1–128 个 UTF-8 字节，不允许首尾空白或控制字符。
- `--output`（`-o`）：私钥备份文件路径，必填，相对于当前工作目录；父目录必须已经存在。
- `--help`：查看帮助；`--version`：查看版本。

命令使用操作系统安全随机源生成新的 BBS+ 密钥。文件内容为 SDK `Issuer::export_secret()` 的原始 JSON 备份，包含协议版本、组织公开信息和私钥，可用 `Issuer::import_secret()` 恢复。每次运行都会生成新的密钥。

标准输出仅包含 `IssuerPublic` JSON，可通过 `wire::from_json::<IssuerPublic>()` 读取，供钱包和验证方配置。保存位置和 issuer 指纹写入标准错误。

私钥先写入同目录临时文件，写入成功后保存到目标路径；目标已存在时返回非零退出码，不覆盖原文件。Unix 下私钥文件权限为 `0600`，Windows 下继承目标目录的访问权限。

备份文件**未加密**，应保存到受限目录并妥善保管，不要提交到版本库或分发给钱包和验证方。

## 检查

```sh
cargo test -p utils
cargo fmt -p utils -- --check
cargo clippy -p utils --all-targets -- -D warnings
```
