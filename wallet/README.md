# 身份钱包

Solid + Tauri 桌面应用。首期使用 Windows DPAPI 保护数据密钥，以 AES-256-GCM 加密申领、凭证、受信连接与草稿；SDK 和网络操作全部在 Rust 层完成。

在仓库根准备 `.env`，安装钱包依赖后执行 `bun run --cwd wallet desktop:dev`。单独运行 Vite 只能预览网页，无法访问桌面 IPC。完整准备与运行说明见 [部署文档](../deploy/README.md)。

- `src/pages`：中心连接、注册、领证、凭证和披露授权界面。
- `src/App.tsx`：HashRouter、布局、解锁门禁、QueryClient 和钱包事件订阅。
- `src/bridge`：集中 IPC 调用与从 Rust 生成的展示类型。
- `src-tauri/src/lib.rs`：环境初始化、单一 `wallet_command` 入口、后台申请同步与事件发送。
- `src-tauri/src/engine.rs`：用例和解锁代次检查；网络与密码学操作不持有本地写锁。
- `src-tauri/src/storage.rs`：DPAPI、认证加密、单写入者锁及原子提交。
- `src-tauri/src/model.rs`：加密文档内部模型；秘密字段不导出到前端类型。
- `src-tauri/src/dto.rs`：公开 IPC 展示类型，由 `examples/export_ipc.rs` 导出到 `src/bridge/generated.ts`。

`desktop:build` 从环境读取配置并构建桌面包。本期不提供凭证撤销、自动补发或跨系统迁移。
