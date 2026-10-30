import { defineConfig, loadEnv } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig(({ mode, command }) => {
  const env = loadEnv(mode, "..", "");
  if (command === "serve") {
    for (const key of [
      "ADMIN_DEV_HOST",
      "ADMIN_DEV_PORT",
      "ISSUER_API_TARGET",
    ]) {
      if (!env[key]) throw new Error(`Missing environment variable ${key}`);
    }
  }
  return {
    base: "/admin/",
    envDir: "..",
    plugins: [solid()],
    server: {
      host: env.ADMIN_DEV_HOST,
      port: Number(env.ADMIN_DEV_PORT),
      strictPort: true,
      proxy: { "/api": { target: env.ISSUER_API_TARGET, changeOrigin: false } },
    },
  };
});
