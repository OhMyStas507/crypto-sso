# 开发与运行

要求 Rust 1.95+、Bun、Node.js 24+、Docker Compose v2。钱包首期使用 Windows DPAPI；其他系统不会降级为明文存储。Windows 需安装 Tauri 所需的 MSVC 工具链与 WebView2。

应用部署配置来自进程环境变量，开发时从仓库根 `.env` 读取，进程环境优先。开发 Docker 的数据库、账号、密码、端口、健康检查和数据库初始化配置全部直接定义在 `deploy/compose.dev.yaml`；Docker 启动不读取应用 `.env`。不提交 `.env`、私钥或运行数据。前端仅通过同源 HTTP 或受限 IPC 访问服务，数据库密码、管理员哈希和会话密钥不会进入 Vite 客户端环境。

## 首次准备

在仓库根运行：

```powershell
node scripts/dev-compose.mjs up -d --wait
node scripts/init-dev-db.mjs
node scripts/setup-dev.js
bun install --cwd issuer-service-webui
bun install --cwd wallet
bun install --cwd examples/relying-party/web
bun install --cwd scripts
```

`setup-dev.js` 使用 Node.js 内置模块创建 `.env`、固定 issuer 密钥和随机管理员密码，终端仅显示一次开发密码；已有 `.env` 时拒绝覆盖。已有 issuer 私钥会被保留。可通过 `node scripts/setup-dev.js --organization "组织名称"` 指定组织，默认使用“本地开发身份中心”。也可以手工复制 `.env.example`，使用 `utils` 生成密钥，并设置 `ADMIN_BOOTSTRAP_PASSWORD` 后运行 `cargo run -p issuer-service --bin hash-password` 得到 Argon2id 哈希。密码哈希应使用单引号写入 `.env`，避免 `$` 被环境替换。

`dev-compose.mjs` 为 Docker Compose 显式设置 `COMPOSE_DISABLE_ENV_FILE=true`，可透传 `up`、`ps`、`logs`、`down` 等命令，既不加载 `.env`，也不依赖其中的值。也可在终端先设置此开关，再直接运行 `docker compose -f deploy/compose.dev.yaml ...`。

`init-dev-db.mjs` 仅运行 Compose 中声明的 `postgres-init` 服务，不读取应用配置。该服务幂等创建 `crypto_sso_demo` 和 `crypto_sso_test`，不清除已有数据；使用独立 `init` profile 避免普通 `up --wait` 把初始化任务当作长期运行服务。修改开发数据库设置时，更新 Compose 并同步应用 `.env` 中的连接字符串。`TEST_DATABASE_URL` 必须指向独立的、名称以 `_test` 结尾的数据库；集成测试会清空这个数据库内的业务记录。

## 启动

开发时各在一个终端运行：

```powershell
cargo run -p issuer-service --bin issuer-service
bun run --cwd issuer-service-webui dev
cargo run -p crypto-sso-relying-party
bun run --cwd examples/relying-party/web dev
bun run --cwd wallet desktop:dev
```

默认管理后台地址为 `http://127.0.0.1:5173/admin/login`，示例应用为 `http://127.0.0.1:5174`。两个 Rust 后端仅监听 API，分别为 `3000`、`3001`，不提供 HTML、前端资源或 SPA fallback。管理前端和示例应用的 Vite 开发服务器分别将 `/api` 反向代理到对应 Rust API。

代理保留浏览器的实际 Origin；`ADMIN_ALLOWED_ORIGINS_JSON` 明确配置管理前端地址。`RP_ORIGIN` 必须等于示例应用实际浏览器地址，开发配置为 `http://127.0.0.1:5174`，verifier 和钱包都固定这个 Origin。服务公开地址、前端浏览器地址和 API 代理目标分别通过环境变量配置。

新数据库默认关闭注册和签发。管理员需在服务设置中显式开启，并配置可覆盖的 Claims 建议。钱包中心地址填写 API 的公开基址，开发时为 `http://127.0.0.1:3000`。

注册中心与示例应用每次启动都会先连接各自数据库，通过 Toasty 自动应用待执行迁移，再启动 HTTP 服务和后台任务。首次启动自动建表；重复启动由 Toasty 跳过已应用版本。迁移失败时程序退出，不开始接收请求。数据库本身由 Compose 初始化，应用表结构由程序启动时的迁移负责。

生产时分别构建前端：

```powershell
bun run --cwd issuer-service-webui build
bun run --cwd examples/relying-party/web build
```

将构建产物交由独立 Nginx 或其他前端托管服务提供。`deploy/nginx.admin.conf.template` 给出了管理前端独立部署的配置：前端静态文件放到 `${FRONTEND_STATIC_ROOT}/admin/`，由前端网关将 `/api` 转发到 `${ISSUER_API_UPSTREAM}`。模板变量见 `.env.example`；生产域名应同时出现在 `ADMIN_ALLOWED_ORIGINS_JSON` 中。Rust 服务无需访问前端目录，部署包也无需携带前端构建产物。

示例应用前端同样独立托管，并把 `/api/v1/login` 代理到应用后端；生产 `RP_ORIGIN` 改为实际应用域名，钱包重新独立核对该 Origin。

钱包启动脚本读取同一环境文件，向 Tauri 注入开发地址。`bun run --cwd wallet desktop:build` 构建桌面包；部署运行时仍需从进程环境或 `.env` 提供钱包目录、网络限制和 SDK 限制。路径建议用绝对路径，初始化脚本会自动展开。

## 手工验收

1. 管理员登录，设置公开信息，启用注册和签发；可添加 `name`、`role=member` 等建议。
2. 钱包创建本地钱包，添加中心地址，通过独立渠道核对完整 issuer 指纹后信任。
3. 创建注册草稿，增删、修改建议或提交空集合；预览完整属性后提交。
4. 管理后台打开申请，根据可信资料填写内部成员标识、核验来源和说明，确认全部属性并批准。
5. 钱包同步后领取、解盲验签并加密保存，显示“凭证可用”。关闭后重新打开也能恢复。
6. 在钱包配置示例应用的实际 Origin 和固定 issuer；示例页面获取挑战，粘贴到钱包，确认披露，复制公开证明回同一浏览器提交。
7. 示例应用默认要求 `role=member`。钱包仅声明“证明已生成”；应用独立验真并建立会话后显示“登录成功”。已有凭证的此流程不需要中心在线。

## 配置与部署

完整变量名称见根 `.env.example`。数据库迁移在启动时自动执行，无需开关或预先运行迁移命令。实现遵循 [Toasty 0.11 Schema Management](https://tokio-rs.github.io/toasty/0.11.0/guide/schema-management.html)：用官方 `ToastyCli` 从注册模型生成 SQL、快照和 `history.toml`，通过 `toasty::embed_migrations!()` 编译进程序，启动时调用 `MigrationSet::apply`。已应用版本由 Toasty 的 `__toasty_migrations` 表维护，程序没有自建迁移执行器或迁移历史表。

模型定义集中在 `issuer-service/src/models.rs`，`store.rs` 只负责统一注册、连接和启动迁移。修改模型后，在仓库根运行：

```powershell
cargo run -p issuer-service --bin issuer-db -- migration generate --name describe_change
```

审阅并提交 `issuer-service/toasty/migrations/`、`snapshots/` 和 `history.toml`，然后重新构建、启动服务；待执行迁移会在启动时自动应用。根 `Toasty.toml` 仅配置开发工具的产物目录，数据库连接仍由环境变量提供。示例应用与中心共用同一迁移集，分别应用到各自数据库。多实例部署先启动一个实例完成迁移，再启动其余实例。

初始迁移由 Toasty 生成，创建语句保留 `IF NOT EXISTS`，用于接纳旧原型已创建的同构表并保留业务数据；之后的变更由模型快照差异生成。启动流程不使用 `push_schema`、`reset_db` 或数据库清空操作。

业务数据使用独立的 Toasty 类型化模型和表：`IssuerMetadata`、`Settings`、`ClaimHintRevision`、`Enrollment`、`Member`、`Idempotency`、`AuditRecord`、`WebSession`、`RevokedSession`、`StoredChallenge` 和 `ApplicationSession`。CRUD 直接采用框架生成的 API：`toasty::create!(Model { ... }).exec(&mut tx)` 创建，`Model::get_by_*(&mut tx, &key)` 读取，`model.update().field(value).exec(&mut tx)` 或生成的查询更新器更新，`model.delete()` 或查询删除器删除。需要可选结果时使用 `.filter_by_*().first().exec(...)`；框架会话保存确需 upsert 时使用原生 `upsert_by_id`。没有通用 JSON 记录表的 `get/put/list/remove` 封装或手写 CRUD SQL。

成员通过 `#[belongs_to]` 关联申请，详情使用 `member.enrollment().exec(&mut tx)` 加载。Claims、建议、申请公开状态及审批快照使用 Toasty 自带的 `Json<T>` JSONB 字段，编解码由 ORM 完成；密码学请求和响应保留原始严格解析后的文本。HTTP 视图仍显式选取公开字段，数据库模型不会整体返回管理前端。

按 [Toasty 0.11 Transactions](https://tokio-rs.github.io/toasty/0.11.0/guide/transactions.html)，用例通过 `Db::transaction_builder().isolation(IsolationLevel::Serializable).begin()` 开启写事务，关联 CRUD 均使用同一个事务 Executor，成功后调用 `commit()`。错误经 `?` 返回时由 Toasty 自动回滚。

`Serializable` 隔离保护状态/修订检查、成员名额、幂等记录、签发租约与结果，以及挑战消费和应用会话的原子性。PostgreSQL 拒绝并发序列化冲突时，业务接口返回 `409 REVISION_CONFLICT`；刷新后可用原幂等键重试整个用例，不单独重试某条语句。worker 领取租约遇到此冲突时跳过本轮，由下一次 tick 重试；结果提交失败后仍按持久化租约恢复。结果事务通过 Toasty ORM 写入已检查的同一设置行，防止旧快照中的“允许签发”在关闭操作已提交后继续提交结果。Salvo 会话适配器也用原生事务原子保存撤销标记和会话删除，阻止迟到请求恢复已撤销会话；框架接收并处理适配器错误。

管理列表使用 Toasty 原生只读 `RepeatableRead` 事务，保证一次响应中的多次查询使用同一快照。单条独立读取或写入直接调用 Toasty Executor。查询直接访问相应模型，详情按主键或模型关联加载，会话按 namespace 索引查询。列表排序、业务搜索和分页当前仍在内存完成；扩大数据量前应增加检索列、索引及数据库分页。密码学运算在数据库事务之外的阻塞工作线程执行。

`0001_remove_transaction_guard.sql` 移除旧全局锁表；`0002_typed_business_models.sql` 的表结构和快照由官方 CLI 生成，补充同一迁移中的一次性数据搬迁 SQL，将旧 JSON 记录转换到各类型化表。原始记录完整保留为 `identity_records_legacy`，包括未知扩展键；该归档不注册为运行时模型，也不参与 CRUD。迁移执行、失败回滚和历史仍全部由 Toasty 管理，没有自建迁移器。升级前停止旧版服务和 worker，再启动新版完成自动迁移，避免旧版继续向已归档的存储结构写入。

Cookie、会话签名、加载、轮换、有效期与退出由 Salvo `SessionHandler` 负责，Toasty 只实现框架的 `SessionStore` 持久化适配。管理路由使用 Salvo Session CSRF 中间件与来源校验，中间件保护挂在路由树对应分组上；各接口由独立 Handler 处理。

生产将 `COOKIE_SECURE=true`，使用 HTTPS 反向代理，准确设置公开 Origin 和允许的管理 Origin，使用独立随机会话密钥与 CSRF 密钥。固定 issuer 私钥应置于受限目录，与数据库备份、前端资源分别管理。更换启动配置中的 issuer 与数据库记录不符时，服务拒绝启动。Redis 仅由既有开发 Compose 提供，当前实现不依赖它。

日志统一使用 Rust `tracing`，三个应用及迁移 CLI 在读取 `.env` 后通过共享 `crypto-sso-telemetry` 初始化 subscriber，输出到标准错误。`RUST_LOG` 为必填环境变量，示例配置为 `warn,salvo_extra::logging=info,issuer_service=info,crypto_sso_relying_party=info,wallet_lib=info`；已有 `.env` 需补上此项。临时排查可把单个应用 target 改为 `debug` 或 `trace`，进程环境优先于 `.env`。Docker 基础设施仍仅使用 Compose 中的配置。

`info` 记录 API 访问、启动/停止、迁移完成及成功提交；`warn` 记录请求拒绝、租约恢复及钱包同步重试；`error` 记录数据库、worker 和加密存储失败；`debug` 记录预期状态冲突及租约竞争；`trace` 用于空闲轮询等细节。API 失败日志与响应共用 `trace_id`。应用 span 均跳过函数参数，事件不包含连接串、密码、Cookie、Claims、原始 SDK 消息、证明或钱包目录；除 Salvo 访问日志 target 外，第三方库保持 `warn`，避免打开其 SQL/HTTP 调试日志。数据库原始错误不直接写入日志。

两个 Rust API 服务均在 `Service` 层挂载 Salvo 原生 `logging::Logger`。每个请求完成后产生一条 `info` 访问日志，包含 HTTP 方法、路径、协议版本、远端地址、状态码和耗时；路径不含查询字符串，也不记录请求头或请求体。访问日志在权限、来源、限流和 CSRF 中间件之外执行，因此早期拒绝以及未匹配路由的 404/405 也能记录。业务告警和错误会附带当前请求 span 的方法、路径信息。

访问日志的 target 是 `salvo_extra::logging`，必须显式设置为 `info`，或者使用全局 `RUST_LOG=info`。进程环境中的过滤器优先于 `.env`；若进程已有 `RUST_LOG=warn`，仅修改 `.env` 不会提高日志级别。例如：

```powershell
$env:RUST_LOG='warn,salvo_extra::logging=info,issuer_service=info,crypto_sso_relying_party=info,wallet_lib=info'
cargo run -p issuer-service --bin issuer-service
```

访问日志示例（字段值以实际请求为准）：

```text
INFO Request{remote_addr=127.0.0.1:53000 version=HTTP/1.1 method=GET path=/api/v1/service-info}: salvo_extra::logging: Response status=200 OK duration=2ms
INFO Request{remote_addr=127.0.0.1:53000 version=HTTP/1.1 method=GET path=/api/v1/admin/settings}: salvo_extra::logging: Response status=401 Unauthorized duration=1ms
```

`cargo test -p issuer-service --test access_logs` 无需数据库，验证不同路径和方法的 200/401/403/404/405/503 访问记录、并发请求的 span、日志等级，以及查询参数、Bearer、Cookie、证明请求体不会进入访问日志。

钱包备份必须保留完整加密目录和可恢复的 Windows 用户环境。DPAPI 绑定当前系统用户，不提供跨系统迁移或明文秘密导出。SDK 当前没有撤销、凭证到期或保持原 subject 的补发机制。

## 检查

```powershell
# UI 检查使用隔离的公开展示数据，不修改运行中的中心或钱包。
cd scripts
bunx playwright install chromium
cd ..
./scripts/check-integration.ps1
```

若使用已安装的 Edge，可设置 `$env:PLAYWRIGHT_CHANNEL='msedge'` 后运行检查。UI 截图和报告保存在 `.dev-data/ui-check`。HTTP 和 IPC 的 TypeScript 类型分别从 Rust 生成，使用 `scripts/check-contracts.ps1` 检查同步。`issuer-service/tests/transactions.rs` 在随机隔离 schema 中验证提交可见性、错误自动回滚、显式回滚、嵌套保存点和并发写偏差冲突；迁移验收检查空库、旧原型及已应用旧迁移的数据库，逐一验证设置、申请、成员关联、审计、幂等记录、Salvo 会话、挑战和应用会话的数据保留，以及归档和重复启动。

已启动开发 API 与 Vite 前端时，可运行 `node scripts/check-proxy.mjs`，检查独立前端页面、API 转发、Origin、Salvo Session Cookie 和 CSRF。
