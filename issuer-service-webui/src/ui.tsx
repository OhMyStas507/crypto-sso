import { For, Show, type JSX } from "solid-js";
import { A } from "@solidjs/router";
import type { Claims, EnrollmentStatus } from "@crypto-sso/identity-api-types";
export const states: Record<EnrollmentStatus, string> = {
  PENDING_REVIEW: "等待审批",
  CHANGES_REQUIRED: "需要修改",
  REJECTED: "已拒绝",
  CANCELLED: "已取消",
  EXPIRED: "待审已过期",
  APPROVED: "审批通过 · 等待签发",
  SIGNING: "正在签发",
  ISSUED: "中心已签发",
  ISSUE_FAILED: "签发暂未完成",
  INVALID_REQUEST: "请求验证失败",
};
export function Icon(p: { name: string }) {
  return <i aria-hidden="true" class={`bi bi-${p.name}`} />;
}
export function Button(p: {
  children: JSX.Element;
  onClick?: () => void;
  primary?: boolean;
  danger?: boolean;
  disabled?: boolean;
  type?: "button" | "submit";
}) {
  return (
    <button
      type={p.type ?? "button"}
      class={`btn ${p.primary ? "primary" : ""} ${p.danger ? "danger" : ""}`}
      disabled={p.disabled}
      onClick={p.onClick}
    >
      {p.children}
    </button>
  );
}
export function Head(p: { title: string; children?: JSX.Element }) {
  return (
    <div class="page-head">
      <h1>{p.title}</h1>
      <div class="actions">{p.children}</div>
    </div>
  );
}
export function Field(p: {
  label: string;
  hint?: string;
  children: JSX.Element;
}) {
  return (
    <label class="field">
      <span>{p.label}</span>
      {p.children}
      <Show when={p.hint}>
        <small class="hint">{p.hint}</small>
      </Show>
    </label>
  );
}
export function Notice(p: { children: JSX.Element; danger?: boolean }) {
  return (
    <div
      role={p.danger ? "alert" : "status"}
      class={`notice ${p.danger ? "danger" : ""}`}
    >
      <Icon name="info-circle" />
      <div>{p.children}</div>
    </div>
  );
}
export function Status(p: { value: EnrollmentStatus }) {
  return (
    <span
      class={`badge ${p.value === "ISSUED" ? "success" : ["ISSUE_FAILED", "REJECTED", "INVALID_REQUEST"].includes(p.value) ? "danger" : "purple"}`}
    >
      <Icon name={p.value === "ISSUED" ? "check-circle" : "clock"} />
      {states[p.value]}
    </span>
  );
}
export function date(value: number | null | undefined) {
  return value
    ? new Date(value * 1000).toLocaleString("zh-CN", { hour12: false })
    : "—";
}
export function short(value: string) {
  return value.length > 22 ? `${value.slice(0, 12)}…${value.slice(-6)}` : value;
}
export function ClaimsTable(p: { claims: Claims }) {
  return (
    <table class="table">
      <thead>
        <tr>
          <th>属性键</th>
          <th>完整字符串值</th>
        </tr>
      </thead>
      <tbody>
        <For
          each={Object.entries(p.claims)}
          fallback={
            <tr>
              <td colspan="2" class="muted">
                空属性集合 · 仅证明成员资格
              </td>
            </tr>
          }
        >
          {([key, value]) => (
            <tr>
              <td class="mono">{key}</td>
              <td class="preserve">
                {value === "" ? <span class="muted">空字符串</span> : value}
              </td>
            </tr>
          )}
        </For>
      </tbody>
    </table>
  );
}
export function Empty(p: { children: JSX.Element }) {
  return (
    <div class="empty-state">
      <Icon name="inbox" />
      <p>{p.children}</p>
    </div>
  );
}
export function ErrorNotice(p: { error: unknown }) {
  return (
    <Show when={p.error}>
      <Notice danger>
        {p.error instanceof Error ? p.error.message : String(p.error)}
      </Notice>
    </Show>
  );
}
export function Modal(p: {
  title: string;
  children: JSX.Element;
  onClose: () => void;
  footer?: JSX.Element;
}) {
  return (
    <div class="modal-overlay" onClick={p.onClose}>
      <section
        class="modal"
        role="dialog"
        aria-modal="true"
        aria-label={p.title}
        onClick={(e) => e.stopPropagation()}
      >
        <div class="modal-title">
          {p.title}
          <Button onClick={p.onClose}>×</Button>
        </div>
        <div class="modal-body">{p.children}</div>
        <div class="modal-footer">{p.footer}</div>
      </section>
    </div>
  );
}
export function SettingsTabs() {
  return (
    <div class="tabs">
      <A class="tab" activeClass="active" href="/settings/service">
        服务设置
      </A>
      <A class="tab" activeClass="active" href="/settings/claims">
        Claims 默认建议
      </A>
      <A class="tab" activeClass="active" href="/settings/issuer">
        签发身份
      </A>
    </div>
  );
}
