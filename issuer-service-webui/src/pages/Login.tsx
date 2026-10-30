import { createSignal } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { api } from "../api";
import { Button, ErrorNotice, Field, Icon } from "../ui";
import { useWrite } from "../features/useWrite";

export function Login() {
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const write = useWrite();
  const [bootstrapError, setBootstrapError] = createSignal<unknown>();
  const navigate = useNavigate();
  const submit = async (e: SubmitEvent) => {
    e.preventDefault();
    setBootstrapError(undefined);
    try {
      await api("csrf");
    } catch (error) {
      setBootstrapError(error);
      return;
    }
    if (
      await write.run("sessions", "POST", {
        username: username(),
        password: password(),
      })
    ) {
      setPassword("");
      navigate("/overview");
    }
  };
  return (
    <div class="auth">
      <div class="auth-brand">
        <Icon name="intersect" />
        身份中心
      </div>
      <form class="auth-main" onSubmit={submit}>
        <h1>管理员登录</h1>
        <ErrorNotice error={write.error() || bootstrapError()} />
        <Field label="管理员账号">
          <input
            class="input"
            required
            autocomplete="username"
            value={username()}
            onInput={(e) => setUsername(e.currentTarget.value)}
          />
        </Field>
        <Field label="密码">
          <input
            class="input"
            type="password"
            required
            autocomplete="current-password"
            value={password()}
            onInput={(e) => setPassword(e.currentTarget.value)}
          />
        </Field>
        <Button primary type="submit" disabled={write.busy()}>
          {write.busy() ? "正在登录…" : "登录"}
        </Button>
        <div class="auth-info">
          仅限授权管理员访问。
          <br />
          管理会话与钱包申请相互独立。
        </div>
      </form>
      <footer class="auth-foot">
        <span>Crypto SSO · 身份中心</span>
        <span>管理控制台</span>
      </footer>
    </div>
  );
}
