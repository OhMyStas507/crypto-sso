import { Show } from "solid-js";
import { createQuery } from "@tanstack/solid-query";
import type { ServiceInfo } from "@crypto-sso/identity-api-types";
import { Button, ErrorNotice, Head, Notice, SettingsTabs } from "../ui";

export function IssuerPage() {
  const q = createQuery(() => ({
    queryKey: ["issuer-public"],
    queryFn: async () => {
      const r = await fetch("/api/v1/service-info");
      if (!r.ok) throw new Error("无法读取签发身份");
      return r.json() as Promise<ServiceInfo>;
    },
  }));
  return (
    <>
      <Head title="签发身份" />
      <SettingsTabs />
      <ErrorNotice error={q.error} />
      <Show when={q.data}>
        {(data) => (
          <div class="settings-frame">
            <Notice>
              签发身份由部署时导入的固定密钥确定。修改服务展示名或 Claims
              建议不会改变指纹。
            </Notice>
            <div class="kv">
              <div>组织标识</div>
              <div>
                {
                  (data().issuer_public as { organization: string })
                    .organization
                }
              </div>
              <div>Issuer 指纹</div>
              <div class="mono">{data().issuer_id}</div>
              <div>密钥状态</div>
              <div class="badge success">已导入并校验</div>
              <div>协议版本</div>
              <div>v1</div>
            </div>
            <div class="form-footer">
              <Button
                onClick={() => navigator.clipboard.writeText(data().issuer_id)}
              >
                复制完整指纹
              </Button>
            </div>
            <h2 class="section-title">公开信任信息</h2>
            <pre class="codebox">
              {JSON.stringify(data().issuer_public, null, 2)}
            </pre>
            <h2 class="section-title">进程生效资源限制</h2>
            <div class="kv">
              <div>属性数量</div>
              <div>{data().effective_claims_limits.max_keys}</div>
              <div>键 / 值字节数</div>
              <div>
                {data().effective_claims_limits.max_key_bytes} /{" "}
                {data().effective_claims_limits.max_value_bytes}
              </div>
            </div>
          </div>
        )}
      </Show>
    </>
  );
}
