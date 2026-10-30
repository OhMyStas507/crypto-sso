import { readFileSync } from "node:fs";
import { parseEnv } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const workspace = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
export function environment() {
  let file = {};
  try {
    file = parseEnv(readFileSync(path.join(workspace, ".env"), "utf8"));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  // Process variables always take priority over the development file.
  return { ...file, ...process.env };
}
