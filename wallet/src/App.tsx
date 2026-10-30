import {
  For,
  Show,
  createEffect,
  onCleanup,
  onMount,
  type ParentProps,
} from "solid-js";
import {
  HashRouter,
  Route,
  A,
  useLocation,
  useNavigate,
} from "@solidjs/router";
import {
  QueryClient,
  QueryClientProvider,
  createQuery,
} from "@tanstack/solid-query";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import { command, type WalletStatus } from "./bridge";
import { Icon, ErrorNotice } from "./ui";
import { Welcome, Unlock, Home, Preferences, NotFound } from "./pages/Home";
import { CenterList, AddCenter, CenterDetail } from "./pages/Centers";
import {
  DraftPage,
  EnrollmentList,
  EnrollmentDetail,
} from "./pages/Enrollments";
import { CredentialList, CredentialDetail } from "./pages/Credentials";
import { Apps, AppEditor, StartLogin, LoginTaskPage } from "./pages/Login";
import "./design.css";
import "bootstrap-icons/font/bootstrap-icons.css";
import "./App.css";

export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 1000 } },
});
const nav = [
  ["/", "grid-1x2", "首页"],
  ["/credentials", "person-vcard", "我的凭证"],
  ["/enrollments", "inbox", "注册申请"],
  ["/centers", "buildings", "身份中心"],
  ["/apps", "app-indicator", "受信应用"],
];

function Root(p: ParentProps) {
  const location = useLocation(),
    navigate = useNavigate();
  const status = createQuery(() => ({
    queryKey: ["wallet-status"],
    queryFn: () => command<WalletStatus>("get_wallet_status"),
    refetchInterval: 1000,
  }));
  let epoch = -1;
  createEffect(() => {
    const s = status.data;
    if (!s) return;
    if (epoch !== s.epoch) {
      const hadSession = epoch !== -1;
      epoch = s.epoch;
      // Initial unlocked queries may already be loading. Evict only a revoked session.
      if (hadSession && !s.unlocked) {
        queryClient.removeQueries({
          predicate: (q) => q.queryKey[0] !== "wallet-status",
        });
      }
    }
    if (!s.initialized && location.pathname !== "/welcome")
      navigate("/welcome", { replace: true });
    else if (s.initialized && !s.unlocked && location.pathname !== "/unlock")
      navigate("/unlock", { replace: true });
    else if (s.unlocked && ["/welcome", "/unlock"].includes(location.pathname))
      navigate("/", { replace: true });
  });
  const unlisten: (() => void)[] = [];
  let disposed = false;
  onMount(async () => {
    if (!isTauri()) return;
    for (const event of [
      "wallet://lock-changed",
      "wallet://enrollment-changed",
    ]) {
      const stop = await listen(event, () => {
        void queryClient.invalidateQueries();
      });
      if (disposed) stop();
      else unlisten.push(stop);
    }
  });
  let lastTouch = 0;
  const touch = () => {
    if (status.data?.unlocked && Date.now() - lastTouch > 5000) {
      lastTouch = Date.now();
      void command("touch_wallet").catch(() => {});
    }
  };
  onMount(() => {
    window.addEventListener("pointerdown", touch);
    window.addEventListener("keydown", touch);
  });
  onCleanup(() => {
    window.removeEventListener("pointerdown", touch);
    window.removeEventListener("keydown", touch);
    disposed = true;
    unlisten.forEach((f) => f());
  });
  return (
    <Show
      when={!status.error}
      fallback={
        <div class="auth">
          <div class="auth-brand">
            <Icon name="shield-lock" />
            身份钱包
          </div>
          <div class="auth-main">
            <ErrorNotice error={status.error} />
          </div>
        </div>
      }
    >
      <Show
        when={status.data}
        fallback={<div class="loading">正在打开本地钱包…</div>}
      >
        <Show when={status.data?.unlocked} fallback={p.children}>
          <div class="app wallet">
            <aside class="sidebar">
              <div class="brand">
                <span class="brandmark">
                  <Icon name="shield-lock" />
                </span>
                身份钱包
              </div>
              <For each={nav}>
                {([href, icon, label], i) => (
                  <>
                    <Show when={i() === 3}>
                      <div class="nav-caption">连接</div>
                    </Show>
                    <A
                      href={href}
                      end={href === "/"}
                      class="nav-item"
                      activeClass="selected"
                    >
                      <Icon name={icon} />
                      {label}
                    </A>
                  </>
                )}
              </For>
              <div class="nav-spacer" />
              <A class="nav-item" activeClass="selected" href="/settings">
                <Icon name="gear" />
                本地设置
              </A>
              <button
                class="nav-item nav-button"
                onClick={async () => {
                  await command("lock_wallet");
                  queryClient.removeQueries({
                    predicate: (q) => q.queryKey[0] !== "wallet-status",
                  });
                  await queryClient.invalidateQueries({
                    queryKey: ["wallet-status"],
                  });
                  navigate("/unlock");
                }}
              >
                <Icon name="lock" />
                锁定钱包
              </button>
              <div class="sidebar-bottom">
                <span class="avatar">本</span>
                <div>
                  本地钱包<div class="tiny muted">Windows 安全保护</div>
                </div>
                <Icon name="shield-check" />
              </div>
            </aside>
            <main class="workspace">
              <header class="topbar">
                <Icon name="shield-lock" />
                <span class="crumb">个人空间</span>
                <span class="slash">/</span>
                {nav.find(
                  ([href]) =>
                    href !== "/" && location.pathname.startsWith(href),
                )?.[2] ?? "身份钱包"}
                <div class="top-right">
                  <Icon name="unlock" />
                  已解锁
                </div>
              </header>
              <div class="content">{p.children}</div>
            </main>
          </div>
        </Show>
      </Show>
    </Show>
  );
}
export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <HashRouter root={Root}>
        <Route path="/welcome" component={Welcome} />
        <Route path="/unlock" component={Unlock} />
        <Route path="/" component={Home} />
        <Route path="/centers" component={CenterList} />
        <Route path="/centers/add" component={AddCenter} />
        <Route path="/centers/:centerId" component={CenterDetail} />
        <Route path="/centers/:centerId/enroll" component={DraftPage} />
        <Route path="/enrollments" component={EnrollmentList} />
        <Route path="/enrollments/:enrollmentId" component={EnrollmentDetail} />
        <Route path="/credentials" component={CredentialList} />
        <Route path="/credentials/:credentialId" component={CredentialDetail} />
        <Route path="/apps" component={Apps} />
        <Route path="/apps/new" component={AppEditor} />
        <Route path="/apps/:appId/edit" component={AppEditor} />
        <Route path="/login/new" component={StartLogin} />
        <Route path="/login/:taskId" component={LoginTaskPage} />
        <Route path="/settings" component={Preferences} />
        <Route path="*" component={NotFound} />
      </HashRouter>
    </QueryClientProvider>
  );
}
