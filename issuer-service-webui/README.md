# 身份中心管理后台

Solid + Solid Router + Solid Query。界面遵循 `docs/gui-design` 的布局与状态语义，使用同源 `/api/v1/admin` 接口。Cookie、Session 与 CSRF 由 Salvo 后端中间件管理。

仓库根 `.env` 配置开发端口、代理目标和服务端允许的管理 Origin。执行 `bun install --cwd issuer-service-webui`，然后运行 `bun run --cwd issuer-service-webui dev`；生产使用 `build`，由独立 Nginx 或前端托管服务提供 `/admin`，并代理 API 到 Rust 后端。Rust 后端只提供接口，不读取前端构建产物。

路由、布局、会话门禁与 QueryClient 位于 `src/App.tsx`；页面位于 `src/pages`，申请列表和签发台账共用 `EnrollmentList.tsx`，服务设置和属性提示共用 `SettingsPage.tsx`。请求、错误及 CSRF 封装位于 `src/api.ts`，可重试写操作的幂等键位于 `src/features/useWrite.tsx`。共享 HTTP 类型来自 Rust 契约生成的本地包，管理端专用展示接口目前声明在 `api.ts`。

完整配置、初始化和运行说明见 [部署文档](../deploy/README.md)。

`App.tsx` 的 `CenterGate` 在普通管理员会话前检查中心初始化状态。空库进入 `pages/SetupPage.tsx`，使用独立 `/api/v1/setup`、Cookie 和 CSRF 上下文，完成部署验证、管理员、服务信息、签发身份及最终提交；`setup.css` 沿用现有设计规范。草稿只由服务端持久化，页面不会把密码、初始化码或私钥写入浏览器存储。初始化完成后仍需通过正常登录页建立管理会话。
