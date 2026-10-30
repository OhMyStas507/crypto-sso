import { For, Index, Show, createEffect, createSignal } from "solid-js";
import { A, useNavigate, useParams, useSearchParams } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import {
  command,
  registrationClosedMessage,
  type Center,
  type Draft,
  type LocalEnrollment,
} from "../bridge";
import { useAction } from "../actions";
import {
  Button,
  ClaimsTable,
  Empty,
  ErrorNotice,
  Field,
  Head,
  Icon,
  Modal,
  Notice,
  Status,
  date,
  short,
} from "../ui";

export const localStates: Record<string, string> = {
  SAVED_PENDING_SUBMISSION: "已保存待提交",
  SUBMISSION_UNKNOWN: "提交结果未知",
  WAITING_CENTER: "等待中心处理",
  RESPONSE_SAVED: "已保存签发响应",
  CREDENTIAL_SAVE_FAILED: "凭证保存失败",
  CREDENTIAL_READY: "凭证可用",
  CREDENTIAL_DELETED: "凭证已在本地删除",
};
const bytes = (s: string) => new TextEncoder().encode(s).length;

export function DraftPage() {
  const params = useParams(),
    [search, setSearch] = useSearchParams(),
    navigate = useNavigate(),
    action = useAction();
  const [draft, setDraft] = createSignal<Draft>(),
    [preview, setPreview] = createSignal(false),
    [hintsDialog, setHintsDialog] = createSignal(false),
    [saved, setSaved] = createSignal(false);
  const center = createQuery(() => ({
    queryKey: ["center", params.centerId],
    queryFn: () => command<Center>("get_center", { id: params.centerId }),
  }));
  const q = createQuery(() => ({
    queryKey: ["draft", search.draft],
    enabled: !!search.draft,
    queryFn: () => command<Draft>("get_enrollment_draft", { id: search.draft }),
  }));
  createEffect(() => {
    if (q.data && !draft()) setDraft(JSON.parse(JSON.stringify(q.data)));
  });
  createEffect(() => {
    if (!search.draft)
      void action
        .run<Draft>("create_enrollment_draft", { center_id: params.centerId })
        .then((d) => {
          if (d) {
            setDraft(d);
            setSearch({ draft: d.id });
          }
        });
  });
  const update = (rows: [string, string][]) => {
    setDraft((d) => (d ? { ...d, rows } : d));
    setSaved(false);
  };
  const edit = (index: number, column: number, value: string) =>
    update(
      draft()!.rows.map((row, i) =>
        i === index ? (column === 0 ? [value, row[1]] : [row[0], value]) : row,
      ),
    );
  const error = () => {
    const rows = draft()?.rows ?? [],
      limits = center.data?.info.effective_claims_limits;
    if (!limits) return "";
    if (rows.length > Math.min(128, limits.max_keys))
      return "属性数量超过双方允许的范围。";
    if (new Set(rows.map(([k]) => k)).size !== rows.length)
      return "存在重复属性键。";
    if (
      rows.some(
        ([k, v]) =>
          !/^[a-z][a-z0-9_.-]*$/.test(k) ||
          bytes(k) > Math.min(64, limits.max_key_bytes) ||
          bytes(v) > Math.min(1024, limits.max_value_bytes),
      )
    )
      return "属性键格式或 UTF-8 字节数不符合限制。";
    return "";
  };
  const save = async () => {
    if (await action.run("save_enrollment_draft", draft())) setSaved(true);
  };
  const submit = async () => {
    if (!(await action.run("save_enrollment_draft", draft()))) return;
    const entry = await action.run<LocalEnrollment>("submit_enrollment", {
      draft_id: draft()!.id,
    });
    if (entry) navigate(`/enrollments/${entry.id}`);
  };
  const restore = () => {
    const current = draft()!,
      hints = center.data!.info.settings;
    // User-edited and explicitly removed rows are preserved. Reintroductions are individually selected below.
    setDraft({ ...current, claim_hints_revision: hints.claim_hints_revision });
    setHintsDialog(false);
    setSaved(false);
  };
  return (
    <>
      <Head title="申请注册">
        <Button disabled={action.busy() || !!error()} onClick={save}>
          保存草稿
        </Button>
        <Button
          primary
          disabled={!draft() || !!error() || action.busy()}
          onClick={() => setPreview(true)}
        >
          预览并提交
        </Button>
      </Head>
      <div class="steps">
        <span class="active">① 填写属性</span>
        <span>—</span>
        <span>② 确认申请</span>
        <span>—</span>
        <span>③ 提交审批</span>
      </div>
      <ErrorNotice error={q.error || action.error() || center.error} />
      <Show when={center.data?.info.settings.registration_enabled === false}>
        <Notice>
          中心上次公开的信息显示注册已关闭。当前草稿仍可编辑和保存；提交时会重新检查注册状态。
        </Notice>
      </Show>
      <Show when={error()}>
        <Notice danger>{error()}</Notice>
      </Show>
      <Show when={draft()}>
        {(d) => (
          <div class="form">
            <section class="section">
              <h2 class="section-title">
                {center.data?.info.settings.display_name}
                <span class="meta badge success">指纹已固定</span>
              </h2>
              <span class="mono secondary">{center.data?.base_url}</span>
            </section>
            <div class="claim-editor">
              <div class="editor-head">
                <span>属性键</span>
                <span>字符串值</span>
                <span />
              </div>
              <Index
                each={d().rows}
                fallback={
                  <Notice>当前为空属性集合。中心仍会核验成员资格。</Notice>
                }
              >
                {(row, i) => (
                  <div class="editor-row">
                    <div>
                      <input
                        class="input mono"
                        aria-label={`属性键 ${i + 1}`}
                        value={row()[0]}
                        onInput={(e) => edit(i, 0, e.currentTarget.value)}
                      />
                      <div class="below">键名不自动去除空格</div>
                    </div>
                    <div>
                      <input
                        class="input"
                        aria-label={`属性值 ${i + 1}`}
                        value={row()[1]}
                        onInput={(e) => edit(i, 1, e.currentTarget.value)}
                      />
                      <div class="below">
                        {bytes(row()[1])} /{" "}
                        {Math.min(
                          1024,
                          center.data?.info.effective_claims_limits
                            .max_value_bytes ?? 1024,
                        )}{" "}
                        字节
                      </div>
                    </div>
                    <button
                      class="icon-button"
                      aria-label={`删除属性 ${i + 1}`}
                      onClick={() => update(d().rows.filter((_, n) => n !== i))}
                    >
                      <Icon name="trash3" />
                    </button>
                  </div>
                )}
              </Index>
            </div>
            <div class="between">
              <div class="actions">
                <Button onClick={() => update([...d().rows, ["", ""]])}>
                  <Icon name="plus-lg" />
                  添加属性
                </Button>
                <Button onClick={() => setHintsDialog(true)}>
                  查看 / 恢复建议
                </Button>
                <Button
                  disabled={action.busy()}
                  onClick={() =>
                    action.run("refresh_claim_hints", { id: params.centerId })
                  }
                >
                  刷新建议缓存
                </Button>
              </div>
              <span class="hint">
                {d().rows.length} /{" "}
                {Math.min(
                  128,
                  center.data?.info.effective_claims_limits.max_keys ?? 128,
                )}{" "}
                项
              </span>
            </div>
            <p class="note">
              保留空值会签署空字符串；删除行表示不包含该键。全部属性都会提交中心审批。
            </p>
            <Field label="申请说明（可选）" hint="仅用于审批沟通，不签入凭证。">
              <textarea
                class="input"
                maxLength={4096}
                value={d().application_note}
                onInput={(e) => {
                  setDraft({ ...d(), application_note: e.currentTarget.value });
                  setSaved(false);
                }}
              />
            </Field>
            <p class="hint">
              {saved() ? "草稿已加密保存到本地。" : "存在未保存更改。"} ·
              建议来源{" "}
              {d().claim_hints_revision ? `r${d().claim_hints_revision}` : "无"}
            </p>
          </div>
        )}
      </Show>
      <Show when={preview()}>
        <Modal
          title="确认完整申请"
          onClose={() => setPreview(false)}
          footer={
            <>
              <Button onClick={() => setPreview(false)}>返回编辑</Button>
              <Button
                primary
                disabled={action.busy() || !!error()}
                onClick={submit}
              >
                {action.busy() ? "正在保存并提交…" : "提交申请"}
              </Button>
            </>
          }
        >
          <ErrorNotice error={action.error()} />
          <Notice>
            以下完整属性将提交给中心。审批通过后仍需签发、领证与本地保存。
          </Notice>
          <ClaimsTable claims={Object.fromEntries(draft()!.rows)} />
          <p class="preserve">{draft()?.application_note}</p>
        </Modal>
      </Show>
      <Show when={hintsDialog()}>
        <Modal
          title="对比并选择建议"
          onClose={() => setHintsDialog(false)}
          footer={
            <Button primary onClick={restore}>
              保留当前字段并采用此建议版本
            </Button>
          }
        >
          <p>当前草稿保留所有用户修改。下方缺失项仅在点击“加入”后恢复。</p>
          <For each={center.data?.info.settings.claim_hints}>
            {(h) => (
              <div class="list-row">
                <div>
                  <span class="mono">{h.key}</span>
                  <div class="sub">
                    {h.label} · 建议值：{h.default_value ?? "空字符串"}
                  </div>
                </div>
                <Show
                  when={!draft()?.rows.some(([k]) => k === h.key)}
                  fallback={<span class="hint">已存在 · 保留用户值</span>}
                >
                  <Button
                    onClick={() =>
                      update([...draft()!.rows, [h.key, h.default_value ?? ""]])
                    }
                  >
                    加入
                  </Button>
                </Show>
              </div>
            )}
          </For>
        </Modal>
      </Show>
    </>
  );
}

export function EnrollmentList() {
  const q = createQuery(() => ({
    queryKey: ["enrollments"],
    queryFn: () => command<LocalEnrollment[]>("list_enrollments"),
  }));
  return (
    <>
      <Head title="注册申请" />
      <ErrorNotice error={q.error} />
      <table class="table">
        <thead>
          <tr>
            <th>请求 ID</th>
            <th>中心状态</th>
            <th>本地状态</th>
            <th>最近同步</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <For
            each={q.data}
            fallback={
              <tr>
                <td colspan="5">
                  <Empty>暂无注册申请</Empty>
                </td>
              </tr>
            }
          >
            {(e) => (
              <tr>
                <td class="mono">{short(e.id)}</td>
                <td>
                  <Show when={e.view} fallback="待确认提交">
                    {(v) => <Status value={v().status} />}
                  </Show>
                </td>
                <td>
                  {localStates[e.local_state] ?? e.local_state}
                  <Show when={e.error}>
                    <div class="sub">同步待恢复</div>
                  </Show>
                </td>
                <td>{date(e.synced_at)}</td>
                <td>
                  <A href={`/enrollments/${e.id}`} class="inline-link">
                    查看详情 →
                  </A>
                </td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
    </>
  );
}
export function EnrollmentDetail() {
  const params = useParams(),
    navigate = useNavigate(),
    action = useAction();
  const [cancel, setCancel] = createSignal(false);
  const q = createQuery(() => ({
    queryKey: ["enrollment", params.enrollmentId],
    queryFn: () =>
      command<LocalEnrollment>("get_enrollment", { id: params.enrollmentId }),
  }));
  const newDraft = async () => {
    const e = q.data;
    if (!e) return;
    const draft = await action.run<Draft>("create_enrollment_draft", {
      center_id: e.center_id,
    });
    if (!draft) return;
    draft.rows = Object.entries(e.claims);
    if (await action.run("save_enrollment_draft", draft))
      navigate(`/centers/${e.center_id}/enroll?draft=${draft.id}`);
  };
  return (
    <>
      <ErrorNotice error={q.error || action.error()} />
      <Show when={q.data}>
        {(d) => (
          <>
            <Head title="申请详情">
              <Button
                disabled={action.busy()}
                onClick={() =>
                  action.run("refresh_enrollment", { id: params.enrollmentId })
                }
              >
                刷新 / 重试原申请
              </Button>
              <Show when={d().view?.status === "PENDING_REVIEW"}>
                <Button danger onClick={() => setCancel(true)}>
                  取消申请
                </Button>
              </Show>
            </Head>
            <Show when={d().error}>
              <Notice danger>
                {d().error === "ISSUER_MISMATCH"
                  ? "中心指纹发生变化，已阻止传输。"
                  : d().error === "REGISTRATION_CLOSED"
                    ? `${registrationClosedMessage} 原申请已保留，自动重试已暂停；恢复注册后点击“刷新 / 重试原申请”。`
                    : "中心连接暂不可用，保留上次中心状态和本地恢复材料。"}
              </Notice>
            </Show>
            <Show
              when={
                d().local_state === "SUBMISSION_UNKNOWN" &&
                d().error !== "REGISTRATION_CLOSED"
              }
            >
              <Notice>
                提交结果未知。请重试原申请；钱包会使用原请求 ID、原 pending
                与原访问秘密。
              </Notice>
            </Show>
            <Show when={d().local_state === "CREDENTIAL_DELETED"}>
              <Notice danger>
                凭证已在本地删除。中心签发记录和成员名额仍保留，当前记录不能恢复原钱包秘密。
              </Notice>
            </Show>
            <Show when={d().local_state === "CREDENTIAL_READY"}>
              <Notice>
                凭证已解盲验签并加密保存，可以使用。
                <A class="btn primary" href={`/credentials/${d().id}`}>
                  查看凭证
                </A>
              </Notice>
            </Show>
            <Show when={d().local_state === "CREDENTIAL_SAVE_FAILED"}>
              <Notice danger>
                恢复材料已保留。
                <Button
                  onClick={() =>
                    action.run("retry_credential_save", { id: d().id })
                  }
                >
                  重试本地保存
                </Button>
              </Notice>
            </Show>
            <div class="status-grid">
              <div>
                <div class="label">本地恢复材料</div>
                <div class="state">
                  {d().recovery_saved ? "已加密保存" : "缺失"}
                </div>
              </div>
              <div>
                <div class="label">中心审批与签发</div>
                <Show when={d().view} fallback="尚未确认接收">
                  {(v) => <Status value={v().status} />}
                </Show>
              </div>
              <div>
                <div class="label">钱包凭证</div>
                <div class="state">{localStates[d().local_state]}</div>
              </div>
            </div>
            <div class="two-col">
              <section>
                <div class="kv">
                  <div>请求 ID</div>
                  <div class="mono">{d().id}</div>
                  <div>请求哈希</div>
                  <div class="mono">
                    {d().view?.request_hash ?? "保存在本地"}
                  </div>
                  <div>提交 / 截止</div>
                  <div>
                    {date(d().view?.submitted_at)} /{" "}
                    {date(d().view?.review_deadline)}
                  </div>
                  <div>最近同步</div>
                  <div>{date(d().synced_at)}</div>
                </div>
                <h2 class="section-title">已提交属性 · 内容固定</h2>
                <ClaimsTable claims={d().claims} />
                <Show when={d().view?.public_reason}>
                  <Notice>{d().view?.public_reason}</Notice>
                </Show>
                <Show
                  when={[
                    "CHANGES_REQUIRED",
                    "REJECTED",
                    "EXPIRED",
                    "CANCELLED",
                    "INVALID_REQUEST",
                  ].includes(d().view?.status ?? "")}
                >
                  <Button primary onClick={newDraft}>
                    从当前属性创建新草稿
                  </Button>
                </Show>
              </section>
              <aside>
                <h2 class="section-title">下一步</h2>
                <Notice>
                  审批通过、中心签发、本地凭证可用和应用登录是独立阶段。等待期间可关闭钱包，解锁后继续恢复。
                </Notice>
                <Show when={d().local_state === "CREDENTIAL_READY"}>
                  <A class="btn primary" href="/login/new">
                    返回应用获取新挑战
                  </A>
                </Show>
              </aside>
            </div>
            <Show when={cancel()}>
              <Modal
                title="取消待审申请"
                onClose={() => setCancel(false)}
                footer={
                  <>
                    <Button onClick={() => setCancel(false)}>返回</Button>
                    <Button
                      danger
                      disabled={action.busy()}
                      onClick={async () => {
                        await action.run("cancel_enrollment", { id: d().id });
                        setCancel(false);
                      }}
                    >
                      确认取消
                    </Button>
                  </>
                }
              >
                <p>
                  仅当中心仍未批准时可取消。与管理员审批竞争时，以服务端事务结果为准。
                </p>
              </Modal>
            </Show>
          </>
        )}
      </Show>
    </>
  );
}
