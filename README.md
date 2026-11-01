# Crypto-SSO
这是一个基于BBS+实现的单点登录系统。
它基于密码学算法完成对用户的校验，用户登录过程中应用程序后端不需要与登录中心通信。

## 系统中的几个角色
用户：真实世界中的用户，应用程序的访问者，持有自身钱包私钥
登录中心：持有信任源的私钥，为用户身份提供签名，
应用程序：用户访问的服务，持有登录中心的公钥，要求用户钱包通过零知识证明验证身份。

## 待完成的工作：
- [ ] 详细的文档
  - [ ] 规范化名词表
- [ ] 完善应用程序开发sdk
  - 常见语言的sdk：
    - [ ] C++ (FFI)
    - [ ] JAVA (FFI)
    - [ ] Springboot starter
    - [ ] Python (FFI)
    - [ ] Go (cgo ffi或重新实现)
    - [ ] Nodejs (FFI或wasm)
  - [ ] 对旧程序的无侵入适配：
    - nginx/caddy等网关的插件（仍需技术方案讨论）
  - [ ] 规范化的，长期稳定的Claims约定（例如必须签发包含("expires_at", "2027/1/1")以表示过期时间，例如身份验证中心应该承诺不签发("role": "admin")这样的正式）
  - [ ] 在登录中心内嵌脚本语言，对于以支持灵活的自动支持/拒绝方案（任务技术方案讨论）
  - [ ] 部分信息应该只保存进入redis而不应该进入postgres主数据库
  - [ ] 无状态化登录中心服务（当前部分Session设计需要依赖于本地数据）
- [ ] 钱包和身份中心的业务流程设计还需要打磨
- [ ] 本地钱包的安全性设计还不是很好，当前只支持windows的安全计算
- [ ] 整理ai的垃圾代码

## 仓库文件夹划分

| 目录 | 当前职责 |
|---|---|
| `crates/identity-api/` | 共享 HTTP 业务契约与 TypeScript 类型导出 |
| `crates/telemetry/` | 共享 tracing 日志初始化 |
| `packages/identity-api-types/` | 生成的 TypeScript HTTP 类型包 |
| `issuer-service/` | 登录中心后端，按 `models / viewmodels / views` 划分 MVVM |
| `issuer-service-webui/` | 独立的 Solid 管理前端 |
| `wallet/src/` | 钱包 Solid 前端与 IPC bridge |
| `wallet/src-tauri/` | 钱包 Rust Engine、加密存储与 Tauri 入口 |
| `examples/relying-party/` | 独立应用后端、登录前端与完整端到端测试 |
| `sdk/` | core、issuer、holder、verifier 四个密码学协议 crate |
| `utils/` | 签发密钥生成等命令行工具，见 [使用说明](utils/README.md) |
| `dev-utils/` | 开发资料抓取工具 |
| `scripts/` | 环境初始化、类型生成校验、UI 与集成检查 |
| `deploy/` | 开发数据库 Compose、管理前端网关模板与部署说明 |
| `docs/` | 业务、当前代码结构、SDK 和静态 GUI 资料 |

SDK 已按 core、issuer、holder、verifier 四个 crate 划分，支持带属性的 BBS+ 盲签发与选择披露。使用方式和检查命令见 [SDK README](sdk/README.md)，测试文档全部 89 个编号的覆盖入口见 [测试覆盖清单](sdk/TEST-COVERAGE.md)。

身份验证中心与钱包的业务设计见 [业务流程文档](docs/identity-center-wallet-business-flow.md)，包含服务设置、Claims 默认建议 API、钱包注册、人工审批与签发、凭证保存及应用登录续接。注册中心、Solid 管理后台、Tauri 钱包和最小应用登录示例已实现；当前启动方式见 [开发与运行](deploy/README.md)。

当前代码模块、实际目录、页面路由及 HTTP/IPC 契约见 [项目结构与模块说明](docs/identity-center-wallet-project-structure.md)，采用 PostgreSQL、Salvo、Toasty、Solid 与 Tauri。

空数据库自动迁移后，可通过 WebUI 设置管理员、服务信息和新的签发密钥，支持草稿恢复及幂等提交。详见 [首次启动与初始化](docs/identity-center-bootstrap-design.md) 和 [GUI 设计图集](docs/gui-design/index.html)。

## 开发数据库

安装并启动 Docker（需支持 Docker Compose v2）。开发容器的数据库、账号、密码、端口和初始化配置全部直接定义在 Compose 文件中，无需准备 `.env`：

```sh
bun scripts/dev-compose.mjs up -d --wait
bun scripts/init-dev-db.mjs
```

[开发 Compose 配置](deploy/compose.dev.yaml) 提供 PostgreSQL 17 和 Redis 7，端口仅绑定 `127.0.0.1`，数据目录使用 `tmpfs`，停止或删除容器后自动清空数据。开发连接地址：

| 服务 | 连接地址 |
| --- | --- |
| PostgreSQL | `postgresql://crypto_sso:crypto_sso_dev@127.0.0.1:5432/crypto_sso` |
| Redis | `redis://127.0.0.1:6379/0`（无密码） |

`dev-compose.mjs` 显式禁用 Docker Compose 自动加载 `.env`，直接透传 Compose 命令。需要调整开发基础设施时修改 `deploy/compose.dev.yaml`；PostgreSQL 每次重新启动都会按配置初始化全新数据库，之后需再次运行 `init-dev-db.mjs` 创建示例和测试数据库。应用自身的连接字符串仍通过环境变量或应用 `.env` 提供，并与 Compose 中的配置保持一致。

```sh
# 停止并移除容器，同时清空所有开发数据库数据
bun scripts/dev-compose.mjs down

# 下次启动全新数据库，并创建示例和测试数据库
bun scripts/dev-compose.mjs up -d --wait
bun scripts/init-dev-db.mjs
```

## 注册中心与钱包

应用基础配置从环境变量读取，开发时使用根目录 `.env`；Docker 基础设施单独由 Compose 文件配置。执行 `bun scripts/setup-dev.js` 生成应用部署配置与加密根密钥，启动后通过管理网页完成中心初始化；完整准备、启动和验收步骤见 [部署说明](deploy/README.md)。

- 注册中心使用 MVVM：`src/models/` 定义持久化实体与领域规则，`src/viewmodels/` 编排业务事务及明确类型的展示契约，`src/views/` 组装 Salvo 路由、Handler、Session 和 CSRF 中间件；`worker.rs` 执行后台签发。业务数据以 Toasty 原生 CRUD 与关联查询持久化至 PostgreSQL。中心和示例应用启动时自动应用迁移，完成后再接收请求；业务写事务使用 Serializable 隔离与自动回滚。应用日志使用 `tracing`，通过 `RUST_LOG` 配置级别。
- 管理后台实现服务设置、Claims 建议、人工核验与审批、成员查询、签发台账和审计。
- 钱包实现中心指纹固定、可编辑 Claims 草稿、加密申领恢复、领证与本地凭证、受信应用和选择披露。
- `examples/relying-party` 提供独立应用挑战、证明验真、原子消费与应用会话闭环；已有凭证登录无需联系中心。
- Windows 钱包使用 DPAPI 保护数据密钥，AES-256-GCM 加密本地记录；不向前端提供 pending、holder 或访问秘密。

验证入口：`scripts/check-integration.ps1`。其中真实数据库验收仅操作名称以 `_test` 结尾的独立测试数据库。
