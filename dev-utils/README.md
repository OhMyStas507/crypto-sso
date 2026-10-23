# 开发工具

`dev-utils` 是一个包含多个 bin 的 Rust crate，由根目录 workspace 纳入。
每个工具放在 `dev-utils/src/bin/<工具名>.rs` 中，Cargo 会自动识别为独立的可执行目标，
所有工具共用 `dev-utils/Cargo.toml` 中的依赖。

## 拉取参考仓库

需要安装 Git。在仓库根目录执行：

```sh
cargo fetch-refs
```

该别名等价于 `cargo run -p dev-utils --bin fetch-references`。
工具读取根目录 `references.toml`，将仓库克隆到 `docs/references/<name>`。
配置初始为空；添加仓库时将 `repositories = []` 替换为以下条目，可重复添加：

```toml
[[repositories]]
name = "example"
url = "https://github.com/owner/example.git"
branch = "main" # 可选，不填则使用远端默认分支
```

`name` 只允许 ASCII 字母、数字、`-`、`_`、`.`，不能为 `.`、`..` 或以 `-` 开头；
各仓库名称不能重复（忽略大小写）。克隆保留完整历史。
目标路径已存在时跳过，不更新或覆盖本地内容；需要更新时在对应仓库中自行执行 Git 命令。
单个仓库克隆失败后仍会尝试其余仓库，最后汇总失败信息并返回非零退出码。
拉取内容已由 `docs/.gitignore` 排除。
