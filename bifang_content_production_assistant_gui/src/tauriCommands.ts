import { invoke } from "@tauri-apps/api/core";

import type { SupportedInterfaceLanguage } from "./brand-support/SettingsPageTemplate";
import type { UpdatePresentation } from "./brand-support/updatePresentation";

/** Tauri 返回的权威应用身份。 */
export interface AppMetadata {
  applicationName: string;
  version: string;
  contactChannel: string;
  contactValue: string;
  title: string;
}

/** Rust 公共服务适配器直接序列化的稳定状态集合。 */
type AdapterUpdaterStatus =
  | "not-configured"
  | "checking"
  | "up-to-date"
  | "optional-update"
  | "required-update"
  | "updates-paused"
  | "failed";

const ADAPTER_UPDATER_STATUSES: ReadonlySet<AdapterUpdaterStatus> = new Set([
  "not-configured",
  "checking",
  "up-to-date",
  "optional-update",
  "required-update",
  "updates-paused",
  "failed",
]);

/** 判断未知 IPC 值是否是普通记录对象。 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 要求 IPC 记录只包含固定字段，避免默默接受协议漂移。 */
function hasExactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
): boolean {
  const actual = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  return (
    actual.length === sortedExpected.length &&
    actual.every((key, index) => key === sortedExpected[index])
  );
}

/** 从 IPC 记录读取经过修剪且非空的字符串字段。 */
function readString(value: Record<string, unknown>, key: string): string {
  const field = value[key];
  if (
    typeof field !== "string" ||
    field.trim() !== field ||
    field.length === 0
  ) {
    throw new Error(`invalid non-empty string field: ${key}`);
  }
  return field;
}

/** 从未知 IPC 值严格收窄权威应用元数据。 */
export function decodeAppMetadata(value: unknown): AppMetadata {
  const keys = [
    "applicationName",
    "contactChannel",
    "contactValue",
    "title",
    "version",
  ] as const;
  if (!isRecord(value) || !hasExactKeys(value, keys)) {
    throw new Error("invalid app metadata");
  }
  return {
    applicationName: readString(value, "applicationName"),
    contactChannel: readString(value, "contactChannel"),
    contactValue: readString(value, "contactValue"),
    title: readString(value, "title"),
    version: readString(value, "version"),
  };
}

/** 从未知 IPC 值严格收窄布尔宿主状态。 */
function decodeBoolean(value: unknown, command: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`invalid boolean response for ${command}`);
  }
  return value;
}

/** 从未知 IPC 值解码系统或宿主 locale，并归一化到受支持语言。 */
export function decodeInterfaceLanguage(
  value: unknown,
): SupportedInterfaceLanguage {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error("invalid interface language");
  }
  return value.toLowerCase().startsWith("zh") ? "zh-CN" : "en-US";
}

/** 从未知 IPC 值严格收窄 Rust updater 的稳定状态。 */
export function decodeAdapterUpdaterStatus(
  value: unknown,
): AdapterUpdaterStatus {
  if (
    typeof value !== "string" ||
    !ADAPTER_UPDATER_STATUSES.has(value as AdapterUpdaterStatus)
  ) {
    throw new Error("invalid updater status");
  }
  return value as AdapterUpdaterStatus;
}

/** 严格解码公共版本服务的最小展示对象。 */
export function decodeUpdatePresentation(value: unknown): UpdatePresentation {
  if (!isRecord(value)) throw new Error("invalid update presentation");
  const allowed = new Set([
    "currentVersion",
    "downloadReady",
    "message",
    "status",
    "title",
  ]);
  if (Object.keys(value).some((key) => !allowed.has(key))) {
    throw new Error("invalid update presentation fields");
  }
  const status = decodeAdapterUpdaterStatus(value.status);
  const downloadReady = value.downloadReady;
  if (typeof downloadReady !== "boolean") {
    throw new Error("invalid update download state");
  }
  const optionalText = (key: "title" | "message"): string | undefined => {
    if (!(key in value)) return undefined;
    return readString(value, key);
  };
  return {
    currentVersion: readString(value, "currentVersion"),
    downloadReady,
    message: optionalText("message"),
    status,
    title: optionalText("title"),
  };
}

/** 调用固定命令读取权威应用元数据。 */
export async function getAppMetadata(): Promise<AppMetadata> {
  return decodeAppMetadata(await invoke<unknown>("get_app_metadata"));
}

/** 使用 Rust 返回的同一权威标题同步 WebView 文档。 */
export function synchronizeDocumentTitle(title: string): void {
  document.title = title;
}

/** 读取保存偏好优先、系统探测兜底的界面语言。 */
export async function getSystemLocale(): Promise<SupportedInterfaceLanguage> {
  return decodeInterfaceLanguage(await invoke<unknown>("get_system_locale"));
}

/** 通过 Rust 适配器保存语言并触发原生文案同步。 */
export async function setInterfaceLanguage(
  language: SupportedInterfaceLanguage,
): Promise<SupportedInterfaceLanguage> {
  return decodeInterfaceLanguage(
    await invoke<unknown>("set_interface_language", { language }),
  );
}

/** 读取系统通知应用偏好的权威值。 */
export async function getSystemNotificationSetting(): Promise<boolean> {
  const command = "get_system_notification_setting";
  return decodeBoolean(await invoke<unknown>(command), command);
}

/** 请求改变系统通知应用偏好并返回最终权威值。 */
export async function setSystemNotificationEnabled(
  enabled: boolean,
): Promise<boolean> {
  const command = "set_system_notification_enabled";
  return decodeBoolean(await invoke<unknown>(command, { enabled }), command);
}

/** 读取操作系统登录项中的开机自启权威值。 */
export async function getAutostartEnabled(): Promise<boolean> {
  const command = "get_autostart_enabled";
  return decodeBoolean(await invoke<unknown>(command), command);
}

/** 请求改变操作系统登录项并返回最终权威值。 */
export async function setAutostartEnabled(enabled: boolean): Promise<boolean> {
  const command = "set_autostart_enabled";
  return decodeBoolean(await invoke<unknown>(command, { enabled }), command);
}

/** 调用受限更新命令，并以权威打包版本组装纯展示状态。 */
export async function startPublicServices(): Promise<UpdatePresentation> {
  return decodeUpdatePresentation(
    await invoke<unknown>("start_public_services"),
  );
}

/** 手动执行一次 application-version API v1 检查。 */
export async function checkForUpdates(): Promise<UpdatePresentation> {
  return decodeUpdatePresentation(
    await invoke<unknown>("check_public_service_updates"),
  );
}

/** 读取本地统计同意状态。 */
export async function getTelemetryEnabled(): Promise<boolean> {
  const command = "get_telemetry_enabled";
  return decodeBoolean(await invoke<unknown>(command), command);
}

/** 修改本地统计同意状态并返回权威值。 */
export async function setTelemetryEnabled(enabled: boolean): Promise<boolean> {
  const command = "set_telemetry_enabled";
  return decodeBoolean(await invoke<unknown>(command, { enabled }), command);
}

/** 打开 Rust 保存的安全下载地址，WebView 不传入 URL。 */
export async function openUpdateDownload(): Promise<void> {
  await invoke("open_update_download");
}

/** 请求完整退出应用。 */
export async function exitApplication(): Promise<void> {
  await invoke("exit_application");
}
