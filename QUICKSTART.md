## 快速启动开发环境：
开环环境数据库
``` sh
docker compose -f ./deploy/compose.dev.yaml up -d --wait
# 快速设置.env环境变量
bun ./scripts/setup-dev.js
```
启动服务
``` sh
cargo run --bin isser-service # 启动后端
bun --cwd ./issuer-service-webui/ install
bun --cwd ./issuer-service-webui/ dev # 启动web前端
```
启动钱包
``` sh
bun --cwd ./wallet/ install
bun --cwd ./wallet/ tauri dev
```
