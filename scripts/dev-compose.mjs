// Keep Docker's automatic .env loading disabled for every development Compose command.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const composeFile = fileURLToPath(
  new URL("../deploy/compose.dev.yaml", import.meta.url),
);

export function runCompose(args) {
  const result = spawnSync("docker", ["compose", "-f", composeFile, ...args], {
    env: { ...process.env, COMPOSE_DISABLE_ENV_FILE: "true" },
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  return result.status ?? 1;
}

if (import.meta.main) process.exit(runCompose(process.argv.slice(2)));
