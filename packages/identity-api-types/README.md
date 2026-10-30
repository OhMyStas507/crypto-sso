# 共享 HTTP 类型

Rust 契约位于 `crates/identity-api`。在仓库根运行 `cargo run -p crypto-sso-identity-api --example export_types` 生成 `src/generated.ts`，不要手工修改生成文件。

SDK 消息在 TypeScript 中保持 `unknown`；由 Rust 的严格 wire 解析器校验原始 JSON。普通 Unix 时间戳作为 JSON number 输出。钱包 IPC 类型另由 `cargo run -p wallet --example export_ipc` 生成。
