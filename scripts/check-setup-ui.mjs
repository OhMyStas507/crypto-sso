// 浏览器使用真实 Salvo API 和独立测试库；仅静态资源由路由拦截提供，等效于前端网关。
import { chromium } from "playwright";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.join(root, "issuer-service-webui/dist");
const output = path.join(root, ".dev-data/ui-check");
const base = process.env.SETUP_TEST_URL,
  code = process.env.SETUP_TEST_CODE;
assert.ok(base && code, "由 initialization 集成测试提供临时 API 和初始化码");
await fs.mkdir(output, { recursive: true });
const browser = await chromium.launch({
  channel:
    process.env.PLAYWRIGHT_CHANNEL ||
    (process.platform === "win32" ? "msedge" : undefined),
  headless: true,
});
const errors = [];
const context = await browser.newContext({
  viewport: { width: 1440, height: 1000 },
});
async function frontend(context) {
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (
      url.origin !== base ||
      url.pathname.startsWith("/api/") ||
      url.pathname === "/health"
    )
      return route.continue();
    const asset = url.pathname.replace(/^\/admin\//, "");
    const target = path.resolve(
      dist,
      asset.startsWith("assets/") ? asset : "index.html",
    );
    assert.ok(target.startsWith(dist + path.sep));
    const mime = {
      ".html": "text/html; charset=utf-8",
      ".js": "text/javascript; charset=utf-8",
      ".css": "text/css; charset=utf-8",
      ".woff": "font/woff",
      ".woff2": "font/woff2",
      ".svg": "image/svg+xml",
    };
    await route.fulfill({
      body: await fs.readFile(target),
      contentType: mime[path.extname(target)] ?? "application/octet-stream",
    });
  });
}
await frontend(context);
const page = await context.newPage();
page.on("pageerror", (e) => errors.push(e.message));
try {
  await page.goto(`${base}/admin/overview`);
  await page
    .getByRole("heading", { name: "初始化身份中心", exact: true })
    .waitFor();
  await page.getByLabel("一次性初始化码").fill(code);
  await page.getByRole("button", { name: "验证并开始" }).click();
  await page
    .getByRole("heading", { name: "设置管理员账号", exact: true })
    .waitFor();
  await page.getByLabel("管理员账号", { exact: true }).fill("ui-admin");
  await page
    .getByLabel("登录密码", { exact: true })
    .fill("Browser test password 123");
  await page
    .getByLabel("确认密码", { exact: true })
    .fill("different password 123");
  await page.getByRole("button", { name: "保存并继续" }).click();
  await page
    .getByRole("alert")
    .filter({ hasText: "两次输入的密码不一致" })
    .waitFor();
  await page
    .getByLabel("确认密码", { exact: true })
    .fill("Browser test password 123");
  await page.getByRole("button", { name: "保存并继续" }).click();
  await page
    .getByRole("heading", { name: "配置服务信息", exact: true })
    .waitFor();
  await page.getByLabel("服务展示名", { exact: true }).fill("星河身份中心");
  await page
    .getByLabel("公开服务地址", { exact: true })
    .fill("https://identity.example.org");
  await page.getByLabel("服务说明（可选）").fill("组织成员身份凭证");
  await page.screenshot({ path: path.join(output, "setup-service.png") });
  await page.getByRole("button", { name: "保存并继续" }).click();
  await page
    .getByRole("heading", { name: "创建签发身份", exact: true })
    .waitFor();
  await page.getByLabel("组织身份字符串").fill("星河组织");
  await page.getByRole("button", { name: "生成签发密钥", exact: true }).click();
  await page.getByRole("button", { name: "使用此签发身份" }).waitFor();
  const fingerprint = await page.locator(".setup-kv dd .mono").innerText();
  assert.match(fingerprint, /^[a-f0-9]{64}$/);
  const downloadEvent = page.waitForEvent("download");
  await page.getByRole("button", { name: "下载公开身份" }).click();
  const downloaded = await downloadEvent;
  const publicIdentity = JSON.parse(
    await fs.readFile(await downloaded.path(), "utf8"),
  );
  assert.equal(publicIdentity.organization, "星河组织");
  assert.equal(publicIdentity.version, 1);
  assert.ok(publicIdentity.public_key);
  assert.equal(publicIdentity.secret_key, undefined);
  await page.reload();
  await page.getByRole("button", { name: "使用此签发身份" }).waitFor();
  assert.equal(
    await page.locator(".setup-kv dd .mono").innerText(),
    fingerprint,
  );
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    ),
    false,
  );
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.screenshot({ path: path.join(output, "setup-issuer.png") });
  await page.getByRole("button", { name: "使用此签发身份" }).click();
  await page
    .getByRole("heading", { name: "确认初始化配置", exact: true })
    .waitFor();
  await page.getByRole("checkbox").check();
  await page.screenshot({ path: path.join(output, "setup-review.png") });
  // 服务端提交成功后丢弃响应，验证回执查询而非重新初始化的恢复路径。
  await page.route(`${base}/api/v1/setup/complete`, async (route) => {
    const response = await route.fetch();
    assert.equal(response.status(), 200);
    await route.abort("failed");
  });
  await page.getByRole("button", { name: "完成初始化", exact: true }).click();
  await page.getByRole("button", { name: "查询本次结果" }).waitFor();
  await page.getByRole("button", { name: "查询本次结果" }).click();
  await page
    .getByRole("heading", { name: "身份中心已初始化", exact: true })
    .waitFor();
  await page.screenshot({ path: path.join(output, "setup-complete.png") });
  await page.reload();
  await page
    .getByRole("heading", { name: "身份中心已初始化", exact: true })
    .waitFor();
  assert.ok(page.url().endsWith("/admin/setup/complete"));
  await page.getByRole("link", { name: "前往管理员登录" }).click();
  await page
    .getByRole("heading", { name: "管理员登录", exact: true })
    .waitFor();
  await page.getByLabel("管理员账号", { exact: true }).fill("ui-admin");
  await page
    .getByLabel("密码", { exact: true })
    .fill("Browser test password 123");
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await page.getByRole("heading", { name: "概览", exact: true }).waitFor();
  const outsider = await browser.newContext();
  await frontend(outsider);
  const revisit = await outsider.newPage();
  await revisit.goto(`${base}/admin/setup`);
  await revisit
    .getByRole("heading", { name: "此身份中心已完成初始化", exact: true })
    .waitFor();
  assert.equal(await revisit.getByLabel("一次性初始化码").count(), 0);
  await outsider.close();
  assert.deepEqual(errors, []);
  console.log(
    "初始化浏览器验收通过：字段校验、服务端密钥、公开身份下载、刷新恢复、响应丢失恢复、管理员登录及入口关闭。",
  );
} catch (error) {
  await page.screenshot({ path: path.join(output, "setup-failure.png") });
  console.error(await page.locator("body").innerText());
  throw error;
} finally {
  await browser.close();
}
