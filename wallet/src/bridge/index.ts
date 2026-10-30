import { invoke, isTauri } from "@tauri-apps/api/core";
export type {
  WalletStatus,
  Center,
  Draft,
  TrustedApp,
  LoginTask,
  DisclosurePlan,
  CredentialView as Credential,
  LocalEnrollmentView as LocalEnrollment,
} from "./generated";

export type Command =
  | "touch_wallet"
  | "get_wallet_status"
  | "initialize_wallet"
  | "unlock_wallet"
  | "lock_wallet"
  | "probe_center"
  | "confirm_center"
  | "list_centers"
  | "get_center"
  | "refresh_claim_hints"
  | "create_enrollment_draft"
  | "get_enrollment_draft"
  | "save_enrollment_draft"
  | "submit_enrollment"
  | "list_enrollments"
  | "get_enrollment"
  | "refresh_enrollment"
  | "retry_credential_save"
  | "cancel_enrollment"
  | "list_credentials"
  | "get_credential"
  | "delete_credential"
  | "list_trusted_apps"
  | "save_trusted_app"
  | "remove_trusted_app"
  | "list_login_tasks"
  | "import_login_challenge"
  | "get_disclosure_plan"
  | "create_presentation"
  | "refuse_disclosure"
  | "get_preferences"
  | "save_preferences";

const messages: Record<string, string> = {
  CREDENTIAL_DELETED: "本地凭证已删除，中心名额仍保留。本期不支持自动补发。",
  WALLET_LOCKED: "钱包已锁定，请重新解锁。",
  NETWORK_UNAVAILABLE: "无法连接中心，已保留本地恢复材料与上次状态。",
  ISSUER_MISMATCH: "中心指纹与固定身份不符，已停止发送数据。",
  INVALID_CLAIMS: "属性键重复、格式错误或 UTF-8 字节数超过限制。",
  CREDENTIAL_SAVE_FAILED: "凭证保存失败，已保留原申领恢复材料，请重试保存。",
  CHALLENGE_EXPIRED: "挑战已过期，请回到原应用获取新挑战。",
  INVALID_DISCLOSURE_OR_EXPIRED: "披露项不完整或挑战已过期，请重新核对。",
  CHALLENGE_CONTEXT_MISMATCH: "挑战的应用 Origin 或中心身份与受信配置不符。",
  VAULT_CORRUPT: "加密钱包校验失败，停止写入。请检查受保护的数据目录。",
  CENTER_ALREADY_EXISTS: "该签发身份已连接，请使用已有中心。",
  PENDING_MISSING: "本地 pending 已丢失，不能仅凭请求 ID 恢复凭证。",
};
// All IPC strings live here. Pages receive views only; secrets remain in Rust.
export async function command<T>(
  name: Command,
  args: unknown = {},
): Promise<T> {
  if (!isTauri())
    throw new Error(
      "请通过 Tauri 桌面应用打开钱包。浏览器预览无法访问受系统保护的本地数据。",
    );
  try {
    return await invoke<T>("wallet_command", { command: name, args });
  } catch (error) {
    const code = String(error);
    throw new Error(
      messages[code] ??
        (code.startsWith("RATE_LIMITED:")
          ? "中心请求限流，请稍后重试。"
          : code),
    );
  }
}
