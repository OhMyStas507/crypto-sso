import { For, Show, type ParentProps } from "solid-js";
import { Router, Route, A, Navigate, useLocation } from "@solidjs/router";
import {
  QueryClient,
  QueryClientProvider,
  createQuery,
} from "@tanstack/solid-query";
import { api, setupApi, type Session } from "./api";
import type { SetupStatus } from "@crypto-sso/identity-api-types";
import { Icon, ErrorNotice, Button } from "./ui";
import { SetupPage } from "./pages/SetupPage";
import {
  Login,
  OverviewPage,
  EnrollmentList,
  ReviewPage,
  MembersPage,
  MemberPage,
  AuditPage,
  SettingsPage,
  IssuerPage,
  NotFound,
} from "./pages";
import "./design.css";
import "bootstrap-icons/font/bootstrap-icons.css";
import "./App.css";

const client = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 5000 } },
});
const nav = [
  ["/overview", "grid-1x2", "概览"],
  ["/enrollments", "inbox", "注册申请"],
  ["/members", "people", "成员"],
  ["/issuances", "file-earmark-check", "签发台账"],
  ["/settings/service", "sliders2", "服务设置"],
  ["/settings/claims", "list-ul", "Claims 默认建议"],
  ["/settings/issuer", "fingerprint", "签发身份"],
  ["/audit", "clock-history", "审计记录"],
];
function CenterGate(p: ParentProps) {
  const location = useLocation();
  const status = createQuery(() => ({
    queryKey: ["setup-status"],
    queryFn: () => setupApi<SetupStatus>("status"),
  }));
  return (
    <Show
      when={location.pathname.startsWith("/admin/setup")}
      fallback={
        <Show
          when={status.data}
          fallback={
            <div class="loading">
              <ErrorNotice error={status.error} />
              <Show when={status.error} fallback="正在检查身份中心状态…">
                <Button onClick={() => void status.refetch()}>重新检查</Button>
              </Show>
            </div>
          }
        >
          <Show
            when={status.data?.state === "initialized"}
            fallback={<Navigate href="/setup" />}
          >
            {p.children}
          </Show>
        </Show>
      }
    >
      {p.children}
    </Show>
  );
}
function Layout(p: ParentProps) {
  const session = createQuery(() => ({
    queryKey: ["admin-session"],
    queryFn: () => api<Session>("session"),
  }));
  const location = useLocation();
  const title = () =>
    nav.find(([path]) => location.pathname.startsWith(`/admin${path}`))?.[2] ??
    "身份中心";
  const logout = async () => {
    await api("session", "DELETE");
    client.clear();
    window.location.assign("/admin/login");
  };
  return (
    <Show
      when={session.data}
      fallback={
        <div class="loading">
          <ErrorNotice error={session.error} />
          <Show when={!session.error}>正在验证管理员会话…</Show>
        </div>
      }
    >
      <div class="app">
        <aside class="sidebar">
          <div class="brand">
            <span class="brandmark">
              <Icon name="intersect" />
            </span>
            身份中心
          </div>
          <For each={nav}>
            {([href, icon, label], i) => (
              <>
                <Show when={i() === 4}>
                  <div class="nav-caption">管理</div>
                </Show>
                <A href={href} class="nav-item" activeClass="selected">
                  <Icon name={icon} />
                  {label}
                </A>
              </>
            )}
          </For>
          <div class="nav-spacer" />
          <div class="sidebar-bottom">
            <span class="avatar">AD</span>
            <div>
              管理员<div class="tiny muted">{session.data?.username}</div>
            </div>
            <button class="icon-button" aria-label="退出登录" onClick={logout}>
              <Icon name="box-arrow-right" />
            </button>
          </div>
        </aside>
        <main class="workspace">
          <header class="topbar">
            <Icon name="buildings" />
            <span class="crumb">身份中心</span>
            <span class="slash">/</span>
            {title()}
            <div class="top-right">
              {new Date().toLocaleDateString("zh-CN")}
            </div>
          </header>
          <div class="content">{p.children}</div>
        </main>
      </div>
    </Show>
  );
}
export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Router base="/admin" root={CenterGate}>
        <Route path="/setup/:step?" component={SetupPage} />
        <Route path="/login" component={Login} />
        <Route component={Layout}>
          <Route path="/" component={() => <Navigate href="/overview" />} />
          <Route path="/overview" component={OverviewPage} />
          <Route path="/enrollments" component={EnrollmentList} />
          <Route path="/enrollments/:requestId" component={ReviewPage} />
          <Route path="/members" component={MembersPage} />
          <Route path="/members/:memberRef" component={MemberPage} />
          <Route path="/issuances" component={EnrollmentList} />
          <Route path="/settings/service" component={SettingsPage} />
          <Route path="/settings/claims" component={SettingsPage} />
          <Route path="/settings/issuer" component={IssuerPage} />
          <Route path="/audit" component={AuditPage} />
          <Route path="*" component={NotFound} />
        </Route>
      </Router>
    </QueryClientProvider>
  );
}
