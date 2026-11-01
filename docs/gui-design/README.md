# Crypto SSO GUI 设计图

根据 [业务流程](../identity-center-wallet-business-flow.md) 与 [页面和路由设计](../identity-center-wallet-project-structure.md) 绘制，2026-10-30；[首次启动与 WebUI 初始化](../identity-center-bootstrap-design.md) 更新于 2026-11-01。

共 70 张独立 PNG：身份中心管理端 35 张（含首次初始化 16 张），钱包 35 张。覆盖当前文档中的 28 个页面路由与 6 个拟新增初始化路由，另含管理端 404 页面与关键交互状态。

## 查看

- [完整图集](index.html)：在浏览器中打开，搜索页面、查看原图或打开静态界面。
- [管理端总览](admin-overview.png)
- [钱包总览](wallet-overview.png)
- [首次初始化总览](setup-overview.png)
- [首次初始化预览](setup-preview.png)
- [管理端 PNG](png/admin/)
- [钱包 PNG](png/wallet/)
- [页面、路由与图片清单](manifest.json)
- [自动检查结果](qa-report.json)

每个页面均提供独立图片。总览是方便审阅的缩略图，不替代原图。

## 视觉规范

沿用 Linear 的简洁布局：固定侧栏与顶栏、面包屑、细边框、紧凑列表、轻量状态和清晰的主要操作。页面不增加装饰性副标题、渐变、插画或大面积品牌色块。

| 项目 | 规范 |
| --- | --- |
| 管理端画布 | 1440 × 1000，导出 2880 × 2000 PNG |
| 钱包画布 | 1280 × 900，导出 2560 × 1800 PNG |
| 主背景 / 侧栏 | `#ffffff` / `#f7f7f9` |
| 主文字 / 次文字 | `#25262c` / `#686872` |
| 分隔线 | `#e9e9ed`，1 px |
| 强调色 | `#655cc6`，用于主要按钮、当前页和少量状态 |
| 侧栏 / 顶栏 | 216 px / 54 px，两端保持一致 |
| 字号 | 页面标题 23 px，正文 12–13 px，辅助信息 10–11 px |
| 图标 | 项目已安装的 Bootstrap Icons 1.13.2 原始 SVG，通常 16 px |
| 按钮 / 圆角 | 31 px 高；输入框和按钮 5 px，面板 7 px |
| 字体 | Segoe UI、Microsoft YaHei UI 与系统无衬线字体 |

当前 Tauri 模板的 800 × 600 窗口设置未在本次改动中调整。钱包设计稿按 1280 × 900 的桌面窗口绘制。

## 页面覆盖

### 身份中心管理端

| 路由 | 基础设计图 | 额外状态 |
| --- | --- | --- |
| `/admin/login` | a01 | a18 登录错误 |
| `/admin/setup` | a20 部署验证 | a28 后端未就绪、a29 重新验证、a32 已初始化 |
| `/admin/setup/admin` | a21 首位管理员 | a34 密码不一致 |
| `/admin/setup/service` | a22 服务信息 | a33 草稿修订冲突 |
| `/admin/setup/issuer` | a23 创建签发身份 | a24 已生成待启用、a30 存储失败、a35 生成中 |
| `/admin/setup/review` | a25 最终确认 | a26 提交中、a31 结果未知 |
| `/admin/setup/complete` | a27 初始化完成 | — |
| `/admin/overview` | a02 | — |
| `/admin/enrollments` | a03 | — |
| `/admin/enrollments/:requestId` | a04 | a13 批准确认、a14 退回、a15 拒绝、a16 签发失败及重试 |
| `/admin/members` | a05 | — |
| `/admin/members/:memberRef` | a06 | — |
| `/admin/issuances` | a07 | — |
| `/admin/settings/service` | a08 | a17 设置修订冲突 |
| `/admin/settings/claims` | a09 | a19 钱包建议表单预览 |
| `/admin/settings/issuer` | a10 | — |
| `/admin/audit` | a11 | — |
| `/admin/*` | a12 站内 404 | — |

### 本地身份钱包

| 路由 | 基础设计图 | 额外状态 |
| --- | --- | --- |
| `/welcome` | w01 | — |
| `/unlock` | w02 | — |
| `/` | w03 | — |
| `/centers` | w04 | — |
| `/centers/add` | w05 | w18 独立核对指纹、w30 指纹不匹配 |
| `/centers/:centerId` | w06 | — |
| `/centers/:centerId/enroll?draft=:draftId` | w07 | w19 完整提交预览、w31 空属性集合 |
| `/enrollments` | w08 | — |
| `/enrollments/:enrollmentId` | w09 | w20 退回修改、w21 保存失败恢复、w22 领证完成、w23 离线、w32 提交结果未知 |
| `/credentials` | w10 | — |
| `/credentials/:credentialId` | w11 | w24 删除本地凭证确认 |
| `/apps` | w12 | — |
| `/apps/new` | w13 | — |
| `/apps/:appId/edit` | w14 | — |
| `/login/new?app=:appId` | w15 | — |
| `/login/:taskId` | w16 披露授权 | w25 缺少凭证、w26 等待领证、w27 获取新挑战、w28 生成中、w29 交付证明、w33 过期、w34 缺少必需属性、w35 拒绝披露 |
| `/settings` | w17 | — |

## 业务语义

- 初始化必须先验证部署权限；网页仅展示公开身份，私钥由服务端生成并安全保存。
- 最终事务提交前均为可恢复草稿；初始化完成、管理员登录和开放注册/签发分别展示。
- 已有中心不会重新开放向导；重启、超时和重复提交不会静默生成另一把签发密钥。
- 原始申请 Claims 只读，修改需要新草稿；后台不会直接改签申请。
- Claims 建议可删除、修改或扩展，空集合允许提交。
- 审批通过、中心签发、本地凭证可用、应用建立会话分别展示。
- 领证保存失败可使用原恢复材料继续，不提示重新注册。
- 离线时保留上次中心状态；提交结果未知时重试原申请。
- 应用 Origin 与中心指纹独立确认；指纹不符时禁止连接。
- 应用必需属性、额外披露及隐藏属性分开呈现，拒绝必需披露则中止。
- 首次领证后获取新挑战。钱包仅生成并交付公开证明，不宣称已完成应用登录。
- 凭证删除仅作用于本地，不表示撤销或解除中心成员签发名额。

## 源文件与重绘

`shared.js` 与 `design.css` 提供共享组件和视觉规范；`admin.js`、`setup.js`、`wallet.js` 定义页面；`screen.html` 与 `boot.js` 用于逐页渲染。图标从项目现有依赖复制到 `assets/icons/`，保留其 MIT LICENSE。查看图集不依赖网络。初始化稿的主要步骤按钮可链接到下一张静态稿，表单值和操作结果仍为演示数据，不执行生成或保存。

重绘使用本机 Node.js、Playwright、Edge headless 和 Sharp：

```powershell
node docs/gui-design/render.mjs
```

脚本默认使用当前 Codex 的本机依赖位置。换机器时可用 `GUI_NODE_MODULES` 指定包含 `playwright` 和 `sharp` 的 `node_modules` 目录。

脚本导出全部独立 PNG、总览、预览、图集与清单，并检查文档路由覆盖、JavaScript 错误、缺失图标、页面溢出和控件横向裁切。

本目录是静态 GUI 设计产物，未接入业务 API。名称、成员、指纹、时间及密码学消息内容均为演示数据；不同图片展示不同业务阶段。消息中的省略号仅为设计展示，不能作为 SDK wire 消息输入。
