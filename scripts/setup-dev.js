import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { workspace } from "./env.mjs";

function runCargo(args, failureMessage, env = process.env) {
  const result = spawnSync("cargo", ["run", ...args], {
    cwd: workspace,
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.error) throw result.error;
  if (result.status !== 0 || !result.stdout.trim()) {
    throw new Error(failureMessage);
  }
  return result.stdout.trim();
}

function main() {
  const { values } = parseArgs({
    options: {
      organization: {
        type: "string",
        short: "n",
        default: "本地开发身份中心",
      },
      help: { type: "boolean", short: "h" },
    },
  });
  if (values.help) {
    console.log("Usage: node scripts/setup-dev.js [--organization <name>]");
    console.log("Default organization: 本地开发身份中心");
    return;
  }

  const envPath = path.join(workspace, ".env");
  if (existsSync(envPath)) {
    throw new Error(".env already exists; preserve your current configuration.");
  }
  let content = readFileSync(path.join(workspace, ".env.example"), "utf8");
  mkdirSync(path.join(workspace, ".dev-data"), { recursive: true });

  if (!existsSync(path.join(workspace, ".dev-data/issuer-secret.json"))) {
    const publicJson = runCargo(
      [
        "-p",
        "utils",
        "--bin",
        "generate-issuer-key",
        "--",
        "--organization",
        values.organization,
        "--output",
        ".dev-data/issuer-secret.json",
      ],
      "Issuer key generation failed",
    );
    writeFileSync(path.join(workspace, ".dev-data/issuer-public.json"), publicJson);
  }

  const password = randomBytes(16).toString("hex");
  const passwordHash = runCargo(
    ["-p", "issuer-service", "--bin", "hash-password"],
    "Password hashing failed",
    { ...process.env, ADMIN_BOOTSTRAP_PASSWORD: password },
  );
  const replacements = {
    REPLACE_WITH_ARGON2_HASH: passwordHash,
    REPLACE_WITH_64_OR_MORE_RANDOM_BYTES: randomBytes(64).toString("hex"),
    REPLACE_WITH_64_HEX_CHARACTERS: randomBytes(32).toString("hex"),
    REPLACE_WITH_A_DIFFERENT_64_BYTE_SECRET: randomBytes(64).toString("hex"),
    REPLACE_WITH_A_DIFFERENT_64_HEX_KEY: randomBytes(32).toString("hex"),
  };
  for (const [placeholder, value] of Object.entries(replacements)) {
    content = content.replaceAll(placeholder, () => value);
  }
  for (const relative of [
    ".dev-data/issuer-secret.json",
    ".dev-data/issuer-public.json",
    ".dev-data/wallet",
  ]) {
    const absolute = path.join(workspace, relative).replaceAll("\\", "/");
    content = content.replaceAll(`=${relative}`, () => `='${absolute}'`);
  }

  // Exclusive creation also prevents overwrites if another setup finishes first.
  writeFileSync(envPath, content, { encoding: "utf8", flag: "wx", mode: 0o600 });
  console.log(`Created .env. Development admin: admin / ${password}`);
  console.log("Keep this generated password private. It is not saved in plaintext.");
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
