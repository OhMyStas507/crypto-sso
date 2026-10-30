import { defineConfig, loadEnv } from "vite";
import solid from "vite-plugin-solid";
// @ts-expect-error Node types are provided by Vite's build environment.
import process from "node:process";
export default defineConfig(({ mode, command }) => {
  const env = loadEnv(mode, "..", "");
  if (command === "serve") {
    for (const key of ["WALLET_DEV_HOST", "WALLET_DEV_PORT"]) {
      if (!env[key]) throw new Error(`Missing environment variable ${key}`);
    }
  }
  const host = process.env.TAURI_DEV_HOST || env.WALLET_DEV_HOST;
  return {
    plugins: [solid()],
    envDir: "..",
    clearScreen: false,
    server: {
      host,
      port: Number(env.WALLET_DEV_PORT),
      strictPort: true,
      watch: { ignored: ["**/src-tauri/**"] },
    },
  };
});
