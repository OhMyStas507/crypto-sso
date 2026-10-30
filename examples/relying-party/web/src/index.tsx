import { render } from "solid-js/web";
import { Show, createSignal, onMount } from "solid-js";
import "./style.css";
function App() {
  const [context, setContext] = createSignal<{
    csrf_token: string;
    origin: string;
    issuer_id: string;
    required_claims: string[];
  }>();
  const [challenge, setChallenge] = createSignal(""),
    [proof, setProof] = createSignal(""),
    [session, setSession] = createSignal<{
      subject: string;
      claims: Record<string, string>;
    } | null>(null),
    [error, setError] = createSignal(""),
    [busy, setBusy] = createSignal(false);
  async function request(path: string, method = "GET", body?: string) {
    const r = await fetch(`/api/v1/login/${path}`, {
      method,
      credentials: "same-origin",
      headers: {
        "content-type": "application/json",
        "x-csrf-token": context()?.csrf_token ?? "",
      },
      body,
    });
    const value = await r
      .json()
      .catch(() => ({ message: "请求被拒绝，请重新加载页面" }));
    if (!r.ok) throw new Error(value.message);
    return value;
  }
  async function act(action: () => Promise<void>) {
    setError("");
    setBusy(true);
    try {
      await action();
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(false);
    }
  }
  onMount(() => {
    void act(async () => {
      setContext(await request("context"));
      try {
        setSession(await request("session"));
      } catch {
        setSession(null);
      }
    });
  });
  return (
    <main>
      <header>
        <span class="mark">◫</span>Crypto SSO{" "}
        <span class="muted">应用登录演示</span>
      </header>
      <Show
        when={!session()}
        fallback={
          <>
            <h1>应用登录成功</h1>
            <p>身份来自已经验证的证明，中心没有参与本次登录。</p>
            <label>应用内 Subject</label>
            <pre>{session()?.subject}</pre>
            <label>已验证且披露的属性</label>
            <pre>{JSON.stringify(session()?.claims, null, 2)}</pre>
            <button
              disabled={busy()}
              onClick={() =>
                act(async () => {
                  await request("session", "DELETE");
                  setSession(null);
                  setContext(await request("context"));
                })
              }
            >
              退出应用
            </button>
          </>
        }
      >
        <h1>使用身份钱包登录</h1>
        <p>将公开挑战交给钱包，再将钱包生成的公开证明交回当前浏览器。</p>
        <dl>
          <dt>当前应用 Origin</dt>
          <dd>{context()?.origin}</dd>
          <dt>固定中心指纹</dt>
          <dd>{context()?.issuer_id}</dd>
          <dt>必需披露</dt>
          <dd>{context()?.required_claims.join("、") || "无"}</dd>
        </dl>
        <section>
          <h2>1 获取公开挑战</h2>
          <button
            disabled={busy() || !context()}
            onClick={() =>
              act(async () => {
                setChallenge(
                  (await request("challenges", "POST", "{}")).challenge_json,
                );
              })
            }
          >
            获取 / 更新挑战
          </button>
          <Show when={challenge()}>
            <textarea readonly value={challenge()} />
            <button
              class="secondary"
              onClick={() => navigator.clipboard.writeText(challenge())}
            >
              复制挑战
            </button>
            <p class="muted">
              挑战有效期为 120
              秒。首次领证后请重新获取挑战。此页面需保持在同一浏览器中。
            </p>
          </Show>
        </section>
        <section>
          <h2>2 提交钱包公开证明</h2>
          <textarea
            placeholder="粘贴钱包生成的 Presentation JSON"
            value={proof()}
            onInput={(e) => setProof(e.currentTarget.value)}
          />
          <button
            disabled={busy() || !proof()}
            onClick={() =>
              act(async () => {
                setSession(await request("proofs", "POST", proof()));
                setProof("");
              })
            }
          >
            {busy() ? "处理中…" : "验证证明并登录"}
          </button>
        </section>
      </Show>
      <Show when={error()}>
        <p class="error" role="alert">
          {error()}
        </p>
      </Show>
      <footer>
        Cookie 与会话由 Salvo 中间件管理。钱包不会获取浏览器 Cookie。
      </footer>
    </main>
  );
}
render(() => <App />, document.getElementById("root")!);
