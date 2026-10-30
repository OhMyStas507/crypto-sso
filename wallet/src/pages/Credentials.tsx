import { For, Show, createSignal } from "solid-js";
import { A, useNavigate, useParams } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import { command, type Center, type Credential } from "../bridge";
import { useAction } from "../actions";
import {
  Button,
  ClaimsTable,
  Empty,
  ErrorNotice,
  Head,
  Icon,
  Modal,
  Notice,
  date,
} from "../ui";

export function CredentialList() {
  const q = createQuery(() => ({
    queryKey: ["credentials"],
    queryFn: () => command<Credential[]>("list_credentials"),
  }));
  const centers = createQuery(() => ({
    queryKey: ["centers"],
    queryFn: () => command<Center[]>("list_centers"),
  }));
  return (
    <>
      <Head title="我的凭证">
        <A class="btn" href="/centers">
          申请新凭证
        </A>
      </Head>
      <ErrorNotice error={q.error} />
      <For
        each={q.data}
        fallback={<Empty>尚无可用凭证。先连接身份中心并提交申请。</Empty>}
      >
        {(c) => (
          <A class="list-row" href={`/credentials/${c.id}`}>
            <div class="row-main">
              <span class="object-icon">
                <Icon name="person-vcard" />
              </span>
              <div>
                {centers.data?.find((x) => x.id === c.center_id)?.info.settings
                  .display_name ?? "身份凭证"}
                <div class="sub">
                  {Object.keys(c.claims).length} 项属性 · 保存于{" "}
                  {date(c.saved_at)}
                </div>
                <div class="sub mono">{c.issuer_id}</div>
              </div>
            </div>
            <span class="badge success">
              凭证可用 <Icon name="chevron-right" />
            </span>
          </A>
        )}
      </For>
    </>
  );
}
export function CredentialDetail() {
  const params = useParams(),
    navigate = useNavigate(),
    action = useAction(),
    [remove, setRemove] = createSignal(false);
  const q = createQuery(() => ({
    queryKey: ["credential", params.credentialId],
    queryFn: () =>
      command<Credential>("get_credential", { id: params.credentialId }),
  }));
  return (
    <>
      <ErrorNotice error={q.error || action.error()} />
      <Show when={q.data}>
        {(c) => (
          <>
            <Head title="凭证详情">
              <span class="badge success">凭证可用</span>
              <Button danger onClick={() => setRemove(true)}>
                删除本地凭证
              </Button>
              <A class="btn primary" href="/login/new">
                用于应用登录
              </A>
            </Head>
            <div class="kv">
              <div>Issuer 指纹</div>
              <div class="mono">{c().issuer_id}</div>
              <div>原请求 ID</div>
              <div class="mono">{c().id}</div>
              <div>本地保存时间</div>
              <div>{date(c().saved_at)}</div>
            </div>
            <h2 class="section-title">完整本地属性</h2>
            <ClaimsTable claims={c().claims} />
            <Notice>
              应用只会看到您在每次登录时明确选择披露的属性。用户秘密与凭证备份不会交给界面或应用。
            </Notice>
            <Show when={remove()}>
              <Modal
                title="删除本地凭证"
                onClose={() => setRemove(false)}
                footer={
                  <>
                    <Button onClick={() => setRemove(false)}>取消</Button>
                    <Button
                      danger
                      disabled={action.busy()}
                      onClick={async () => {
                        await action.run("delete_credential", { id: c().id });
                        if (!action.error()) navigate("/credentials");
                      }}
                    >
                      确认删除
                    </Button>
                  </>
                }
              >
                <p>
                  删除后本机将不能使用该凭证登录应用。中心签发记录仍存在，成员签发名额不会释放；本期没有自动补发或迁移功能。
                </p>
                <label class="hint">请确认您理解删除的影响。</label>
              </Modal>
            </Show>
          </>
        )}
      </Show>
    </>
  );
}
