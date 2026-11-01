import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { workspace } from "./env.mjs";

function main() {
  const { values } = parseArgs({
    options: {
      help: { type: "boolean", short: "h" },
    },
  });
  if (values.help) {
    console.log("Usage: node scripts/setup-dev.js");
    console.log("创建部署配置；管理员、服务信息和签发身份在 WebUI 初始化。");
    return;
  }

  const envPath = path.join(workspace, ".env");
  if (existsSync(envPath)) {
    throw new Error(
      ".env already exists; preserve your current configuration.",
    );
  }
  let content = readFileSync(path.join(workspace, ".env.example"), "utf8");
  mkdirSync(path.join(workspace, ".dev-data"), { recursive: true });

  const replacements = {
    REPLACE_WITH_A_DIFFERENT_64_BYTE_SECRET: randomBytes(64).toString("hex"),
    REPLACE_WITH_A_DIFFERENT_64_HEX_KEY: randomBytes(32).toString("hex"),
  };
  for (const [placeholder, value] of Object.entries(replacements)) {
    content = content.replaceAll(placeholder, () => value);
  }
  for (const relative of [
    ".dev-data/issuer-public.json",
    ".dev-data/wallet",
  ]) {
    const absolute = path.join(workspace, relative).replaceAll("\\", "/");
    content = content.replaceAll(`=${relative}`, () => `='${absolute}'`);
  }

  // 独占创建，防止另一个初始化进程先完成后被覆盖。
  writeFileSync(envPath, content, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  console.log(
    "已创建 .env。启动后端后，从后端日志领取一次性初始化码，在 /admin/setup 完成配置。",
  );
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
