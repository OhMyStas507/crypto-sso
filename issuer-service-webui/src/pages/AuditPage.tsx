import { For, createSignal } from "solid-js";
import { createQuery } from "@tanstack/solid-query";
import type { AuditEvent, Page } from "@crypto-sso/identity-api-types";
import { api } from "../api";
import { Button, Empty, ErrorNotice, Head, date } from "../ui";

export function AuditPage() {
  const [cursor, setCursor] = createSignal("0");
  const q = createQuery(() => ({
    queryKey: ["audit", cursor()],
    queryFn: () => api<Page<AuditEvent>>(`audit-events?cursor=${cursor()}`),
  }));
  return (
    <>
      <Head title="审计记录">
        <Button onClick={() => q.refetch()}>刷新</Button>
      </Head>
      <ErrorNotice error={q.error} />
      <table class="table">
        <thead>
          <tr>
            <th>时间</th>
            <th>操作者</th>
            <th>动作</th>
            <th>对象</th>
          </tr>
        </thead>
        <tbody>
          <For
            each={q.data?.items}
            fallback={
              <tr>
                <td colspan="4">
                  <Empty>暂无审计事件</Empty>
                </td>
              </tr>
            }
          >
            {(e) => (
              <tr>
                <td>{date(e.at)}</td>
                <td>{e.actor}</td>
                <td>{e.action}</td>
                <td class="mono">{e.target}</td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
      <div class="pagination">
        <span>共 {q.data?.total ?? 0} 条</span>
        <div class="actions">
          <Button
            disabled={cursor() === "0"}
            onClick={() =>
              setCursor(String(Math.max(0, Number(cursor()) - 25)))
            }
          >
            上一页
          </Button>
          <Button
            disabled={!q.data?.next_cursor}
            onClick={() => setCursor(q.data!.next_cursor!)}
          >
            下一页
          </Button>
        </div>
      </div>
    </>
  );
}
