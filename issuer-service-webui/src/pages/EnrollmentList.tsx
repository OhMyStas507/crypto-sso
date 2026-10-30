import { For, Show, createSignal } from "solid-js";
import { useLocation } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import type { Page } from "@crypto-sso/identity-api-types";
import { api, type Enrollment } from "../api";
import { Button, ErrorNotice, Head, states } from "../ui";
import { EnrollmentRows } from "../features/EnrollmentRows";

export function EnrollmentList() {
  const location = useLocation();
  const issuance = () => location.pathname.endsWith("/issuances");
  const [status, setStatus] = createSignal("");
  const [search, setSearch] = createSignal("");
  const [cursor, setCursor] = createSignal("0");
  const q = createQuery(() => ({
    queryKey: ["enrollments", issuance(), status(), search(), cursor()],
    queryFn: () =>
      api<Page<Enrollment>>(
        `${issuance() ? "issuances" : "enrollments"}?${new URLSearchParams({ ...(status() ? { status: status() } : {}), q: search(), cursor: cursor() })}`,
      ),
    refetchInterval: 10000,
  }));
  return (
    <>
      <Head title={issuance() ? "签发台账" : "注册申请"}>
        <Button onClick={() => q.refetch()}>刷新</Button>
      </Head>
      <div class="toolbar">
        <input
          class="input"
          aria-label="检索申请"
          placeholder="搜索请求 ID 或属性值"
          value={search()}
          onInput={(e) => {
            setSearch(e.currentTarget.value);
            setCursor("0");
          }}
        />
        <select
          class="input"
          aria-label="状态筛选"
          value={status()}
          onChange={(e) => {
            setStatus(e.currentTarget.value);
            setCursor("0");
          }}
        >
          <option value="">全部状态</option>
          <For each={Object.entries(states)}>
            {([value, label]) => <option value={value}>{label}</option>}
          </For>
        </select>
      </div>
      <ErrorNotice error={q.error} />
      <Show when={q.data}>
        {(data) => (
          <>
            <EnrollmentRows entries={data().items} />
            <div class="pagination">
              <span>共 {data().total} 条</span>
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
                  disabled={!data().next_cursor}
                  onClick={() => setCursor(data().next_cursor!)}
                >
                  下一页
                </Button>
              </div>
            </div>
          </>
        )}
      </Show>
    </>
  );
}
