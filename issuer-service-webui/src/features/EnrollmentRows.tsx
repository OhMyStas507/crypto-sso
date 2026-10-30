import { For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { type Enrollment } from "../api";
import { Empty, Icon, Status, date, short } from "../ui";

export function EnrollmentRows(p: { entries: Enrollment[] }) {
  return (
    <table class="table">
      <thead>
        <tr>
          <th>申请 / 成员</th>
          <th>提交时间</th>
          <th>属性</th>
          <th>审批与签发状态</th>
          <th />
        </tr>
      </thead>
      <tbody>
        <For
          each={p.entries}
          fallback={
            <tr>
              <td colspan="5">
                <Empty>暂无记录</Empty>
              </td>
            </tr>
          }
        >
          {(e) => (
            <tr>
              <td>
                <A href={`/enrollments/${e.view.request_id}`}>
                  <div class="strong">
                    {e.claims.name || e.decision?.member_ref || "未命名申请"}
                  </div>
                  <div class="sub mono">{short(e.view.request_id)}</div>
                </A>
              </td>
              <td>{date(e.view.submitted_at)}</td>
              <td>{Object.keys(e.claims).length} 项</td>
              <td>
                <Status value={e.view.status} />
                <Show when={e.failure_code}>
                  <div class="sub">{e.failure_code}</div>
                </Show>
              </td>
              <td>
                <A
                  class="inline-link"
                  href={`/enrollments/${e.view.request_id}`}
                >
                  查看 <Icon name="chevron-right" />
                </A>
              </td>
            </tr>
          )}
        </For>
      </tbody>
    </table>
  );
}
