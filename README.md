# Crypto-SSO
这是一个基于BBS+实现的单点登录系统。
它基于密码学算法完成对用户的校验，用户登录过程中应用程序后端不需要与登录中心通信。

## 系统中的几个角色
用户：真实世界中的用户，应用程序的访问者，持有自身钱包私钥
登录中心：持有信任源的私钥，为用户身份提供签名，
应用程序：用户访问的服务，持有登录中心的公钥，要求用户钱包通过零知识证明验证身份。

## 仓库文件夹划分
dev-utils/ 用于开发的工具
docs/ 文档和开发资料
issuer-service/ 登录中心后端实现
issuer-service-webui/ 登录中心前端实现
sdk/ 算法和验证机制实现
utils/ 命令行工具（如生成 issuer 私钥，见 [使用说明](utils/README.md)）
wallet/ 用户本地钱包

SDK 已按 core、issuer、holder、verifier 四个 crate 划分，支持带属性的 BBS+ 盲签发与选择披露。使用方式和检查命令见 [SDK README](sdk/README.md)，测试文档全部 89 个编号的覆盖入口见 [测试覆盖清单](sdk/TEST-COVERAGE.md)。

身份验证中心与钱包的后续业务设计见 [业务流程文档](docs/identity-center-wallet-business-flow.md)，包含服务设置、Claims 默认建议 API、钱包注册、人工审批与签发、凭证保存及应用登录续接；该文档描述待实现功能。

具体代码模块、前端页面与路由、HTTP/IPC 契约和目标目录见 [项目结构与模块设计](docs/identity-center-wallet-project-structure.md)，采用 PostgreSQL、Salvo、Toasty、Solid 与 Tauri。
