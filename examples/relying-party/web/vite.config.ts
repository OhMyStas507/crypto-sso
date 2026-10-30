import { defineConfig, loadEnv } from "vite";
import solid from "vite-plugin-solid";
export default defineConfig(({ mode, command }) => {
  const env = loadEnv(mode, "../../..", "");
  if (command === "serve") {
    for (const key of ["RP_DEV_HOST", "RP_DEV_PORT", "RP_API_TARGET"]) {
      if (!env[key]) throw new Error(`Missing environment variable ${key}`);
    }
  }
  return {
    plugins: [solid()],
    envDir: "../../..",
    server: {
      host: env.RP_DEV_HOST,
      port: Number(env.RP_DEV_PORT),
      strictPort: true,
      proxy: { "/api": { target: env.RP_API_TARGET, changeOrigin: false } },
    },
  };
});
