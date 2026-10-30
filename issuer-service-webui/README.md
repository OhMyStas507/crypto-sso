# 身份中心管理后台

Solid + Solid Router + Solid Query。界面遵循 `docs/gui-design` 的布局与状态语义，使用同源 `/api/v1/admin` 接口。Cookie、Session 与 CSRF 由 Salvo 后端中间件管理。

仓库根 `.env` 配置开发端口、代理目标和服务端允许的管理 Origin。执行 `bun install --cwd issuer-service-webui`，然后运行 `bun run --cwd issuer-service-webui dev`；生产使用 `build`，由独立 Nginx 或前端托管服务提供 `/admin`，并代理 API 到 Rust 后端。Rust 后端只提供接口，不读取前端构建产物。

页面位于 `src/pages`；请求、错误及 CSRF 封装位于 `src/api.ts`，可重试写操作的幂等键位于 `src/features/useWrite.tsx`。共享 HTTP 类型来自 Rust 契约生成的本地包。

完整配置、初始化和运行说明见 [部署文档](../deploy/README.md)。
