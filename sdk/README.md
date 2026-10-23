# Crypto SSO SDK v0.1

协议 v1 的同步 Rust SDK：BBS+ 属性盲签发、解盲验签、选择披露、应用域化名和共享秘密组合证明。时间和 `rand 0.8` 的安全随机源由调用方显式传入；SDK 不读取文件、网络、时钟或数据库。

| 调用方 | 依赖 | 主要入口 |
|---|---|---|
| 签发服务 | `crypto-sso-issuer` | `Issuer::generate`、`issue_blind`、私钥备份 |
| 钱包 | `crypto-sso-holder` | `Holder::begin_enrollment`、`PendingEnrollment::finish`、`Holder::present` |
| 应用后端 | `crypto-sso-verifier` | `Verifier::create_challenge`、`verify_proof` |
| SDK 角色实现 | `crypto-sso-core` | 值类型、严格 wire、固定参数和公共数学原语 |

三个角色仅正常依赖 core，不相互依赖。各角色重导出业务所需的共享类型、`wire` 和 `CoreError/CoreResult`。四个包版本同步，角色精确依赖 core `=0.1.0`。根 workspace 管理全部包，`sdk/` 不是额外 crate 或嵌套 workspace。

## 使用顺序

1. 签发方生成组织密钥，通过可信渠道分发 `IssuerPublic`。
2. 钱包提交完整 `Claims`，先安全保存 pending，再发送 `EnrollmentRequest`。
3. 签发服务从可信资料取得核准属性，调用 `issue_blind`，保存响应和唯一性台账。
4. 钱包调用 `finish`，成功保存 holder 后清理 pending。
5. 应用固定 issuer 与 origin，创建并保存完整挑战，独立下发客户端 Cookie 秘密。
6. 钱包取得可信 origin 和用户明确同意的 `ClaimKeys`，生成证明。
7. 应用从存储读取挑战，从 HTTP Cookie 取得独立秘密，调用 `verify_proof`；仅从 `VerifiedProof` 读取身份和属性。
8. 应用执行属性值策略，重新读取提交时间，在事务中检查完整挑战和 `expires_at > commit_now`，原子消费挑战并创建会话。

可执行三方流程位于 [offline_flow.rs](verifier/tests/offline_flow.rs)，完整接口契约见 [sdk-api.md](../docs/sdk-api.md)。出示端和验证端在领证后只需 holder 凭证和组织公钥。

## 数据与资源契约

- 仅五种公共消息支持 `wire::to_json/from_json`。拒绝重复/未知字段、错误类型、非规范编码、非法群元素、错序集合、不完整证明及伪造长度。
- 属性是字符串键值对，按键排序，不修改空白或 Unicode 形式；披露须显式选择，未披露与空字符串有不同含义。
- `ClaimsLimits` 默认为每集合 128 键、键 64 字节、值 1024 字节。这些是可配置的本地策略，不进入协议指纹、哈希或证明。
- 如需自定义限制，在任何 SDK 调用与工作线程启动之前，通过 `unsafe set_claims_limits` 初始化一次，并在运行期间保持不变。
- `SecretBytes` 是未加密备份；秘密类型不实现 `Debug`、`Clone` 或通用 Serde。调用方负责加密存储及其副本清理。
- `verify_proof` 是无状态验真，可以重复成功；它不执行自然人身份授权、撤销、业务权限策略或会话创建。

## 验证与发布

```sh
cargo test -p crypto-sso-core -p crypto-sso-issuer -p crypto-sso-holder -p crypto-sso-verifier
cargo fmt -p crypto-sso-core -p crypto-sso-issuer -p crypto-sso-holder -p crypto-sso-verifier -- --check
cargo clippy -p crypto-sso-core -p crypto-sso-issuer -p crypto-sso-holder -p crypto-sso-verifier --all-targets -- -D warnings
cargo tree -p crypto-sso-issuer -e normal
cargo tree -p crypto-sso-holder -e normal
cargo tree -p crypto-sso-verifier -e normal
python sdk/verifier/tests/fixtures/v1/verify_hashes.py
```

[测试覆盖清单](TEST-COVERAGE.md) 逐项对应测试文档编号。自定义配置使用独立进程，三方正常流程仅使用 0、1、3 对属性。SV 服务层要点由内存事务模型覆盖调用契约；真实数据库、HTTP Cookie、加密存储和钱包 UI 的验收仍属于对应集成工程。

[固定协议向量](verifier/tests/fixtures/v1/README.md) 和生成器随 verifier 包发布；测试不依赖 references。生成器仅创建新文件，不自动覆盖基线。协议具体编码补充见 [PROTOCOL.md](PROTOCOL.md)。发布顺序为 core，再发布三个角色。

数学流程参考 `org-anon-auth` 的指定提交，保留 Apache-2.0 许可与各包 `NOTICE.md`；实现已扩展为本项目带属性的 v1 协议。
