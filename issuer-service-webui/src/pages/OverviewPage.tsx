import { For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { createQuery } from "@tanstack/solid-query";
import { api, type Overview } from "../api";
import { Button, ErrorNotice, Head, Icon } from "../ui";
import { EnrollmentRows } from "../features/EnrollmentRows";

export function OverviewPage() {
  const q = createQuery(() => ({
    queryKey: ["overview"],
    queryFn: () => api<Overview>("overview"),
    refetchInterval: 10000,
  }));
  return (
    <>
      <Head title="概览">
        <Button onClick={() => q.refetch()}>
          <Icon name="arrow-clockwise" />
          刷新
        </Button>
      </Head>
      <ErrorNotice error={q.error} />
      <Show when={q.data}>
        {(data) => (
          <>
            <div class="stat-strip">
              <For
                each={[
                  [String(data().pending), "等待审批", "逐项核验完整申请"],
                  [String(data().failed), "签发待恢复", "已批准的原申请"],
                  [String(data().issued), "中心已签发", "钱包需完成解盲保存"],
                  [
                    data().key_available ? "可用" : "不可用",
                    "签发密钥",
                    "已校验固定签发身份",
                  ],
                ]}
              >
                {([value, label, foot]) => (
                  <div class="stat">
                    <div class="label">{label}</div>
                    <div class="value">{value}</div>
                    <div class="foot">{foot}</div>
                  </div>
                )}
              </For>
            </div>
            <div class="two-col">
              <section>
                <h2 class="section-title">
                  近期申请{" "}
                  <A class="meta inline-link" href="/enrollments">
                    全部申请 →
                  </A>
                </h2>
                <EnrollmentRows entries={data().recent} />
              </section>
              <aside class="panel">
                <div class="panel-head">服务状态</div>
                <div class="panel-body">
                  <div class="kv">
                    <div>接受新注册</div>
                    <div>
                      {data().settings.registration_enabled
                        ? "已开放"
                        : "已关闭"}
                    </div>
                    <div>凭证签发</div>
                    <div>
                      {data().settings.issuance_enabled ? "已开启" : "已暂停"}
                    </div>
                    <div>服务修订</div>
                    <div>r{data().settings.revision}</div>
                    <div>Claims 建议</div>
                    <div>r{data().settings.claim_hints_revision}</div>
                  </div>
                  <p class="hint">已有凭证的应用登录无需联系中心。</p>
                  <A class="btn" href="/settings/service">
                    查看服务设置
                  </A>
                </div>
              </aside>
            </div>
          </>
        )}
      </Show>
    </>
  );
}
