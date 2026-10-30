import { For, Index, Show, createEffect, createSignal } from "solid-js";
import { useLocation } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import type { ClaimHint } from "@crypto-sso/identity-api-types";
import { api, ApiFailure, type ServiceSettings } from "../api";
import {
  Button,
  ErrorNotice,
  Field,
  Head,
  Icon,
  Modal,
  Notice,
  SettingsTabs,
} from "../ui";
import { useWrite } from "../features/useWrite";

export function SettingsPage() {
  const location = useLocation();
  const claimsMode = () => location.pathname.endsWith("/claims");
  const q = createQuery(() => ({
    queryKey: ["settings"],
    queryFn: () => api<ServiceSettings>("settings"),
  }));
  const [draft, setDraft] = createSignal<ServiceSettings>();
  const [saved, setSaved] = createSignal(false);
  const [preview, setPreview] = createSignal(false);
  const write = useWrite();
  createEffect(() => {
    if (q.data && !draft()) setDraft(JSON.parse(JSON.stringify(q.data)));
  });
  const update = <K extends keyof ServiceSettings>(
    key: K,
    value: ServiceSettings[K],
  ) => {
    setSaved(false);
    setDraft((d) => (d ? { ...d, [key]: value } : d));
  };
  const hint = (index: number, key: keyof ClaimHint, value: unknown) =>
    update(
      "claim_hints",
      draft()!.claim_hints.map((h, i) =>
        i === index ? { ...h, [key]: value } : h,
      ),
    );
  const save = async () => {
    const result = await write.run<ServiceSettings>("settings", "PUT", draft());
    if (result) {
      setDraft(result);
      setSaved(true);
      await q.refetch();
    }
  };
  const reload = async () => {
    const data = await q.refetch();
    setDraft(JSON.parse(JSON.stringify(data.data!)));
    write.clear();
  };
  const move = (index: number, offset: number) => {
    const hints = [...draft()!.claim_hints];
    [hints[index], hints[index + offset]] = [
      hints[index + offset],
      hints[index],
    ];
    update("claim_hints", hints);
  };
  return (
    <>
      <Head title={claimsMode() ? "Claims 默认建议" : "服务设置"}>
        <Show when={claimsMode()}>
          <Button onClick={() => setPreview(true)}>预览钱包表单</Button>
        </Show>
        <Button primary disabled={!draft() || write.busy()} onClick={save}>
          {write.busy() ? "保存中…" : "保存更改"}
        </Button>
      </Head>
      <SettingsTabs />
      <ErrorNotice error={q.error} />
      <ErrorNotice error={write.error()} />
      <Show
        when={
          write.error() instanceof ApiFailure &&
          (write.error() as ApiFailure).code === "REVISION_CONFLICT"
        }
      >
        <Notice>
          服务端配置已更新。当前草稿已保留；记录草稿后，可重新加载并手动合并。
          <Button onClick={reload}>重新加载服务端配置</Button>
        </Notice>
      </Show>
      <Show when={saved()}>
        <Notice>设置已保存并生效。</Notice>
      </Show>
      <Show when={draft()}>
        {(d) => (
          <div class="settings-frame">
            <Show
              when={claimsMode()}
              fallback={
                <>
                  <div class="section section-rule">
                    <h2 class="section-title">
                      公开服务信息{" "}
                      <span class="meta">配置 r{d().revision}</span>
                    </h2>
                    <Field label="服务展示名">
                      <input
                        class="input"
                        value={d().display_name}
                        onInput={(e) =>
                          update("display_name", e.currentTarget.value)
                        }
                      />
                    </Field>
                    <Field
                      label="公开服务地址"
                      hint="HTTPS 或本地 loopback HTTP；与反向代理的公开地址保持一致。"
                    >
                      <input
                        class="input"
                        value={d().public_base_url}
                        onInput={(e) =>
                          update("public_base_url", e.currentTarget.value)
                        }
                      />
                    </Field>
                    <Field label="服务说明">
                      <textarea
                        class="input"
                        value={d().description}
                        onInput={(e) =>
                          update("description", e.currentTarget.value)
                        }
                      />
                    </Field>
                    <Field label="联系地址">
                      <input
                        class="input"
                        value={d().contact}
                        onInput={(e) =>
                          update("contact", e.currentTarget.value)
                        }
                      />
                    </Field>
                  </div>
                  <h2 class="section-title">注册与签发策略</h2>
                  <label class="pref-row">
                    <div>
                      接受新注册
                      <div class="desc">关闭后，既有申请仍可查询和领取。</div>
                    </div>
                    <input
                      type="checkbox"
                      checked={d().registration_enabled}
                      onChange={(e) =>
                        update("registration_enabled", e.currentTarget.checked)
                      }
                    />
                  </label>
                  <label class="pref-row">
                    <div>
                      开启凭证签发
                      <div class="desc">暂停会保留审批结果与成员名额。</div>
                    </div>
                    <input
                      type="checkbox"
                      checked={d().issuance_enabled}
                      onChange={(e) =>
                        update("issuance_enabled", e.currentTarget.checked)
                      }
                    />
                  </label>
                  <div class="form-grid">
                    <Field label="新申请待审期限（秒）">
                      <input
                        class="input"
                        type="number"
                        min="60"
                        value={d().review_ttl_seconds}
                        onInput={(e) =>
                          update(
                            "review_ttl_seconds",
                            Number(e.currentTarget.value),
                          )
                        }
                      />
                    </Field>
                    <Field label="建议轮询间隔（秒）">
                      <input
                        class="input"
                        type="number"
                        min="1"
                        value={d().poll_interval_seconds}
                        onInput={(e) =>
                          update(
                            "poll_interval_seconds",
                            Number(e.currentTarget.value),
                          )
                        }
                      />
                    </Field>
                  </div>
                </>
              }
            >
              <Notice>
                建议可修改、删除或扩展；允许空属性集合。所有实际申请属性仍需人工核验。
              </Notice>
              <Index each={d().claim_hints}>
                {(h, i) => (
                  <section class="section">
                    <div class="hint-editor">
                      <input
                        class="input mono"
                        aria-label="建议键"
                        placeholder="属性键"
                        value={h().key}
                        onInput={(e) => hint(i, "key", e.currentTarget.value)}
                      />
                      <input
                        class="input"
                        aria-label="显示名称"
                        placeholder="显示名称"
                        value={h().label}
                        onInput={(e) => hint(i, "label", e.currentTarget.value)}
                      />
                      <input
                        class="input"
                        aria-label="默认值"
                        placeholder="默认值（未填为空）"
                        value={h().default_value ?? ""}
                        onInput={(e) =>
                          hint(i, "default_value", e.currentTarget.value)
                        }
                      />
                      <Button
                        danger
                        onClick={() =>
                          update(
                            "claim_hints",
                            d().claim_hints.filter((_, n) => n !== i),
                          )
                        }
                      >
                        删除
                      </Button>
                    </div>
                    <div class="form-grid">
                      <Field label="说明">
                        <input
                          class="input"
                          value={h().description}
                          onInput={(e) =>
                            hint(i, "description", e.currentTarget.value)
                          }
                        />
                      </Field>
                      <Field label="占位示例（不提交）">
                        <input
                          class="input"
                          value={h().example_value ?? ""}
                          onInput={(e) =>
                            hint(i, "example_value", e.currentTarget.value)
                          }
                        />
                      </Field>
                    </div>
                    <div class="between">
                      <label class="check-label">
                        <input
                          type="checkbox"
                          checked={h().include_by_default}
                          onChange={(e) =>
                            hint(
                              i,
                              "include_by_default",
                              e.currentTarget.checked,
                            )
                          }
                        />
                        默认加入钱包草稿
                      </label>
                      <div class="actions">
                        <Button disabled={i === 0} onClick={() => move(i, -1)}>
                          上移
                        </Button>
                        <Button
                          disabled={i === d().claim_hints.length - 1}
                          onClick={() => move(i, 1)}
                        >
                          下移
                        </Button>
                      </div>
                    </div>
                  </section>
                )}
              </Index>
              <Button
                onClick={() =>
                  update("claim_hints", [
                    ...d().claim_hints,
                    {
                      key: "",
                      label: "",
                      description: "",
                      default_value: null,
                      example_value: null,
                      include_by_default: true,
                    },
                  ])
                }
              >
                <Icon name="plus-lg" />
                添加建议
              </Button>
            </Show>
          </div>
        )}
      </Show>
      <Show when={preview()}>
        <Modal
          title="钱包建议表单预览"
          onClose={() => setPreview(false)}
          footer={
            <Button primary onClick={() => setPreview(false)}>
              关闭预览
            </Button>
          }
        >
          <For
            each={draft()?.claim_hints.filter((h) => h.include_by_default)}
            fallback={<Notice>默认空属性集合</Notice>}
          >
            {(h) => (
              <Field label={`${h.key} · ${h.label}`} hint={h.description}>
                <input
                  class="input"
                  value={h.default_value ?? ""}
                  placeholder={h.example_value ?? ""}
                />
              </Field>
            )}
          </For>
          <p class="hint">全部建议可由钱包用户修改或删除。</p>
        </Modal>
      </Show>
    </>
  );
}
