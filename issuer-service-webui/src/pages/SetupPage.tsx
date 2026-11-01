import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createSignal,
  on,
  onCleanup,
  onMount,
  type JSX,
} from "solid-js";
import { A, useNavigate, useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import type {
  SetupDraft,
  SetupKeyView,
  SetupOperation,
  SetupReceipt,
  SetupService,
  SetupStatus,
} from "@crypto-sso/identity-api-types";
import { ApiFailure, operationKey, setupApi } from "../api";
import { Button, ErrorNotice, Field, Icon, Notice } from "../ui";
import "../setup.css";

const stages = ["verify", "admin", "service", "issuer", "review"];
const labels = ["部署验证", "管理员", "服务信息", "签发身份", "确认完成"];
const emptyService = (): SetupService => ({
  display_name: "",
  public_base_url: "",
  description: "",
  contact: "",
});
function Section(p: {
  title: string;
  children: JSX.Element;
  action?: JSX.Element;
}) {
  return (
    <section class="section">
      <h2 class="section-title">
        {p.title}
        <span class="meta">{p.action}</span>
      </h2>
      {p.children}
    </section>
  );
}
function Summary(p: { rows: [string, JSX.Element][] }) {
  return (
    <dl class="setup-kv">
      <For each={p.rows}>
        {([key, value]) => (
          <>
            <dt>{key}</dt>
            <dd>{value}</dd>
          </>
        )}
      </For>
    </dl>
  );
}

export function SetupPage() {
  const params = useParams();
  const navigate = useNavigate();
  const client = useQueryClient();
  const step = () => params.step ?? "verify";
  const [loading, setLoading] = createSignal(true),
    [closed, setClosed] = createSignal(false);
  const [available, setAvailable] = createSignal(false);
  const [draft, setDraft] = createSignal<SetupDraft>(),
    [receipt, setReceipt] = createSignal<SetupReceipt>();
  const [error, setError] = createSignal<unknown>(),
    [busy, setBusy] = createSignal(false),
    [unknown, setUnknown] = createSignal(false);
  const [code, setCode] = createSignal(""),
    [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal(""),
    [confirmation, setConfirmation] = createSignal(""),
    [visible, setVisible] = createSignal(false);
  const [service, setService] = createSignal<SetupService>(emptyService()),
    [organization, setOrganization] = createSignal("");
  const [confirmed, setConfirmed] = createSignal(false),
    [copied, setCopied] = createSignal(false);
  const [operation, setOperation] = createSignal<SetupOperation>();
  let adminKey = "",
    serviceKey = "",
    generationKey = "",
    completionKey = "";
  const go = (value: string) =>
    navigate(value === "verify" ? "/setup" : `/setup/${value}`);
  const index = () =>
    receipt() || closed() ? 5 : Math.max(0, stages.indexOf(step()));
  const title = () =>
    receipt()
      ? "身份中心已初始化"
      : closed()
        ? "此身份中心已完成初始化"
        : loading()
          ? "正在检查身份中心"
          : !available()
            ? "暂时无法连接身份中心"
            : ({
                verify: "初始化身份中心",
                admin: "设置管理员账号",
                service: "配置服务信息",
                issuer:
                  draft()?.key?.status === "READY"
                    ? "确认签发身份"
                    : "创建签发身份",
                review: unknown() ? "正在核对初始化结果" : "确认初始化配置",
              }[step()] ?? "初始化身份中心");
  function install(value: SetupDraft) {
    setDraft(value);
    setUsername(value.username);
    setService(value.service ?? emptyService());
    if (value.key) setOrganization(value.key.organization);
  }
  function resume(value: SetupDraft, keepRoute = true) {
    const minimum = !value.password_configured
      ? "admin"
      : !value.service
        ? "service"
        : !value.key || value.key.status !== "READY"
          ? "issuer"
          : "review";
    if (
      !keepRoute ||
      step() === "verify" ||
      step() === "complete" ||
      !stages.includes(step()) ||
      stages.indexOf(step()) > stages.indexOf(minimum)
    )
      go(minimum);
  }
  async function checkReceipt() {
    try {
      const op = await setupApi<SetupOperation>("operations/current");
      setOperation(op);
      if (op.completed && op.result) {
        setReceipt(op.result);
        setDraft(undefined);
        setUnknown(false);
        go("complete");
        await client.invalidateQueries({ queryKey: ["setup-status"] });
        return true;
      }
    } catch (e) {
      if (!(e instanceof ApiFailure) || e.status !== 404) throw e;
    }
    return false;
  }
  async function load() {
    setLoading(true);
    setAvailable(false);
    setError(undefined);
    try {
      await setupApi("csrf");
      if (await checkReceipt()) {
        setAvailable(true);
        return;
      }
      const status = await setupApi<SetupStatus>("status");
      setAvailable(true);
      if (status.state === "initialized") {
        setClosed(true);
        return;
      }
      try {
        const value = await setupApi<SetupDraft>("draft");
        install(value);
        resume(value);
      } catch (e) {
        if (e instanceof ApiFailure && e.status === 401) {
          setDraft(undefined);
          go("verify");
        } else throw e;
      }
    } catch (e) {
      setError(e);
    } finally {
      setLoading(false);
    }
  }
  onMount(() => void load());
  createEffect(
    on(
      step,
      () => {
        setPassword("");
        setConfirmation("");
        setVisible(false);
        setConfirmed(false);
        setError(undefined);
        const value = draft();
        if (value && !receipt() && !closed()) resume(value);
      },
      { defer: true },
    ),
  );
  onCleanup(() => {
    setPassword("");
    setConfirmation("");
    setCode("");
  });
  async function action(work: () => Promise<void>) {
    if (busy()) return;
    setBusy(true);
    setError(undefined);
    try {
      await work();
    } catch (e) {
      setError(e);
      if (e instanceof ApiFailure && e.code === "ALREADY_INITIALIZED") {
        try {
          if (!(await checkReceipt())) setClosed(true);
        } catch (lookupError) {
          setError(lookupError);
        }
      }
      if (e instanceof ApiFailure && e.status === 401) {
        setDraft(undefined);
        setPassword("");
        setConfirmation("");
        go("verify");
        setError(e);
      }
    } finally {
      setBusy(false);
    }
  }
  const verify = (e: SubmitEvent) => {
    e.preventDefault();
    void action(async () => {
      await setupApi("csrf");
      const value = await setupApi<SetupDraft>("session", "POST", {
        code: code(),
      });
      setCode("");
      await setupApi("csrf");
      install(value);
      resume(value, false);
    });
  };
  const saveAdmin = (e: SubmitEvent) => {
    e.preventDefault();
    void action(async () => {
      if (password() !== confirmation()) {
        setError(new Error("两次输入的密码不一致，请重新确认。"));
        document.getElementById("setup-password-confirm")?.focus();
        return;
      }
      if (password() && [...password()].length < 12)
        throw new Error("密码须为 12–128 个字符。");
      adminKey ||= operationKey();
      const value = await setupApi<SetupDraft>(
        "draft/admin",
        "PUT",
        {
          expected_revision: draft()!.revision,
          username: username(),
          password: password() || null,
        },
        adminKey,
      );
      install(value);
      adminKey = "";
      setPassword("");
      setConfirmation("");
      go("service");
    });
  };
  const saveService = (e: SubmitEvent) => {
    e.preventDefault();
    void action(async () => {
      serviceKey ||= operationKey();
      install(
        await setupApi<SetupDraft>(
          "draft/service",
          "PUT",
          { expected_revision: draft()!.revision, service: service() },
          serviceKey,
        ),
      );
      serviceKey = "";
      go("issuer");
    });
  };
  async function refreshDraft() {
    const value = await setupApi<SetupDraft>("draft");
    install(value);
    return value;
  }
  async function generate() {
    await action(async () => {
      generationKey =
        draft()?.key?.operation_id || generationKey || operationKey();
      try {
        await setupApi<SetupKeyView>(
          "key-generation",
          "POST",
          {
            expected_revision: draft()!.revision,
            organization: organization(),
          },
          generationKey,
        );
      } finally {
        await refreshDraft();
      }
    });
  }
  async function submit() {
    await action(async () => {
      if (!confirmed()) return;
      completionKey =
        operation()?.operation_id || completionKey || operationKey();
      if (!operation())
        setOperation(
          await setupApi<SetupOperation>(
            "operations",
            "POST",
            undefined,
            completionKey,
          ),
        );
      const value = draft()!;
      try {
        const result = await setupApi<SetupReceipt>(
          "complete",
          "POST",
          {
            operation_id: completionKey,
            draft_id: value.draft_id,
            expected_revision: value.revision,
            key_id: value.key!.operation_id,
            identity_confirmed: true,
          },
          completionKey,
        );
        setReceipt(result);
        setDraft(undefined);
        setUnknown(false);
        go("complete");
        await client.invalidateQueries({ queryKey: ["setup-status"] });
      } catch (e) {
        if (!(e instanceof ApiFailure) || e.status >= 500) {
          setUnknown(true);
          setError(
            new Error("连接中断，尚未确认初始化是否完成。请先查询本次结果。"),
          );
        } else {
          if (e.status === 404) {
            setOperation(undefined);
            completionKey = "";
          }
          throw e;
        }
      }
    });
  }
  async function queryResult() {
    await action(async () => {
      if (await checkReceipt()) return;
      const status = await setupApi<SetupStatus>("status");
      if (status.state === "initialized") {
        setClosed(true);
        setUnknown(false);
        return;
      }
      await refreshDraft();
      setUnknown(false);
      setError(new Error("本次操作尚未完成，配置和密钥已保留，可继续原提交。"));
    });
  }
  const updateService = (key: keyof SetupService, value: string) => {
    serviceKey = "";
    setService((s) => ({ ...s, [key]: value }));
  };
  const back = (to: string) => (
    <Button disabled={busy()} onClick={() => go(to)}>
      上一步
    </Button>
  );
  const footer = (previous: string, label: string, disabled = false) => (
    <div class="form-footer">
      {back(previous)}
      <Button primary type="submit" disabled={busy() || disabled}>
        {busy() ? "正在保存…" : label}
      </Button>
    </div>
  );
  const keySummary = () => (
    <Summary
      rows={[
        ["组织身份", draft()?.key?.organization],
        ["算法 / 协议", "BBS+ / v1"],
        [
          "密钥状态",
          <span class="badge success">
            <Icon name="key" />
            已生成 · 待启用
          </span>,
        ],
        [
          "完整 issuer 指纹",
          <span class="mono">{draft()?.key?.issuer_id}</span>,
        ],
      ]}
    />
  );
  const download = () => {
    const blob = new Blob(
      [JSON.stringify(draft()?.key?.issuer_public, null, 2)],
      { type: "application/json" },
    );
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "issuer-public.json";
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  return (
    <div class="auth setup">
      <div class="auth-brand">
        <span class="brandmark">
          <Icon name="intersect" />
        </span>
        身份中心<span class="setup-label">首次初始化</span>
      </div>
      <main class="setup-main" aria-busy={busy() || loading()}>
        <nav class="setup-progress" aria-label="初始化步骤">
          <For each={labels}>
            {(label, i) => (
              <>
                <span
                  class={index() === i() ? "active" : ""}
                  aria-current={index() === i() ? "step" : undefined}
                >
                  <i class="step-num">
                    {index() > i() ? <Icon name="check" /> : i() + 1}
                  </i>
                  {label}
                </span>
                <Show when={i() < 4}>
                  <i class="connector" />
                </Show>
              </>
            )}
          </For>
        </nav>
        <div class="setup-heading">
          <h1>{title()}</h1>
          <span class="badge">
            {receipt() || closed()
              ? "初始化已完成"
              : draft()
                ? "初始化草稿"
                : "首次启动"}
          </span>
        </div>
        <div class="setup-columns">
          <div class="setup-form">
            <ErrorNotice error={error()} />
            <Show
              when={
                error() instanceof ApiFailure &&
                ["SETUP_REVISION_CONFLICT", "REVISION_CONFLICT"].includes(
                  (error() as ApiFailure).code,
                )
              }
            >
              <Notice>
                当前输入已保留；记录后可加载最新草稿再合并。
                <Button
                  onClick={() =>
                    void action(async () => {
                      await refreshDraft();
                      adminKey = "";
                      serviceKey = "";
                    })
                  }
                >
                  加载最新草稿
                </Button>
              </Notice>
            </Show>
            <Switch>
              <Match when={loading()}>
                <p role="status">正在检查后端就绪状态…</p>
              </Match>
              <Match when={receipt()}>
                {(_) => (
                  <>
                    <Notice>
                      管理员、服务配置与签发身份已保存。初始化入口已关闭。
                    </Notice>
                    <Section title="初始化结果">
                      <Summary
                        rows={[
                          ["管理员账号", receipt()!.username],
                          ["服务地址", receipt()!.service.public_base_url],
                          ["组织身份", receipt()!.organization],
                          [
                            "完整 issuer 指纹",
                            <span class="mono">{receipt()!.issuer_id}</span>,
                          ],
                          ["接受新注册", "关闭"],
                          ["凭证签发", "关闭"],
                        ]}
                      />
                    </Section>
                    <Section title="接下来">
                      <ol class="setup-next">
                        <li>使用刚设置的管理员账号登录。</li>
                        <li>在服务设置中确认 Claims 建议，开放注册和签发。</li>
                        <li>完成密钥备份，通过可信渠道分发地址与指纹。</li>
                      </ol>
                    </Section>
                    <A class="btn primary" href="/login">
                      前往管理员登录
                      <Icon name="arrow-right" />
                    </A>
                  </>
                )}
              </Match>
              <Match when={closed()}>
                <Notice>初始化已经完成，现有管理员与签发身份保持有效。</Notice>
                <p>
                  请使用已创建的管理员账号登录；若账号不可用，请联系部署维护者。
                </p>
                <A class="btn primary" href="/login">
                  前往管理员登录
                </A>
              </Match>
              <Match when={unknown()}>
                <Notice>提交结果未知，请保留当前页面并查询原操作。</Notice>
                <Summary
                  rows={[
                    [
                      "操作编号",
                      <span class="mono">{operation()?.operation_id}</span>,
                    ],
                    ["操作状态", "待核对"],
                  ]}
                />
                <Button
                  primary
                  disabled={busy()}
                  onClick={() => void queryResult()}
                >
                  查询本次结果
                </Button>
              </Match>
              <Match when={!draft()}>
                <Show
                  when={available()}
                  fallback={
                    <>
                      <p>
                        首次启动会自动迁移数据库。当前无法连接后端，迁移结果未知。
                      </p>
                      <Button onClick={() => void load()}>重新检查</Button>
                    </>
                  }
                >
                  <Notice>
                    数据库结构已就绪。验证部署权限后，开始初始化身份中心。
                  </Notice>
                  <form onSubmit={verify}>
                    <Section title="验证部署权限">
                      <Field
                        label="一次性初始化码"
                        hint="从部署主机的受保护文件获取，有效期 30 分钟。"
                      >
                        <input
                          class="input"
                          type="password"
                          autocomplete="off"
                          aria-label="一次性初始化码"
                          required
                          value={code()}
                          onInput={(e) => setCode(e.currentTarget.value)}
                        />
                      </Field>
                      <p class="hint">
                        若初始化码已使用或过期，请部署维护者重新签发。系统不会创建默认管理员或默认密码。
                      </p>
                    </Section>
                    <Button primary type="submit" disabled={busy()}>
                      {busy() ? "正在验证…" : "验证并开始"}
                    </Button>
                  </form>
                </Show>
              </Match>
              <Match when={step() === "admin"}>
                <form onSubmit={saveAdmin}>
                  <Section title="首位管理员">
                    <Field
                      label="管理员账号"
                      hint="独立管理账号，不会创建钱包成员。"
                    >
                      <input
                        class="input"
                        required
                        autocomplete="username"
                        aria-label="管理员账号"
                        minlength={3}
                        maxlength={128}
                        value={username()}
                        onInput={(e) => {
                          adminKey = "";
                          setUsername(e.currentTarget.value);
                        }}
                      />
                    </Field>
                    <Show when={draft()?.password_configured}>
                      <p class="hint">
                        密码已设置。保留密码框为空可继续使用原密码。
                      </p>
                    </Show>
                    <Field
                      label="登录密码"
                      hint="12–128 个字符；建议使用密码管理器生成。"
                    >
                      <div class="setup-password">
                        <input
                          class="input"
                          type={visible() ? "text" : "password"}
                          required={!draft()?.password_configured}
                          autocomplete="new-password"
                          aria-label="登录密码"
                          maxlength={256}
                          value={password()}
                          onInput={(e) => {
                            adminKey = "";
                            setPassword(e.currentTarget.value);
                          }}
                        />
                        <button
                          class="icon-button"
                          type="button"
                          aria-label={visible() ? "隐藏密码" : "显示密码"}
                          onClick={() => setVisible((v) => !v)}
                        >
                          <Icon name={visible() ? "eye" : "eye-slash"} />
                        </button>
                      </div>
                    </Field>
                    <Field label="确认密码">
                      <input
                        id="setup-password-confirm"
                        aria-label="确认密码"
                        class="input"
                        type={visible() ? "text" : "password"}
                        required={!!password()}
                        autocomplete="new-password"
                        value={confirmation()}
                        onInput={(e) => setConfirmation(e.currentTarget.value)}
                      />
                    </Field>
                  </Section>
                  <p class="hint">
                    <Icon name="lock" />
                    密码仅在服务端保存为 Argon2id
                    哈希，修改已设置密码时需重新输入。
                  </p>
                  {footer("verify", "保存并继续")}
                </form>
              </Match>
              <Match when={step() === "service"}>
                <form onSubmit={saveService}>
                  <Section title="公开服务信息">
                    <For
                      each={
                        [
                          ["display_name", "服务展示名"],
                          ["public_base_url", "公开服务地址"],
                          ["description", "服务说明（可选）"],
                          ["contact", "联系地址（可选）"],
                        ] as const
                      }
                    >
                      {([key, label]) => (
                        <Field
                          label={label}
                          hint={
                            key === "public_base_url"
                              ? "HTTPS 根地址；本地开发允许 loopback HTTP。"
                              : undefined
                          }
                        >
                          <input
                            class="input"
                            aria-label={label}
                            required={
                              key === "display_name" ||
                              key === "public_base_url"
                            }
                            value={service()[key]}
                            onInput={(e) =>
                              updateService(key, e.currentTarget.value)
                            }
                          />
                        </Field>
                      )}
                    </For>
                  </Section>
                  <Section title="初始业务策略">
                    <div class="pref-row">
                      接受新注册<span class="muted">关闭 · 登录后开放</span>
                    </div>
                    <div class="pref-row">
                      凭证签发<span class="muted">关闭 · 登录后开启</span>
                    </div>
                    <p class="hint">
                      Claims 默认建议为空；待审有效期 7 天，建议轮询间隔 5 秒。
                    </p>
                  </Section>
                  {footer("admin", "保存并继续")}
                </form>
              </Match>
              <Match when={step() === "issuer"}>
                <Show
                  when={draft()?.key?.status === "READY"}
                  fallback={
                    <>
                      <Section title="组织身份">
                        <Field
                          label="组织身份字符串"
                          hint="将参与 issuer 指纹；初始化后只读。"
                        >
                          <input
                            class="input"
                            value={organization()}
                            aria-label="组织身份字符串"
                            disabled={!!draft()?.key || busy()}
                            onInput={(e) =>
                              setOrganization(e.currentTarget.value)
                            }
                          />
                        </Field>
                      </Section>
                      <Section title="密钥来源">
                        <div class="setup-choice">
                          <Icon name="key" />
                          <div>
                            <strong>在服务端创建新密钥</strong>
                            <p class="hint">
                              使用安全随机源生成，并写入受保护密钥存储。
                            </p>
                          </div>
                        </div>
                        <p class="hint">
                          私钥不会返回浏览器；刷新后可恢复同一次生成操作。
                        </p>
                        <Show when={draft()?.key}>
                          <Notice>
                            {draft()?.key?.status === "FAILED"
                              ? "上次密钥保存失败，请检查服务端存储后重试原操作。"
                              : "密钥生成操作正在处理中，可查询或继续原操作。"}
                          </Notice>
                          <Button
                            disabled={busy()}
                            onClick={() =>
                              void action(async () => {
                                await refreshDraft();
                              })
                            }
                          >
                            查询生成结果
                          </Button>
                        </Show>
                      </Section>
                      <div class="form-footer">
                        {back("service")}
                        <Button
                          primary
                          disabled={busy() || !organization()}
                          onClick={() => void generate()}
                        >
                          {busy()
                            ? "正在生成并保存…"
                            : draft()?.key
                              ? "继续原生成操作"
                              : "生成签发密钥"}
                        </Button>
                      </div>
                    </>
                  }
                >
                  <Notice>
                    签发密钥已安全保存，尚未启用。刷新页面可恢复同一份公开身份。
                  </Notice>
                  <Section title="签发身份">
                    {keySummary()}
                    <div class="actions">
                      <Button
                        onClick={() =>
                          void action(async () => {
                            await navigator.clipboard.writeText(
                              draft()!.key!.issuer_id!,
                            );
                            setCopied(true);
                          })
                        }
                      >
                        {copied() ? "已复制" : "复制指纹"}
                      </Button>
                      <Button onClick={download}>下载公开身份</Button>
                    </div>
                  </Section>
                  <p class="hint">
                    请通过可信渠道分发完整指纹。运维应备份受保护密钥存储及其解密材料。
                  </p>
                  <div class="form-footer">
                    {back("service")}
                    <Button
                      primary
                      disabled={busy()}
                      onClick={() => go("review")}
                    >
                      使用此签发身份
                    </Button>
                  </div>
                </Show>
              </Match>
              <Match when={step() === "review"}>
                <Section
                  title="管理员"
                  action={
                    <Button disabled={busy()} onClick={() => go("admin")}>
                      修改
                    </Button>
                  }
                >
                  <Summary
                    rows={[
                      ["管理员账号", draft()?.username],
                      ["登录密码", "已设置 · 不回显"],
                    ]}
                  />
                </Section>
                <Section
                  title="服务信息"
                  action={
                    <Button disabled={busy()} onClick={() => go("service")}>
                      修改
                    </Button>
                  }
                >
                  <Summary
                    rows={[
                      ["展示名", draft()?.service?.display_name],
                      ["公开服务地址", draft()?.service?.public_base_url],
                      ["服务说明", draft()?.service?.description || "—"],
                      ["联系地址", draft()?.service?.contact || "—"],
                    ]}
                  />
                </Section>
                <Section title="签发身份">{keySummary()}</Section>
                <label class="check-label">
                  <input
                    type="checkbox"
                    checked={confirmed()}
                    onChange={(e) => setConfirmed(e.currentTarget.checked)}
                    disabled={busy()}
                  />
                  已核对组织身份与指纹，确认以此身份创建中心。
                </label>
                <p class="hint">
                  提交后创建管理员并固定签发身份。注册和签发保持关闭。
                </p>
                <div class="form-footer">
                  {back("issuer")}
                  <Button
                    primary
                    disabled={busy() || !confirmed()}
                    onClick={() => void submit()}
                  >
                    {busy() ? "正在完成初始化…" : "完成初始化"}
                  </Button>
                </div>
              </Match>
            </Switch>
          </div>
          <aside class="setup-aside">
            <Section title={receipt() || closed() ? "中心状态" : "本次初始化"}>
              <Summary
                rows={
                  closed()
                    ? [
                        ["初始化", "已完成"],
                        ["初始化入口", "已关闭"],
                        ["管理会话", "待登录"],
                      ]
                    : [
                        ["数据库结构", available() ? "已就绪" : "状态未知"],
                        [
                          "部署权限",
                          draft()
                            ? "已验证"
                            : receipt()
                              ? "初始化已完成"
                              : "等待验证",
                        ],
                        ["接受新注册", "关闭"],
                        ["凭证签发", "关闭"],
                      ]
                }
              />
            </Section>
            <p class="hint">
              完成后使用新建的管理员账号登录，再开放注册和签发。
            </p>
            <Show when={draft() && !busy()}>
              <Button
                onClick={() =>
                  void action(async () => {
                    await setupApi("session", "DELETE");
                    setDraft(undefined);
                    setCode("");
                    go("verify");
                  })
                }
              >
                退出初始化
              </Button>
            </Show>
          </aside>
        </div>
      </main>
      <footer class="auth-foot">
        <span>Crypto SSO · 身份中心</span>
        <span>管理控制台</span>
      </footer>
    </div>
  );
}
