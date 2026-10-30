import { Show } from "solid-js";
import { useParams } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import { api, type Enrollment, type Member } from "../api";
import { ErrorNotice, Head, Notice, date } from "../ui";
import { EnrollmentRows } from "../features/EnrollmentRows";

export function MemberPage() {
  const params = useParams();
  const q = createQuery(() => ({
    queryKey: ["member", params.memberRef],
    queryFn: () =>
      api<{ member: Member; enrollment: Enrollment }>(
        `members/${encodeURIComponent(params.memberRef!)}`,
      ),
  }));
  return (
    <>
      <ErrorNotice error={q.error} />
      <Show when={q.data}>
        {(d) => (
          <>
            <Head title={d().member.member_ref} />
            <div class="kv">
              <div>可信资料来源</div>
              <div>{d().member.evidence_source}</div>
              <div>核验说明</div>
              <div>{d().member.verification_note}</div>
              <div>核验时间</div>
              <div>{date(d().member.verified_at)}</div>
              <div>签发名额</div>
              <div>{d().member.issued ? "已签发" : "已保留"}</div>
            </div>
            <h2 class="section-title">关联申请</h2>
            <EnrollmentRows entries={[d().enrollment]} />
            <Notice>
              当前 SDK 不支持凭证撤销或自动补发。删除钱包凭证不会释放成员名额。
            </Notice>
          </>
        )}
      </Show>
    </>
  );
}
