import { afterEach, describe, expect, it, vi } from "vitest";

const { getSystemLocale } = vi.hoisted(() => ({ getSystemLocale: vi.fn() }));
vi.mock("./tauriCommands", () => ({ getSystemLocale }));
import i18n, { initializeI18n } from "./i18n";

afterEach(() => { getSystemLocale.mockReset(); });

describe("中文界面启动", () => {
  it("宿主读取失败时首次启动和新增导航仍显示中文", async () => {
    getSystemLocale.mockRejectedValue(new Error("host unavailable"));
    expect(await initializeI18n()).toBe("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(i18n.t("navigation.workbench")).toBe("工作台");
    expect(i18n.t("research.navigation.benchmarks")).toBe("对标采集");
    expect(i18n.t("research.navigation.styles")).toBe("风格库");
  });

  it("尊重宿主已保存的语言，初始化重入不会覆盖选择", async () => {
    getSystemLocale.mockResolvedValue("en-US");
    expect(await initializeI18n()).toBe("en-US");
    expect(document.documentElement.lang).toBe("en-US");
    getSystemLocale.mockResolvedValue("zh-CN");
    expect(await initializeI18n()).toBe("zh-CN");
    expect(i18n.t("navigation.workbench")).toBe("工作台");
  });
});
