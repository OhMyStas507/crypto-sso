import { For, Show, createSignal } from "solid-js";
import { useParams } from "@solidjs/router";
import { createQuery, useQueryClient } from "@tanstack/solid-query";
import type { Page, ReviewDecision } from "@crypto-sso/identity-api-types";
import { api, type Enrollment, type Member } from "../api";
import {
  Button,
  ClaimsTable,
  ErrorNotice,
  Field,
  Head,
  Modal,
  Notice,
  Status,
  date,
} from "../ui";
import { useWrite } from "../features/useWrite";

export function ReviewPage() {
  const params = useParams();
  const client = useQueryClient();
  const write = useWrite();
  const q = createQuery(() => ({
    queryKey: ["enrollment", params.requestId],
    queryFn: () => api<Enrollment>(`enrollments/${params.requestId}`),
    refetchInterval: 5000,
  }));
  const [member, setMember] = createSignal(""),
    [source, setSource] = createSignal(""),
    [note, setNote] = createSignal(""),
    [internal, setInternal] = createSignal(""),
    [confirmed, setConfirmed] = createSignal(false);
  const [dialog, setDialog] = createSignal(""),
    [reason, setReason] = createSignal("");
  const members = createQuery(() => ({
    queryKey: ["member-selector"],
    queryFn: () => api<Page<Member>>("members"),
  }));
  const decide = async () => {
    const entry = q.data;
    if (!entry) return;
    const decision: ReviewDecision = {
      expected_revision: entry.view.revision,
      request_hash: entry.view.request_hash,
      member_ref: member() || null,
      evidence_source: source() || null,
      verification_note: note() || null,
      internal_note: internal() || null,
      approved_claims: dialog() === "approve" ? entry.claims : null,
      claims_confirmed: confirmed(),
      public_reason: reason() || null,
    };
    if (
      await write.run(
        `enrollments/${params.requestId}/${dialog()}`,
        "POST",
        decision,
      )
    ) {
      setDialog("");
      await q.refetch();
      await client.invalidateQueries({ queryKey: ["overview"] });
    }
  };
  return (
    <>
      <ErrorNotice error={q.error} />
      <Show when={q.data}>
        {(data) => (
          <>
            <Head title={data().claims.name || "注册申请"}>
              <Status value={data().view.status} />
              <Button
                onClick={() => navigator.clipboard.writeText(params.requestId!)}
              >
                复制请求 ID
              </Button>
            </Head>
            <Show when={data().failure_code}>
              <Notice danger>
                签发未完成：{data().failure_code}
                。批准记录已保存，可重试同一申请。
              </Notice>
            </Show>
            <Show when={data().view.public_reason}>
              <Notice>{data().view.public_reason}</Notice>
            </Show>
            <div class="two-col">
              <div>
                <section class="section">
                  <h2 class="section-title">申请内容</h2>
                  <div class="kv">
                    <div>请求 ID</div>
                    <div class="mono">{data().view.request_id}</div>
                    <div>请求哈希</div>
                    <div class="mono">{data().view.request_hash}</div>
                    <div>提交 / 截止</div>
                    <div>
                      {date(data().view.submitted_at)} /{" "}
                      {date(data().view.review_deadline)}
                    </div>
                    <div>建议快照</div>
                    <div>
                      {data().hints_snapshot
                        ? `r${data().claim_hints_revision} · 提交后内容固定`
                        : "来源快照不可用"}
                    </div>
                  </div>
                </section>
                <ClaimsTable claims={data().claims} />
                <p class="note">
                  核准范围必须与本次完整申请一致。需要调整属性时，请退回修改。
                </p>
                <Show when={data().hints_snapshot}>
                  <h2 class="section-title">提交时建议对比</h2>
                  <table class="table">
                    <thead>
                      <tr>
                        <th>建议属性</th>
                        <th>建议值</th>
                        <th>实际提交</th>
                      </tr>
                    </thead>
                    <tbody>
                      <For each={data().hints_snapshot ?? []}>
                        {(h) => (
                          <tr>
                            <td class="mono">{h.key}</td>
                            <td>{h.default_value ?? "空字符串"}</td>
                            <td>
                              {Object.hasOwn(data().claims, h.key)
                                ? data().claims[h.key] || "空字符串"
                                : "已删除"}
                            </td>
                          </tr>
                        )}
                      </For>
                      <For
                        each={Object.keys(data().claims).filter(
                          (k) =>
                            !data().hints_snapshot?.some((h) => h.key === k),
                        )}
                      >
                        {(k) => (
                          <tr>
                            <td class="mono">{k}</td>
                            <td>自定义</td>
                            <td>{data().claims[k]}</td>
                          </tr>
                        )}
                      </For>
                    </tbody>
                  </table>
                </Show>
                <section class="section">
                  <h2 class="section-title">申请说明</h2>
                  <p class="secondary preserve">
                    {data().application_note || "未填写"}
                  </p>
                </section>
                <h2 class="section-title">处理历史</h2>
                <div class="timeline">
                  <div class="timeline-item active">
                    <Status value={data().view.status} />
                    <div class="time">状态修订 r{data().view.revision}</div>
                  </div>
                  <For each={data().history}>
                    {(entry) => (
                      <div class="timeline-item">
                        {entry.actor} · {entry.action}
                        <div class="time">{date(entry.at)}</div>
                        <Show when={entry.decision.public_reason}>
                          <p class="hint preserve">
                            {entry.decision.public_reason}
                          </p>
                        </Show>
                      </div>
                    )}
                  </For>
                  <div class="timeline-item">
                    申请已接收
                    <div class="time">{date(data().view.submitted_at)}</div>
                  </div>
                </div>
              </div>
              <aside>
                <h2 class="section-title">身份核验</h2>
                <Show
                  when={data().view.status === "PENDING_REVIEW"}
                  fallback={
                    <>
                      <div class="kv">
                        <div>核验成员</div>
                        <div>{data().decision?.member_ref ?? "—"}</div>
                        <div>可信来源</div>
                        <div>{data().decision?.evidence_source ?? "—"}</div>
                        <div>核验说明</div>
                        <div>{data().decision?.verification_note ?? "—"}</div>
                      </div>
                      <Show
                        when={["APPROVED", "ISSUE_FAILED"].includes(
                          data().view.status,
                        )}
                      >
                        <div class="actions">
                          <Show when={data().view.status === "ISSUE_FAILED"}>
                            <Button
                              primary
                              onClick={() => setDialog("retry-issuance")}
                            >
                              重试签发
                            </Button>
                          </Show>
                          <Button danger onClick={() => setDialog("reject")}>
                            终止签发
                          </Button>
                        </div>
                      </Show>
                    </>
                  }
                >
                  <Field
                    label="内部成员标识"
                    hint="由可信资料确认，批准事务会建立并保留签发名额。"
                  >
                    <input
                      class="input"
                      list="members"
                      value={member()}
                      onInput={(e) => setMember(e.currentTarget.value)}
                    />
                    <datalist id="members">
                      <For each={members.data?.items}>
                        {(m) => (
                          <option value={m.member_ref}>已保留名额</option>
                        )}
                      </For>
                    </datalist>
                  </Field>
                  <Field label="可信资料来源">
                    <input
                      class="input"
                      value={source()}
                      onInput={(e) => setSource(e.currentTarget.value)}
                    />
                  </Field>
                  <Field label="核验说明">
                    <textarea
                      class="input"
                      value={note()}
                      onInput={(e) => setNote(e.currentTarget.value)}
                    />
                  </Field>
                  <Field label="内部备注">
                    <textarea
                      class="input"
                      value={internal()}
                      onInput={(e) => setInternal(e.currentTarget.value)}
                    />
                  </Field>
                  <label class="check-label">
                    <input
                      type="checkbox"
                      checked={confirmed()}
                      onChange={(e) => setConfirmed(e.currentTarget.checked)}
                    />
                    已核验申请人的全部 {Object.keys(data().claims).length}{" "}
                    项属性及成员资格。
                  </label>
                  <div class="actions">
                    <Button onClick={() => setDialog("request-changes")}>
                      退回
                    </Button>
                    <Button danger onClick={() => setDialog("reject")}>
                      拒绝
                    </Button>
                    <Button
                      primary
                      disabled={
                        !confirmed() || !member() || !source() || !note()
                      }
                      onClick={() => setDialog("approve")}
                    >
                      通过申请
                    </Button>
                  </div>
                </Show>
              </aside>
            </div>
            <Show when={dialog()}>
              <Modal
                title={
                  dialog() === "approve"
                    ? "确认批准完整申请"
                    : dialog() === "request-changes"
                      ? "退回修改"
                      : dialog() === "retry-issuance"
                        ? "重试原申请签发"
                        : "拒绝或终止申请"
                }
                onClose={() => {
                  setDialog("");
                  write.clear();
                }}
                footer={
                  <>
                    <Button onClick={() => setDialog("")}>取消</Button>
                    <Button
                      primary
                      disabled={
                        write.busy() ||
                        (["reject", "request-changes"].includes(dialog()) &&
                          !reason().trim())
                      }
                      onClick={decide}
                    >
                      {write.busy() ? "处理中…" : "确认"}
                    </Button>
                  </>
                }
              >
                <ErrorNotice error={write.error()} />
                <Show
                  when={dialog() === "approve"}
                  fallback={
                    <Show when={dialog() !== "retry-issuance"}>
                      <Field label="公开理由 / 修改建议">
                        <textarea
                          class="input"
                          value={reason()}
                          onInput={(e) => setReason(e.currentTarget.value)}
                        />
                      </Field>
                    </Show>
                  }
                >
                  <p>
                    批准后保留成员 {member()}{" "}
                    的签发名额。中心签发完成后，钱包仍需领取、解盲并保存凭证。
                  </p>
                  <ClaimsTable claims={data().claims} />
                </Show>
              </Modal>
            </Show>
          </>
        )}
      </Show>
    </>
  );
}
