// Browser-only view tests use sanitized fixtures. Real persistence/crypto is covered by end_to_end.rs.
import { chromium } from "playwright";
import fs from "node:fs/promises";
import path from "node:path";
import http from "node:http";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.join(root, ".dev-data/ui-check");
await fs.mkdir(output, { recursive: true });
const now = Math.floor(Date.now() / 1000),
  fingerprint = "ab".repeat(32),
  requestId = "cd".repeat(32);
const settings = {
  revision: 3,
  claim_hints_revision: 2,
  display_name: "星河身份中心",
  description: "组织成员身份认证",
  contact: "support@example.org",
  public_base_url: "https://identity.example.org",
  registration_enabled: true,
  issuance_enabled: true,
  review_ttl_seconds: 604800,
  poll_interval_seconds: 5,
  updated_at: now,
  claim_hints: [
    {
      key: "name",
      label: "姓名",
      description: "真实姓名",
      default_value: "",
      example_value: "请输入姓名",
      include_by_default: true,
    },
    {
      key: "role",
      label: "成员角色",
      description: "需核验组织授权",
      default_value: "member",
      example_value: null,
      include_by_default: true,
    },
  ],
};
const info = {
  api_version: "v1",
  issuer_id: fingerprint,
  issuer_public: {
    version: 1,
    organization: "星河组织",
    public_key: "public-view-fixture",
  },
  settings,
  effective_claims_limits: {
    max_keys: 128,
    max_key_bytes: 64,
    max_value_bytes: 1024,
  },
};
const view = {
  request_id: requestId,
  request_hash: "ef".repeat(32),
  status: "PENDING_REVIEW",
  revision: 1,
  submitted_at: now,
  review_deadline: now + 604800,
  public_reason: null,
  poll_interval_seconds: 5,
};
const enrollment = {
  view,
  claims: { name: "林知远", role: "member" },
  application_note: "申请组织成员凭证",
  claim_hints_revision: 2,
  hints_snapshot: settings.claim_hints,
  decision: null,
  reviewer: null,
  reviewed_at: null,
  issued_at: null,
  failure_code: null,
  history: [],
};
const center = {
  id: "center-1",
  base_url: "https://identity.example.org",
  info,
};
const credential = {
  id: requestId,
  center_id: center.id,
  issuer_id: fingerprint,
  claims: enrollment.claims,
  saved_at: now,
};
const local = {
  id: requestId,
  center_id: center.id,
  claims: enrollment.claims,
  view,
  local_state: "WAITING_CENTER",
  error: null,
  synced_at: now,
  recovery_saved: true,
};
const app = {
  id: "app-1",
  name: "组织协作空间",
  origin: "https://app.example.org",
  issuer_id: fingerprint,
  revision: 1,
};
const task = {
  id: "task-1",
  app_id: app.id,
  app_revision: 1,
  revision: 1,
  state: "REVIEW_DISCLOSURE",
  challenge: "{}",
};
const plan = {
  task,
  app,
  state: "REVIEW_DISCLOSURE",
  expires_at: now + 120,
  required_claims: ["role"],
  missing_claims: [],
  credential,
  center_id: center.id,
};
const draft = {
  id: "draft-1",
  center_id: center.id,
  rows: [
    ["name", ""],
    ["role", "member"],
  ],
  claim_hints_revision: 2,
  application_note: "",
};
const member = {
  member_ref: "M-001",
  request_id: requestId,
  evidence_source: "可信组织名册",
  verification_note: "已线下核验",
  verified_at: now,
  issued: false,
};
const pageOf = (items) => ({ items, total: items.length, next_cursor: null });
const mime = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
};
const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, "http://localhost"),
      admin = url.pathname.startsWith("/admin");
    const directory = path.join(
      root,
      admin ? "issuer-service-webui/dist" : "wallet/dist",
    );
    const relative = decodeURIComponent(
      admin ? url.pathname.slice(6) : url.pathname,
    );
    let target = path.resolve(directory, "." + relative);
    if (!target.startsWith(directory + path.sep)) {
      target = path.join(directory, "index.html");
    }
    let bytes;
    try {
      bytes = await fs.readFile(target);
    } catch {
      target = path.join(directory, "index.html");
      bytes = await fs.readFile(target);
    }
    res
      .writeHead(200, {
        "content-type":
          mime[path.extname(target)] || "application/octet-stream",
      })
      .end(bytes);
  } catch {
    res.writeHead(500).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch(
  process.env.PLAYWRIGHT_CHANNEL
    ? { channel: process.env.PLAYWRIGHT_CHANNEL, headless: true }
    : { headless: true },
);
const errors = [];
const results = [];
let currentPage;
try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  });
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(e.message));
  await page.route("**/api/v1/**", (route) => {
    const url = new URL(route.request().url()),
      resource = url.pathname.replace("/api/v1/admin/", "");
    let value;
    if (url.pathname.endsWith("/service-info")) value = info;
    else if (resource === "session")
      value = {
        username: "admin@example.org",
        csrf_token: "test-csrf",
        expires_at: now + 3600,
      };
    else if (resource === "overview")
      value = {
        settings,
        issuer_id: fingerprint,
        key_available: true,
        pending: 1,
        failed: 0,
        issued: 0,
        recent: [enrollment],
      };
    else if (resource === "settings") value = settings;
    else if (resource.startsWith("enrollments/")) value = enrollment;
    else if (["enrollments", "issuances"].includes(resource))
      value = pageOf([enrollment]);
    else if (resource === "members") value = pageOf([member]);
    else if (resource.startsWith("members/")) value = { member, enrollment };
    else if (resource === "audit-events")
      value = pageOf([
        {
          id: "audit-1",
          at: now,
          actor: "admin",
          action: "ENROLLMENT_ACCEPTED",
          target: requestId,
        },
      ]);
    else value = {};
    return route.fulfill({ json: value });
  });
  async function check(route, title, name) {
    await page.goto(base + route);
    await page.getByRole("heading", { name: title, exact: true }).waitFor();
    await page.waitForLoadState("networkidle");
    if (errors.length) throw new Error(errors.join("\n"));
    assert.equal(
      await page.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      ),
      false,
      `${route}: horizontal overflow`,
    );
    results.push(route);
    if (name) await page.screenshot({ path: path.join(output, name + ".png") });
  }
  for (const [route, title, name] of [
    ["/admin/login", "管理员登录", "admin-login"],
    ["/admin/overview", "概览", "admin-overview"],
    ["/admin/enrollments", "注册申请"],
    [`/admin/enrollments/${requestId}`, "林知远", "admin-review"],
    ["/admin/members", "成员"],
    ["/admin/members/M-001", "M-001"],
    ["/admin/issuances", "签发台账"],
    ["/admin/settings/service", "服务设置", "admin-settings"],
    ["/admin/settings/claims", "Claims 默认建议"],
    ["/admin/settings/issuer", "签发身份"],
    ["/admin/audit", "审计记录"],
    ["/admin/missing", "页面不存在"],
  ])
    await check(route, title, name);
  await page.goto(base + "/admin/settings/claims");
  await page.getByLabel("建议键", { exact: true }).first().fill("custom.key");
  assert.equal(
    await page.getByLabel("建议键", { exact: true }).first().inputValue(),
    "custom.key",
  );
  await context.close();
  const walletContext = await browser.newContext({
    viewport: { width: 1280, height: 900 },
  });
  await walletContext.addInitScript(
    (fixtures) => {
      window.isTauri = true;
      const initialRoute = location.hash.slice(1);
      const walletStatus = {
        initialized: initialRoute !== "/welcome",
        unlocked: !["/welcome", "/unlock"].includes(initialRoute),
        epoch: 1,
        version: 1,
      };
      window.__TAURI_INTERNALS__ = {
        transformCallback: () => 1,
        unregisterCallback: () => {},
        invoke: async (name, payload) => {
          if (name !== "wallet_command") return 1;
          const { command, args } = payload;
          const route = location.hash.slice(1);
          if (command === "get_wallet_status") return { ...walletStatus };
          if (
            ["lock_wallet", "unlock_wallet", "initialize_wallet"].includes(
              command,
            )
          ) {
            walletStatus.initialized = true;
            walletStatus.unlocked = command !== "lock_wallet";
            walletStatus.epoch++;
            return { unlocked: walletStatus.unlocked };
          }
          const views = {
            list_centers: [fixtures.center],
            get_center: fixtures.center,
            list_credentials: [fixtures.credential],
            get_credential: fixtures.credential,
            list_enrollments: [fixtures.local],
            get_enrollment: fixtures.local,
            list_trusted_apps: [fixtures.app],
            list_login_tasks: [fixtures.task],
            get_disclosure_plan: fixtures.plan,
            get_enrollment_draft: fixtures.draft,
            create_enrollment_draft: fixtures.draft,
            get_preferences: { auto_lock_seconds: 300, version: 1 },
          };
          if (command === "save_enrollment_draft") {
            fixtures.draft = args;
            return args;
          }
          return views[command] ?? {};
        },
      };
    },
    { center, credential, local, app, task, plan, draft },
  );
  const walletPage = await walletContext.newPage();
  currentPage = walletPage;
  walletPage.on("pageerror", (e) => errors.push(e.message));
  for (const [route, title, name] of [
    ["/welcome", "创建本地身份钱包", "wallet-welcome"],
    ["/unlock", "解锁身份钱包"],
    ["/", "首页", "wallet-home"],
    ["/centers", "身份中心"],
    ["/centers/add", "添加身份中心"],
    ["/centers/center-1", "星河身份中心"],
    ["/centers/center-1/enroll?draft=draft-1", "申请注册", "wallet-draft"],
    ["/enrollments", "注册申请"],
    [`/enrollments/${requestId}`, "申请详情"],
    ["/credentials", "我的凭证"],
    [`/credentials/${requestId}`, "凭证详情"],
    ["/apps", "受信应用"],
    ["/apps/new", "添加受信应用"],
    ["/apps/app-1/edit", "编辑受信应用"],
    ["/login/new?app=app-1", "开始应用登录"],
    ["/login/task-1", "登录授权", "wallet-disclosure"],
    ["/settings", "本地设置"],
  ]) {
    await walletPage.goto(
      base + "/?screen=" + encodeURIComponent(route) + "#" + route,
    );
    await walletPage
      .getByRole("heading", { name: title, exact: true })
      .waitFor();
    await walletPage.waitForLoadState("networkidle");
    if (errors.length) throw new Error(errors.join("\n"));
    assert.equal(
      await walletPage.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      ),
      false,
      `${route}: horizontal overflow`,
    );
    results.push(route);
    if (name)
      await walletPage.screenshot({ path: path.join(output, name + ".png") });
  }
  await walletPage.goto(
    base + "/?screen=editor-check#/centers/center-1/enroll?draft=draft-1",
  );
  const nameInput = walletPage.getByRole("textbox", {
    name: "属性值 1",
    exact: true,
  });
  await nameInput.pressSequentially("Alice");
  assert.equal(await nameInput.inputValue(), "Alice");
  await walletPage.getByRole("button", { name: "预览并提交" }).click();
  await walletPage.getByRole("dialog").waitFor();
  assert.ok(
    await walletPage
      .getByRole("dialog")
      .getByText("Alice", { exact: true })
      .count(),
  );
  await walletPage
    .getByRole("button", { name: "返回编辑", exact: true })
    .click();
  await walletPage
    .getByRole("button", { name: "锁定钱包", exact: true })
    .click();
  await walletPage
    .getByRole("heading", { name: "解锁身份钱包", exact: true })
    .waitFor();
  await walletPage
    .getByRole("button", { name: "解锁钱包", exact: true })
    .click();
  await walletPage
    .getByRole("heading", { name: "首页", exact: true })
    .waitFor();
  await walletPage
    .getByRole("link", { name: /星河身份中心/ })
    .first()
    .waitFor();
  assert.deepEqual(errors, []);
  await fs.writeFile(
    path.join(output, "report.json"),
    JSON.stringify({ routes: results.length, errors, results }, null, 2),
  );
  console.log(
    `UI checks passed: ${results.length} routes, claim editor typing, submit preview, no script errors or horizontal overflow.`,
  );
} catch (error) {
  console.error(errors);
  if (currentPage) {
    console.error(await currentPage.locator("body").innerText());
    await currentPage.screenshot({ path: path.join(output, "failure.png") });
  }
  throw error;
} finally {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}
