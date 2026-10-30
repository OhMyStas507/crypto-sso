import { For, Show, createSignal } from "solid-js";
import { A, useNavigate, useParams } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import type { ServiceInfo } from "@crypto-sso/identity-api-types";
import { command, type Center, type Draft } from "../bridge";
import { useAction } from "../actions";
import { Button, Empty, ErrorNotice, Field, Head, Icon, Notice } from "../ui";

export function CenterList() {
  const q = createQuery(() => ({
    queryKey: ["centers"],
    queryFn: () => command<Center[]>("list_centers"),
  }));
  return (
    <>
      <Head title="身份中心">
        <A href="/centers/add" class="btn primary">
          <Icon name="plus-lg" />
          添加中心
        </A>
      </Head>
      <ErrorNotice error={q.error} />
      <For
        each={q.data}
        fallback={
          <Empty>尚未连接身份中心。请从可信渠道获取地址和签发指纹。</Empty>
        }
      >
        {(c) => (
          <A class="list-row" href={`/centers/${c.id}`}>
            <div class="row-main">
              <span class="object-icon">
                <Icon name="buildings" />
              </span>
              <div>
                {c.info.settings.display_name}
                <div class="sub">{c.base_url}</div>
                <div class="sub mono">{c.info.issuer_id}</div>
              </div>
            </div>
            <div class="right-group">
              <span class="badge success">指纹已固定</span>
              <Icon name="chevron-right" />
            </div>
          </A>
        )}
      </For>
    </>
  );
}
export function AddCenter() {
  const [url, setUrl] = createSignal(""),
    [info, setInfo] = createSignal<ServiceInfo>(),
    [fingerprint, setFingerprint] = createSignal(""),
    [confirmed, setConfirmed] = createSignal(false);
  const action = useAction(),
    navigate = useNavigate();
  const probe = async () => {
    const result = await action.run<ServiceInfo>("probe_center", {
      url: url(),
    });
    if (result) setInfo(result);
  };
  const save = async () => {
    const center = await action.run<Center>("confirm_center", {
      url: url(),
      expected_issuer_id: fingerprint(),
    });
    if (center) navigate(`/centers/${center.id}`);
  };
  return (
    <>
      <Head title="添加身份中心" />
      <div class="form">
        <div class="steps">
          <span class="active">1 输入地址</span>
          <span>—</span>
          <span>2 核对指纹</span>
          <span>—</span>
          <span>3 确认连接</span>
        </div>
        <ErrorNotice error={action.error()} />
        <Field
          label="中心服务地址"
          hint="使用 HTTPS；本地开发允许 loopback HTTP。"
        >
          <input
            class="input"
            value={url()}
            placeholder="https://identity.example.org"
            onInput={(e) => {
              setUrl(e.currentTarget.value);
              setInfo(undefined);
              setConfirmed(false);
            }}
          />
        </Field>
        <Button primary disabled={!url() || action.busy()} onClick={probe}>
          {action.busy() ? "正在读取…" : "读取中心信息"}
        </Button>
        <Show when={info()}>
          {(data) => (
            <>
              <section class="section">
                <h2 class="section-title">待核对的中心</h2>
                <div class="kv">
                  <div>展示名</div>
                  <div>{data().settings.display_name}</div>
                  <div>组织标识</div>
                  <div>
                    {
                      (data().issuer_public as { organization: string })
                        .organization
                    }
                  </div>
                  <div>公开指纹</div>
                  <div class="mono">{data().issuer_id}</div>
                  <div>接受新注册</div>
                  <div>
                    {data().settings.registration_enabled ? "开放" : "关闭"}
                  </div>
                </div>
              </section>
              <Notice>
                通过组织官网、线下资料等独立可信渠道取得完整指纹，并在下方输入。当前连接返回的指纹本身不能证明中心可信。
              </Notice>
              <Field label="独立核对的完整 Issuer 指纹">
                <input
                  class="input mono"
                  value={fingerprint()}
                  onInput={(e) => setFingerprint(e.currentTarget.value)}
                />
              </Field>
              <Show when={fingerprint() && fingerprint() !== data().issuer_id}>
                <Notice danger>指纹不匹配，禁止连接。</Notice>
              </Show>
              <label class="check-label">
                <input
                  type="checkbox"
                  checked={confirmed()}
                  onChange={(e) => setConfirmed(e.currentTarget.checked)}
                />
                我已通过独立渠道核对中心地址和完整指纹。
              </label>
              <div class="form-footer">
                <A href="/centers" class="btn">
                  取消
                </A>
                <Button
                  primary
                  disabled={
                    !confirmed() ||
                    fingerprint() !== data().issuer_id ||
                    action.busy()
                  }
                  onClick={save}
                >
                  确认并信任中心
                </Button>
              </div>
            </>
          )}
        </Show>
      </div>
    </>
  );
}
export function CenterDetail() {
  const params = useParams(),
    navigate = useNavigate(),
    action = useAction();
  const q = createQuery(() => ({
    queryKey: ["center", params.centerId],
    queryFn: () => command<Center>("get_center", { id: params.centerId }),
  }));
  const enroll = async () => {
    const draft = await action.run<Draft>("create_enrollment_draft", {
      center_id: params.centerId,
    });
    if (draft) navigate(`/centers/${params.centerId}/enroll?draft=${draft.id}`);
  };
  return (
    <>
      <ErrorNotice error={q.error || action.error()} />
      <Show when={q.data}>
        {(c) => (
          <>
            <Head title={c().info.settings.display_name}>
              <Button
                disabled={action.busy()}
                onClick={() =>
                  action.run("refresh_claim_hints", { id: params.centerId })
                }
              >
                刷新公开信息与建议
              </Button>
              <Button primary onClick={enroll}>
                申请注册
              </Button>
            </Head>
            <div class="two-col">
              <section>
                <h2 class="section-title">中心信息</h2>
                <div class="kv">
                  <div>服务地址</div>
                  <div class="mono">{c().base_url}</div>
                  <div>固定指纹</div>
                  <div class="mono">{c().info.issuer_id}</div>
                  <div>服务说明</div>
                  <div class="preserve">
                    {c().info.settings.description || "—"}
                  </div>
                  <div>联系地址</div>
                  <div>{c().info.settings.contact || "—"}</div>
                  <div>注册 / 签发</div>
                  <div>
                    {c().info.settings.registration_enabled
                      ? "接受新注册"
                      : "注册关闭"}{" "}
                    /{" "}
                    {c().info.settings.issuance_enabled
                      ? "签发开启"
                      : "签发暂停"}
                  </div>
                </div>
                <h2 class="section-title">
                  Claims 默认建议{" "}
                  <span class="meta">
                    缓存 r{c().info.settings.claim_hints_revision}
                  </span>
                </h2>
                <table class="table">
                  <thead>
                    <tr>
                      <th>属性键</th>
                      <th>说明</th>
                      <th>默认值</th>
                      <th>默认加入</th>
                    </tr>
                  </thead>
                  <tbody>
                    <For
                      each={c().info.settings.claim_hints}
                      fallback={
                        <tr>
                          <td colspan="4">
                            无默认建议，可以从空集合手工填写。
                          </td>
                        </tr>
                      }
                    >
                      {(h) => (
                        <tr>
                          <td class="mono">{h.key}</td>
                          <td>
                            {h.label}
                            <div class="sub">{h.description}</div>
                          </td>
                          <td>{h.default_value ?? "空字符串"}</td>
                          <td>{h.include_by_default ? "是" : "否"}</td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </section>
              <aside>
                <Notice>
                  建议允许修改、删除和扩展。中心会看到本次提交的全部属性，后续登录时才可选择披露。
                </Notice>
                <p class="hint">
                  页面显示上次读取的信息。注册提交与领取前会重新校验固定中心身份。
                </p>
              </aside>
            </div>
          </>
        )}
      </Show>
    </>
  );
}
