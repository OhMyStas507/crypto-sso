import { For, Show, createEffect, createSignal } from "solid-js";
import { A, useNavigate, useParams, useSearchParams } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import {
  command,
  type Center,
  type DisclosurePlan,
  type LoginTask,
  type TrustedApp,
} from "../bridge";
import { useAction } from "../actions";
import {
  Button,
  Empty,
  ErrorNotice,
  Field,
  Head,
  Icon,
  Modal,
  Notice,
  date,
} from "../ui";

export function Apps() {
  const q = createQuery(() => ({
    queryKey: ["apps"],
    queryFn: () => command<TrustedApp[]>("list_trusted_apps"),
  }));
  return (
    <>
      <Head title="受信应用">
        <A class="btn primary" href="/apps/new">
          <Icon name="plus-lg" />
          添加应用
        </A>
      </Head>
      <ErrorNotice error={q.error} />
      <Notice>应用 Origin 必须独立核对。导入挑战不会自动建立信任。</Notice>
      <For each={q.data} fallback={<Empty>尚未配置受信应用</Empty>}>
        {(app) => (
          <div class="list-row">
            <div class="row-main">
              <span class="object-icon">
                <Icon name="app-indicator" />
              </span>
              <div>
                {app.name}
                <div class="sub mono">{app.origin}</div>
                <div class="sub mono">Issuer · {app.issuer_id}</div>
              </div>
            </div>
            <div class="actions">
              <A class="btn" href={`/apps/${app.id}/edit`}>
                编辑
              </A>
              <A class="btn primary" href={`/login/new?app=${app.id}`}>
                开始登录
              </A>
            </div>
          </div>
        )}
      </For>
    </>
  );
}
export function AppEditor() {
  const params = useParams(),
    navigate = useNavigate(),
    action = useAction();
  const apps = createQuery(() => ({
    queryKey: ["apps"],
    queryFn: () => command<TrustedApp[]>("list_trusted_apps"),
  }));
  const centers = createQuery(() => ({
    queryKey: ["centers"],
    queryFn: () => command<Center[]>("list_centers"),
  }));
  const [name, setName] = createSignal(""),
    [origin, setOrigin] = createSignal(""),
    [issuer, setIssuer] = createSignal(""),
    [confirmed, setConfirmed] = createSignal(false),
    [remove, setRemove] = createSignal(false);
  let loaded = false;
  createEffect(() => {
    const app = apps.data?.find((a) => a.id === params.appId);
    if (app && !loaded) {
      loaded = true;
      setName(app.name);
      setOrigin(app.origin);
      setIssuer(app.issuer_id);
    }
  });
  const save = async (e: SubmitEvent) => {
    e.preventDefault();
    if (
      await action.run("save_trusted_app", {
        id: params.appId ?? "",
        name: name(),
        origin: origin(),
        issuer_id: issuer(),
      })
    )
      navigate("/apps");
  };
  return (
    <>
      <Head title={params.appId ? "编辑受信应用" : "添加受信应用"}>
        <Show when={params.appId}>
          <Button danger onClick={() => setRemove(true)}>
            移除应用
          </Button>
        </Show>
      </Head>
      <form class="form" onSubmit={save}>
        <ErrorNotice error={action.error()} />
        <Field label="应用名称">
          <input
            class="input"
            required
            value={name()}
            onInput={(e) => setName(e.currentTarget.value)}
          />
        </Field>
        <Field
          label="独立确认的应用 Origin"
          hint="只包含协议、主机和端口，例如 https://forum.example.org。"
        >
          <input
            class="input mono"
            required
            value={origin()}
            onInput={(e) => {
              setOrigin(e.currentTarget.value);
              setConfirmed(false);
            }}
          />
        </Field>
        <Field label="可信签发中心">
          <select
            class="input"
            required
            value={issuer()}
            onChange={(e) => {
              setIssuer(e.currentTarget.value);
              setConfirmed(false);
            }}
          >
            <option value="">选择已连接中心</option>
            <For each={centers.data}>
              {(c) => (
                <option value={c.info.issuer_id}>
                  {c.info.settings.display_name} ·{" "}
                  {c.info.issuer_id.slice(0, 12)}
                </option>
              )}
            </For>
          </select>
        </Field>
        <Show when={issuer()}>
          <div class="mono">{issuer()}</div>
        </Show>
        <label class="check-label">
          <input
            type="checkbox"
            required
            checked={confirmed()}
            onChange={(e) => setConfirmed(e.currentTarget.checked)}
          />
          我已独立核对实际浏览器地址和该应用使用的中心身份。
        </label>
        <Notice>
          修改应用配置会使旧披露授权计划失效，需导入新挑战并重新确认。
        </Notice>
        <div class="form-footer">
          <A class="btn" href="/apps">
            取消
          </A>
          <Button
            primary
            type="submit"
            disabled={!confirmed() || action.busy()}
          >
            保存受信应用
          </Button>
        </div>
      </form>
      <Show when={remove()}>
        <Modal
          title="移除受信应用"
          onClose={() => setRemove(false)}
          footer={
            <Button
              danger
              onClick={async () => {
                await action.run("remove_trusted_app", { id: params.appId });
                if (!action.error()) navigate("/apps");
              }}
            >
              确认移除
            </Button>
          }
        >
          <p>移除应用配置及其本地登录任务。已签发的凭证不受影响。</p>
        </Modal>
      </Show>
    </>
  );
}
export function StartLogin() {
  const [search] = useSearchParams(),
    navigate = useNavigate(),
    action = useAction();
  const apps = createQuery(() => ({
    queryKey: ["apps"],
    queryFn: () => command<TrustedApp[]>("list_trusted_apps"),
  }));
  const [appId, setAppId] = createSignal(
      typeof search.app === "string" ? search.app : "",
    ),
    [challenge, setChallenge] = createSignal(""),
    [confirmed, setConfirmed] = createSignal(false);
  const app = () => apps.data?.find((a) => a.id === appId());
  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    const task = await action.run<LoginTask>("import_login_challenge", {
      app_id: appId(),
      challenge_json: challenge(),
    });
    if (task) navigate(`/login/${task.id}`);
  };
  return (
    <>
      <Head title="开始应用登录" />
      <form class="form" onSubmit={submit}>
        <ErrorNotice error={action.error()} />
        <Field label="受信应用">
          <select
            class="input"
            required
            value={appId()}
            onChange={(e) => {
              setAppId(e.currentTarget.value);
              setConfirmed(false);
            }}
          >
            <option value="">请选择已配置的应用</option>
            <For each={apps.data}>
              {(a) => <option value={a.id}>{a.name}</option>}
            </For>
          </select>
        </Field>
        <Show when={app()}>
          {(a) => (
            <div class="app-identity">
              <span class="object-icon">
                <Icon name="app-indicator" />
              </span>
              <div>
                <strong>{a().name}</strong>
                <div class="origin">{a().origin}</div>
              </div>
              <span class="badge success">已固定 Origin</span>
            </div>
          )}
        </Show>
        <Field
          label="从原应用页面复制的公开挑战"
          hint="只粘贴 LoginChallenge；不包含 Cookie、密码或钱包秘密。"
        >
          <textarea
            class="input mono"
            style={{ height: "210px" }}
            required
            value={challenge()}
            onInput={(e) => setChallenge(e.currentTarget.value)}
          />
        </Field>
        <label class="check-label">
          <input
            type="checkbox"
            required
            checked={confirmed()}
            onChange={(e) => setConfirmed(e.currentTarget.checked)}
          />
          实际浏览器地址与上方固定 Origin 一致，我希望登录此应用。
        </label>
        <div class="form-footer">
          <A class="btn" href="/apps/new">
            配置新应用
          </A>
          <Button
            primary
            type="submit"
            disabled={action.busy() || !app() || !confirmed()}
          >
            导入并核对挑战
          </Button>
        </div>
      </form>
    </>
  );
}
export function LoginTaskPage() {
  const params = useParams(),
    action = useAction();
  const q = createQuery(() => ({
    queryKey: ["login", params.taskId],
    queryFn: () =>
      command<DisclosurePlan>("get_disclosure_plan", { id: params.taskId }),
    refetchInterval: 5000,
  }));
  const [selected, setSelected] = createSignal<string[]>([]),
    [planRevision, setPlanRevision] = createSignal(-1),
    [proof, setProof] = createSignal(""),
    [challenge, setChallenge] = createSignal(""),
    [refresh, setRefresh] = createSignal(false),
    [confirmed, setConfirmed] = createSignal(false);
  createEffect(() => {
    const plan = q.data;
    if (plan && plan.task.revision !== planRevision()) {
      setPlanRevision(plan.task.revision);
      setSelected([...plan.required_claims]);
      setConfirmed(false);
    }
  });
  const present = async () => {
    const plan = q.data;
    if (!plan?.credential) return;
    const result = await action.run<{ presentation_json: string }>(
      "create_presentation",
      {
        id: params.taskId,
        expected_revision: plan.task.revision,
        credential_id: plan.credential.id,
        disclose_keys: selected(),
      },
    );
    if (result) setProof(result.presentation_json);
  };
  const fresh = async () => {
    const plan = q.data;
    if (!plan) return;
    if (
      await action.run("import_login_challenge", {
        app_id: plan.app.id,
        task_id: params.taskId,
        challenge_json: challenge(),
      })
    ) {
      setRefresh(false);
      setProof("");
    }
  };
  return (
    <>
      <ErrorNotice error={q.error || action.error()} />
      <Show when={q.data}>
        {(d) => (
          <div class="login-layout">
            <Head title="登录授权">
              <Button onClick={() => setRefresh(true)}>导入新挑战</Button>
            </Head>
            <div class="app-identity">
              <span class="object-icon">
                <Icon name="app-indicator" />
              </span>
              <div>
                <strong>{d().app.name}</strong>
                <div class="origin">{d().app.origin}</div>
              </div>
              <span class="badge success">独立固定的应用</span>
            </div>
            <Show
              when={proof()}
              fallback={
                <>
                  <Show when={d().state === "CREDENTIAL_DELETED"}>
                    <Notice danger>
                      本地凭证已删除，中心签发名额仍保留。本期没有自动补发或迁移功能，无法从原请求
                      ID 恢复钱包秘密。
                    </Notice>
                  </Show>
                  <Show when={d().state === "NO_CREDENTIAL"}>
                    <Notice>
                      尚无该中心的可用凭证。注册申请独立于本次登录，审批期间挑战可以自然过期。
                    </Notice>
                    <Show when={d().center_id}>
                      <A
                        class="btn primary"
                        href={`/centers/${d().center_id}/enroll`}
                      >
                        申请中心凭证
                      </A>
                    </Show>
                    <A class="btn" href="/enrollments">
                      查看申请进度
                    </A>
                  </Show>
                  <Show
                    when={
                      d().state === "NEEDS_CHALLENGE" ||
                      d().state === "PROOF_GENERATED"
                    }
                  >
                    <Notice>
                      请回到原应用获取新挑战，再次核对并确认披露。凭证不会因挑战到期失效。
                    </Notice>
                    <Button primary onClick={() => setRefresh(true)}>
                      导入新挑战
                    </Button>
                  </Show>
                  <Show when={d().state === "MISSING_CLAIMS"}>
                    <Notice danger>
                      当前凭证缺少应用必需属性：{d().missing_claims.join("、")}
                      。本期不自动续签。
                    </Notice>
                  </Show>
                  <Show when={d().state === "REFUSED"}>
                    <Notice>
                      您已拒绝本次披露。未生成可提交证明，应用会话未建立。
                    </Notice>
                  </Show>
                  <Show when={d().state === "REVIEW_DISCLOSURE"}>
                    <div class="login-meta">
                      <section>
                        <h2 class="section-title">应用要求披露</h2>
                        <For
                          each={d().required_claims}
                          fallback={
                            <p class="hint">本次应用不要求披露任何属性。</p>
                          }
                        >
                          {(key) => (
                            <div class="disclosure-row">
                              <label>
                                <input
                                  type="checkbox"
                                  checked={selected().includes(key)}
                                  onChange={(e) =>
                                    setSelected(
                                      e.currentTarget.checked
                                        ? [...selected(), key]
                                        : selected().filter((k) => k !== key),
                                    )
                                  }
                                />
                                <span class="mono">{key}</span>
                              </label>
                              <span class="claim-value preserve">
                                {d().credential?.claims[key] || "空字符串"}
                              </span>
                              <span class="privacy">必需</span>
                            </div>
                          )}
                        </For>
                        <h2 class="section-title">额外选择披露</h2>
                        <For
                          each={Object.keys(
                            d().credential?.claims ?? {},
                          ).filter((k) => !d().required_claims.includes(k))}
                          fallback={<p class="hint">没有额外属性。</p>}
                        >
                          {(key) => (
                            <div class="disclosure-row">
                              <label>
                                <input
                                  type="checkbox"
                                  checked={selected().includes(key)}
                                  onChange={(e) =>
                                    setSelected(
                                      e.currentTarget.checked
                                        ? [...selected(), key]
                                        : selected().filter((k) => k !== key),
                                    )
                                  }
                                />
                                <span class="mono">{key}</span>
                              </label>
                              <span class="claim-value preserve">
                                {d().credential?.claims[key] || "空字符串"}
                              </span>
                              <span class="privacy">
                                {selected().includes(key)
                                  ? "将披露"
                                  : "保持隐藏"}
                              </span>
                            </div>
                          )}
                        </For>
                        <label class="check-label">
                          <input
                            type="checkbox"
                            checked={confirmed()}
                            onChange={(e) =>
                              setConfirmed(e.currentTarget.checked)
                            }
                          />
                          已核对实际应用 Origin，同意披露当前选中的键值。
                        </label>
                        <Show
                          when={d().required_claims.some(
                            (k) => !selected().includes(k),
                          )}
                        >
                          <Notice danger>拒绝任一必需项将中止本次登录。</Notice>
                        </Show>
                        <div class="form-footer">
                          <Button
                            danger
                            disabled={action.busy()}
                            onClick={() =>
                              action.run("refuse_disclosure", {
                                id: params.taskId,
                              })
                            }
                          >
                            拒绝本次披露
                          </Button>
                          <Button
                            primary
                            disabled={
                              action.busy() ||
                              !confirmed() ||
                              d().required_claims.some(
                                (k) => !selected().includes(k),
                              )
                            }
                            onClick={present}
                          >
                            {action.busy() ? "正在生成证明…" : "确认并生成证明"}
                          </Button>
                        </div>
                      </section>
                      <aside class="login-summary">
                        <div class="meta-row">
                          <div class="label">目标签发中心</div>
                          <div class="mono">{d().app.issuer_id}</div>
                        </div>
                        <div class="meta-row">
                          <div class="label">挑战截止</div>
                          {date(d().expires_at)}
                        </div>
                        <div class="meta-row">
                          <div class="label">未选属性</div>保持隐藏
                        </div>
                        <p class="hint">
                          生成证明不表示应用已登录；将公开证明交回同一浏览器页面。
                        </p>
                      </aside>
                    </div>
                  </Show>
                </>
              }
            >
              <Notice>
                公开证明已生成。请交回同一浏览器的原应用页面，由应用验真并建立会话。
              </Notice>
              <textarea
                class="input mono"
                readonly
                style={{ height: "250px" }}
                value={proof()}
              />
              <div class="form-footer">
                <Button
                  primary
                  onClick={() => navigator.clipboard.writeText(proof())}
                >
                  复制公开证明
                </Button>
              </div>
            </Show>
            <Show when={refresh()}>
              <Modal
                title="导入原应用的新挑战"
                onClose={() => setRefresh(false)}
                footer={
                  <Button
                    primary
                    disabled={action.busy() || !challenge()}
                    onClick={fresh}
                  >
                    核对并更新
                  </Button>
                }
              >
                <ErrorNotice error={action.error()} />
                <p>
                  固定 Origin：<span class="mono">{d().app.origin}</span>
                  。更新后重新确认披露。
                </p>
                <textarea
                  class="input mono"
                  style={{ height: "190px" }}
                  value={challenge()}
                  onInput={(e) => setChallenge(e.currentTarget.value)}
                />
              </Modal>
            </Show>
          </div>
        )}
      </Show>
    </>
  );
}
