import { For, Show, createEffect, createSignal } from "solid-js";
import { A, useNavigate } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import {
  command,
  type Center,
  type Credential,
  type LocalEnrollment,
  type LoginTask,
} from "../bridge";
import { useAction } from "../actions";
import {
  Button,
  Empty,
  ErrorNotice,
  Field,
  Head,
  Icon,
  Notice,
  Status,
  short,
} from "../ui";

function VaultPage(p: { initialize: boolean }) {
  const action = useAction(),
    navigate = useNavigate();
  const open = async () => {
    if (await action.run(p.initialize ? "initialize_wallet" : "unlock_wallet"))
      navigate("/");
  };
  return (
    <div class="auth wallet">
      <div class="auth-brand">
        <Icon name="shield-lock" />
        身份钱包
      </div>
      <div class="auth-main">
        <div class="hero-icon">
          <Icon name={p.initialize ? "shield-check" : "lock"} />
        </div>
        <h1>{p.initialize ? "创建本地身份钱包" : "解锁身份钱包"}</h1>
        <p>
          {p.initialize
            ? "钱包在本机生成并保存身份秘密。注册后，您可以自主选择向应用披露的属性。"
            : "使用当前 Windows 用户解锁受保护的钱包数据。"}
        </p>
        <div class="auth-info">
          数据密钥由 Windows DPAPI 保护，钱包记录经过认证加密。
          <br />
          请妥善保留当前系统用户与受保护的数据目录。
        </div>
        <ErrorNotice error={action.error()} />
        <Button primary disabled={action.busy()} onClick={open}>
          {action.busy() ? "正在打开…" : p.initialize ? "创建钱包" : "解锁钱包"}
        </Button>
        <p class="hint">
          此操作只创建或解锁本地钱包。申请中心凭证需另行提交审批。
        </p>
      </div>
      <footer class="auth-foot">
        <span>Crypto SSO · 本地身份钱包</span>
        <span>Windows 安全保护</span>
      </footer>
    </div>
  );
}
export function Welcome() {
  return <VaultPage initialize />;
}
export function Unlock() {
  return <VaultPage initialize={false} />;
}
export function Home() {
  const credentials = createQuery(() => ({
    queryKey: ["credentials"],
    queryFn: () => command<Credential[]>("list_credentials"),
  }));
  const enrollments = createQuery(() => ({
    queryKey: ["enrollments"],
    queryFn: () => command<LocalEnrollment[]>("list_enrollments"),
  }));
  const centers = createQuery(() => ({
    queryKey: ["centers"],
    queryFn: () => command<Center[]>("list_centers"),
  }));
  const tasks = createQuery(() => ({
    queryKey: ["tasks"],
    queryFn: () => command<LoginTask[]>("list_login_tasks"),
  }));
  const pending = () =>
    enrollments.data?.filter(
      (e) =>
        !["CREDENTIAL_READY", "CREDENTIAL_DELETED"].includes(e.local_state) &&
        ![
          "REJECTED",
          "CANCELLED",
          "EXPIRED",
          "CHANGES_REQUIRED",
          "INVALID_REQUEST",
        ].includes(e.view?.status ?? ""),
    ) ?? [];
  return (
    <>
      <Head title="首页">
        <A class="btn primary" href="/login/new">
          <Icon name="arrow-up-right" />
          登录应用
        </A>
      </Head>
      <ErrorNotice
        error={credentials.error || enrollments.error || centers.error}
      />
      <div class="stat-strip">
        <For
          each={[
            [credentials.data?.length ?? 0, "可用凭证", "已解盲并保存在本地"],
            [pending().length, "待处理申请", "恢复材料已加密保存"],
            [
              tasks.data?.filter(
                (t) => !["REFUSED", "PROOF_GENERATED"].includes(t.state),
              ).length ?? 0,
              "未完成登录",
              "每次披露均需确认",
            ],
            [centers.data?.length ?? 0, "已信任中心", "按签发指纹隔离"],
          ]}
        >
          {([n, label, foot]) => (
            <div class="stat">
              <div class="label">{label}</div>
              <div class="value">{n}</div>
              <div class="foot">{foot}</div>
            </div>
          )}
        </For>
      </div>
      <div class="two-col">
        <section>
          <h2 class="section-title">
            待处理申请{" "}
            <A class="meta inline-link" href="/enrollments">
              全部申请 →
            </A>
          </h2>
          <For each={pending()} fallback={<Empty>暂无待处理申请</Empty>}>
            {(e) => (
              <A class="list-row" href={`/enrollments/${e.id}`}>
                <div>
                  <div>
                    {centers.data?.find((c) => c.id === e.center_id)?.info
                      .settings.display_name ?? "身份中心"}
                  </div>
                  <div class="sub mono">{short(e.id)}</div>
                </div>
                <Show
                  when={e.view}
                  fallback={<span class="badge warning">提交结果待确认</span>}
                >
                  {(v) => <Status value={v().status} />}
                </Show>
              </A>
            )}
          </For>
          <h2 class="section-title">我的凭证</h2>
          <For
            each={credentials.data}
            fallback={<Empty>连接中心并申请您的第一份凭证</Empty>}
          >
            {(c) => (
              <A href={`/credentials/${c.id}`} class="list-row">
                <div class="row-main">
                  <span class="object-icon">
                    <Icon name="person-vcard" />
                  </span>
                  <div>
                    {centers.data?.find((x) => x.id === c.center_id)?.info
                      .settings.display_name ?? "身份凭证"}
                    <div class="sub">
                      {Object.keys(c.claims).length} 项本地属性
                    </div>
                  </div>
                </div>
                <span class="badge success">凭证可用</span>
              </A>
            )}
          </For>
        </section>
        <aside>
          <h2 class="section-title">快速开始</h2>
          <A class="list-row" href="/centers/add">
            <span>
              <Icon name="buildings" /> 添加身份中心
            </span>
            <Icon name="chevron-right" />
          </A>
          <A class="list-row" href="/apps/new">
            <span>
              <Icon name="app-indicator" /> 配置受信应用
            </span>
            <Icon name="chevron-right" />
          </A>
          <Notice>已有凭证可以在中心离线时生成登录证明。</Notice>
        </aside>
      </div>
    </>
  );
}
export function Preferences() {
  const action = useAction();
  const [seconds, setSeconds] = createSignal(300),
    [saved, setSaved] = createSignal(false);
  const q = createQuery(() => ({
    queryKey: ["preferences"],
    queryFn: () =>
      command<{ auto_lock_seconds: number; version: number }>(
        "get_preferences",
      ),
  }));
  createEffect(() => {
    if (q.data) setSeconds(q.data.auto_lock_seconds);
  });
  return (
    <>
      <Head title="本地设置">
        <Button
          primary
          disabled={action.busy()}
          onClick={async () => {
            if (
              await action.run("save_preferences", {
                auto_lock_seconds: seconds(),
              })
            )
              setSaved(true);
          }}
        >
          保存更改
        </Button>
      </Head>
      <ErrorNotice error={action.error() || q.error} />
      <Show when={saved()}>
        <Notice>本地设置已保存。</Notice>
      </Show>
      <div class="settings-frame">
        <h2 class="section-title">锁定与本地保护</h2>
        <Field
          label="自动锁定时间（秒）"
          hint="30–3600 秒；后台同步不会延长使用时间。"
        >
          <input
            class="input"
            type="number"
            min="30"
            max="3600"
            value={seconds()}
            onInput={(e) => {
              setSeconds(Number(e.currentTarget.value));
              setSaved(false);
            }}
          />
        </Field>
        <div class="kv">
          <div>密钥保护</div>
          <div>Windows 当前用户 DPAPI</div>
          <div>本地数据</div>
          <div>AES-256-GCM 认证加密</div>
          <div>格式版本</div>
          <div>v{q.data?.version ?? 1}</div>
        </div>
        <Notice>
          删除本地凭证不会撤销中心签发记录。失去原 pending 后，无法仅用请求 ID
          恢复钱包秘密。
        </Notice>
        <p class="hint">
          本期支持当前 Windows
          用户下的恢复；备份应包含完整受保护数据目录和可恢复的系统用户环境。
        </p>
      </div>
    </>
  );
}
export function NotFound() {
  return (
    <div class="centered">
      <h1>页面不存在</h1>
      <A class="btn primary" href="/">
        返回首页
      </A>
    </div>
  );
}
