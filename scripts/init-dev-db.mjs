// Database names and credentials are defined by the Compose postgres-init service.
import { runCompose } from "./dev-compose.mjs";

process.exit(runCompose(["run", "--rm", "postgres-init"]));
