import type {
  Claims,
  EnrollmentView,
  ReviewDecision,
  ServiceSettings,
} from "@crypto-sso/identity-api-types";
export type { ServiceSettings } from "@crypto-sso/identity-api-types";
export interface Enrollment {
  view: EnrollmentView;
  claims: Claims;
  application_note: string | null;
  claim_hints_revision: number | null;
  hints_snapshot: ServiceSettings["claim_hints"] | null;
  decision: ReviewDecision | null;
  reviewer: string | null;
  reviewed_at: number | null;
  issued_at: number | null;
  failure_code: string | null;
  history: {
    actor: string;
    at: number;
    action: string;
    decision: ReviewDecision;
  }[];
}
export interface Member {
  member_ref: string;
  request_id: string;
  evidence_source: string;
  verification_note: string;
  verified_at: number;
  issued: boolean;
}
export interface Session {
  username: string;
  csrf_token: string;
  expires_at: number;
}
export interface Overview {
  settings: ServiceSettings;
  issuer_id: string;
  key_available: boolean;
  pending: number;
  failed: number;
  issued: number;
  recent: Enrollment[];
}
let csrf = "";
export class ApiFailure extends Error {
  code: string;
  traceId: string;
  constructor(code: string, message: string, traceId = "") {
    super(message);
    this.code = code;
    this.traceId = traceId;
  }
}
export function operationKey(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(32)), (b) =>
    b.toString(16).padStart(2, "0"),
  ).join("");
}
// /admin and /api are same-origin. Deployment credentials never enter browser configuration.
export async function api<T>(
  path: string,
  method = "GET",
  body?: unknown,
  key?: string,
): Promise<T> {
  const response = await fetch(`/api/v1/admin/${path}`, {
    method,
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
      ...(key ? { "Idempotency-Key": key } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const value = await response.json().catch(() => ({
    code: "REQUEST_REJECTED",
    message: "请求被拒绝，请刷新页面后重试",
  }));
  if (!response.ok) {
    if (
      response.status === 401 &&
      path !== "sessions" &&
      location.pathname !== "/admin/login"
    )
      location.assign("/admin/login");
    throw new ApiFailure(
      value.code,
      value.message ?? "请求失败",
      value.trace_id,
    );
  }
  if (
    (path === "session" || path === "sessions" || path === "csrf") &&
    method !== "DELETE"
  )
    csrf = value.csrf_token;
  return value as T;
}
