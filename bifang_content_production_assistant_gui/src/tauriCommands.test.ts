import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  checkForUpdates,
  decodeAdapterUpdaterStatus,
  decodeAppMetadata,
  decodeInterfaceLanguage,
  getAppMetadata,
  getAutostartEnabled,
  getSystemLocale,
  getSystemNotificationSetting,
  setAutostartEnabled,
  setInterfaceLanguage,
  setSystemNotificationEnabled,
  synchronizeDocumentTitle,
} from "./tauriCommands";

const METADATA = {
  applicationName: "毕方内容生产助手",
  contactChannel: "QQ",
  contactValue: "2222980",
  title: "毕方内容生产助手 v0.1.0 QQ:2222980",
  version: "0.1.0",
};

describe("narrow Tauri command boundary", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  /** AppMetadata 只接受完整精确字段，并拒绝多余或错误类型。 */
  it("strictly decodes authoritative application metadata", () => {
    expect(decodeAppMetadata(METADATA)).toEqual(METADATA);
    expect(() => decodeAppMetadata({ ...METADATA, extra: true })).toThrow(
      "invalid app metadata",
    );
    expect(() => decodeAppMetadata({ ...METADATA, version: 1 })).toThrow(
      "invalid non-empty string field: version",
    );
  });

  /** 元数据命令的 title 与 Rust 原生标题共享事实并同步 document。 */
  it("reads metadata and synchronizes its authoritative title", async () => {
    invokeMock.mockResolvedValueOnce(METADATA);

    await expect(getAppMetadata()).resolves.toEqual(METADATA);
    expect(invokeMock).toHaveBeenCalledWith("get_app_metadata");
    synchronizeDocumentTitle(METADATA.title);
    expect(document.title).toBe(METADATA.title);
  });

  /** 系统语言只归一化为中文或英文，并通过窄命令保存。 */
  it("normalizes and persists the shared interface language", async () => {
    expect(decodeInterfaceLanguage("zh-Hant-TW")).toBe("zh-CN");
    expect(decodeInterfaceLanguage("fr-FR")).toBe("en-US");
    expect(() => decodeInterfaceLanguage(false)).toThrow(
      "invalid interface language",
    );
    invokeMock.mockResolvedValueOnce("zh-CN").mockResolvedValueOnce("en-US");

    await expect(getSystemLocale()).resolves.toBe("zh-CN");
    await expect(setInterfaceLanguage("en-US")).resolves.toBe("en-US");
    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_system_locale");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "set_interface_language", {
      language: "en-US",
    });
  });

  /** 通知与自启分别调用自己的 getter/setter，且只接受布尔结果。 */
  it("keeps notification and autostart command state separate", async () => {
    invokeMock
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce(false);

    await expect(getSystemNotificationSetting()).resolves.toBe(false);
    await expect(setSystemNotificationEnabled(true)).resolves.toBe(true);
    await expect(getAutostartEnabled()).resolves.toBe(true);
    await expect(setAutostartEnabled(false)).resolves.toBe(false);
    expect(invokeMock.mock.calls).toEqual([
      ["get_system_notification_setting"],
      ["set_system_notification_enabled", { enabled: true }],
      ["get_autostart_enabled"],
      ["set_autostart_enabled", { enabled: false }],
    ]);

    invokeMock.mockResolvedValueOnce("false");
    await expect(getAutostartEnabled()).rejects.toThrow(
      "invalid boolean response",
    );
  });

  /** 更新入口只接受稳定状态模型并保留 NotConfigured 零出站语义。 */
  it("decodes the not-configured update presentation", async () => {
    expect(decodeAdapterUpdaterStatus("not-configured")).toBe("not-configured");
    expect(() => decodeAdapterUpdaterStatus("idle")).toThrow(
      "invalid updater status",
    );
    invokeMock.mockResolvedValueOnce("not-configured");
    await expect(checkForUpdates("0.1.0")).resolves.toEqual({
      currentVersion: "0.1.0",
      status: "not-configured",
    });
    expect(invokeMock).toHaveBeenCalledWith("check_for_updates");
  });
});
