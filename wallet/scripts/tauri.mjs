import { spawn } from "node:child_process";
import { environment, workspace } from "../../scripts/env.mjs";
import path from "node:path";
const env = environment();
for (const key of ["WALLET_DEV_HOST", "WALLET_DEV_PORT", "WALLET_DATA_DIR"])
  if (!env[key]) throw new Error(`Missing ${key}`);
const mode = process.argv[2] ?? "dev";
const config = {
  build: { devUrl: `http://${env.WALLET_DEV_HOST}:${env.WALLET_DEV_PORT}` },
  app: {
    security: {
      // Only the development WebView permits the explicitly configured Vite HMR connection.
      devCsp: `default-src 'self'; connect-src 'self' ipc: http://ipc.localhost ws://${env.WALLET_DEV_HOST}:${env.WALLET_DEV_PORT}; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost data:`,
    },
  },
};
const child = spawn(
  process.execPath,
  [
    path.join(workspace, "wallet/node_modules/@tauri-apps/cli/tauri.js"),
    mode,
    "--config",
    JSON.stringify(config),
    ...process.argv.slice(3),
  ],
  { cwd: path.join(workspace, "wallet"), env, stdio: "inherit" },
);
child.on("exit", (code) => process.exit(code ?? 1));
