import { For, createSignal } from "solid-js";
import { A } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import type { Page } from "@crypto-sso/identity-api-types";
import { api, type Member } from "../api";
import { Button, Empty, ErrorNotice, Head, date } from "../ui";

export function MembersPage() {
  const [search, setSearch] = createSignal(""),
    [cursor, setCursor] = createSignal("0");
  const q = createQuery(() => ({
    queryKey: ["members", search(), cursor()],
    queryFn: () =>
      api<Page<Member>>(
        `members?${new URLSearchParams({ q: search(), cursor: cursor() })}`,
      ),
  }));
  return (
    <>
      <Head title="成员" />
      <div class="toolbar">
        <input
          class="input"
          placeholder="搜索内部成员标识"
          aria-label="搜索成员"
          value={search()}
          onInput={(e) => {
            setSearch(e.currentTarget.value);
            setCursor("0");
          }}
        />
      </div>
      <ErrorNotice error={q.error} />
      <table class="table">
        <thead>
          <tr>
            <th>成员标识</th>
            <th>核验来源</th>
            <th>核验时间</th>
            <th>签发名额</th>
          </tr>
        </thead>
        <tbody>
          <For
            each={q.data?.items}
            fallback={
              <tr>
                <td colspan="4">
                  <Empty>批准申请后建立可信成员记录</Empty>
                </td>
              </tr>
            }
          >
            {(m) => (
              <tr>
                <td>
                  <A
                    class="inline-link"
                    href={`/members/${encodeURIComponent(m.member_ref)}`}
                  >
                    {m.member_ref}
                  </A>
                </td>
                <td>{m.evidence_source}</td>
                <td>{date(m.verified_at)}</td>
                <td>{m.issued ? "已签发" : "已保留"}</td>
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
