// Run after starting the issuer API and the separate Vite admin development server.
import assert from "node:assert/strict";
import { environment } from "./env.mjs";

const env = environment();
const frontend = `http://${env.ADMIN_DEV_HOST}:${env.ADMIN_DEV_PORT}`;
const backend = env.ISSUER_API_TARGET;
const page = await fetch(`${frontend}/admin/login`);
assert.equal(page.status, 200);
assert.match(page.headers.get("content-type") ?? "", /text\/html/);
assert.equal(
  (await fetch(`${backend}/admin/login`)).status,
  404,
  "Rust must not serve the frontend",
);
const direct = await fetch(`${backend}/api/v1/service-info`).then((r) =>
  r.json(),
);
const proxied = await fetch(`${frontend}/api/v1/service-info`).then((r) =>
  r.json(),
);
assert.equal(proxied.issuer_id, direct.issuer_id);
const bootstrap = await fetch(`${frontend}/api/v1/admin/csrf`);
assert.equal(bootstrap.status, 200);
const cookie = bootstrap.headers.get("set-cookie");
assert.match(cookie ?? "", /HttpOnly/i);
assert.match(cookie ?? "", /SameSite=Strict/i);
const csrf = await bootstrap.json();
const rejected = await fetch(`${frontend}/api/v1/admin/sessions`, {
  method: "POST",
  headers: {
    "content-type": "application/json",
    origin: frontend,
    cookie: cookie.split(";")[0],
    "x-csrf-token": csrf.csrf_token,
  },
  body: JSON.stringify({
    username: "invalid-proxy-check",
    password: "invalid-proxy-check",
  }),
});
assert.equal(
  rejected.status,
  401,
  "Vite must preserve the allowed frontend Origin and Salvo session/CSRF data",
);
assert.equal((await rejected.json()).code, "INVALID_CREDENTIALS");
console.log(
  "Vite proxy passed: independent frontend, API forwarding, Origin, Salvo session Cookie and CSRF.",
);
