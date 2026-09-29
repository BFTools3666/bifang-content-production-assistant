import { MantineProvider } from "@mantine/core";
import { IconHome } from "@tabler/icons-react";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { createInstance, type i18n } from "i18next";
import type { ReactElement } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AboutPageTemplate } from "./AboutPageTemplate";
import {
  APP_SIDEBAR_COMPACT_PADDING_PX,
  APP_SIDEBAR_LOGO_SIZES,
  APP_SIDEBAR_NAV_ICON_SIZE_PX,
  APP_SIDEBAR_WIDTHS,
  AppSidebarTemplate,
} from "./AppSidebarTemplate";
import { SponsorPageTemplate } from "./SponsorPageTemplate";
import enUS from "./i18n/en-US.json";
import zhCN from "./i18n/zh-CN.json";
import type { LocalizedReleaseNoteEntry } from "./releaseNotes";

/** 为 jsdom 提供 Mantine 布局所需的最小观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时无需释放真实宿主资源。 */
  disconnect(): void {}

  /** 测试不执行真实布局测量。 */
  observe(): void {}

  /** 测试允许停止观察但不保存状态。 */
  unobserve(): void {}
}

/** 创建带完整品牌 namespace 的真实测试 i18next 实例。 */
async function createTestI18n(locale: "zh-CN" | "en-US"): Promise<i18n> {
  const instance = createInstance();
  await instance.init({
    fallbackLng: "en-US",
    interpolation: { escapeValue: false },
    lng: locale,
    resources: {
      "en-US": { brandSupport: enUS },
      "zh-CN": { brandSupport: zhCN },
    },
  });
  return instance;
}

/** 在真实 Mantine 与 i18next provider 下渲染支持界面。 */
async function renderTemplate(
  node: ReactElement,
  locale: "zh-CN" | "en-US" = "zh-CN",
  colorScheme: "light" | "dark" = "light",
) {
  const instance = await createTestI18n(locale);
  return render(
    <I18nextProvider i18n={instance}>
      <MantineProvider forceColorScheme={colorScheme}>{node}</MantineProvider>
    </I18nextProvider>,
  );
}

const RELEASE_NOTE: LocalizedReleaseNoteEntry = {
  bugFixes: [{ "en-US": "Fixed startup", "zh-CN": "修复启动问题" }],
  featureOptimizations: [
    { "en-US": "Improved navigation", "zh-CN": "优化导航体验" },
  ],
  releaseDate: "2026-09-10",
  version: "v0.1.0",
};

describe("selected compact GUI baseline", () => {
  beforeEach(() => {
    Object.defineProperty(globalThis, "ResizeObserver", {
      configurable: true,
      value: TestResizeObserver,
    });
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        addEventListener: vi.fn(),
        addListener: vi.fn(),
        dispatchEvent: vi.fn(),
        matches: false,
        media: query,
        onchange: null,
        removeEventListener: vi.fn(),
        removeListener: vi.fn(),
      })),
    });
  });

  afterEach(() => cleanup());

  /** 精简侧栏锁定 80/36/22/6，并让支持菜单保持赞助、设置、关于顺序。 */
  it("renders compact-80 navigation without a collapse control", async () => {
    const onNavigate = vi.fn();
    await renderTemplate(
      <AppSidebarTemplate
        activePath="/"
        applicationName="毕方内容生产助手"
        featureItems={[{ icon: IconHome, id: "home", label: "首页", to: "/" }]}
        logoSrc="/app-identity/logo.png"
        mode="compact"
        onNavigate={onNavigate}
        supportPages={{ aboutPage: true, sponsorPage: true }}
        version="0.1.0"
      />,
    );

    const sidebar = screen.getByRole("navigation", { name: "应用导航" });
    expect(sidebar).toHaveStyle({ width: `${APP_SIDEBAR_WIDTHS.compact}px` });
    expect(screen.getByTestId("app-sidebar-content")).toHaveStyle({
      padding: `${APP_SIDEBAR_COMPACT_PADDING_PX}px`,
    });
    expect(
      screen.getByRole("img", { name: "毕方内容生产助手 应用 Logo" }),
    ).toHaveStyle({
      height: `${APP_SIDEBAR_LOGO_SIZES.compact}px`,
      width: `${APP_SIDEBAR_LOGO_SIZES.compact}px`,
    });
    expect(screen.getByTestId("navigation-icon-home")).toHaveAttribute(
      "width",
      String(APP_SIDEBAR_NAV_ICON_SIZE_PX),
    );
    expect(
      screen.queryByTestId("app-sidebar-collapse-toggle"),
    ).not.toBeInTheDocument();
    const bottomNavigation = screen.getByTestId("fixed-bottom-navigation");
    expect(
      within(bottomNavigation)
        .getAllByRole("button")
        .map((item) => item.getAttribute("aria-label")),
    ).toEqual(["赞助", "设置", "关于"]);
    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    expect(onNavigate).toHaveBeenCalledWith("/settings");
  });

  /** 关于页保持 NotConfigured 禁用检查，同时在加载失败后由重试按钮恢复。 */
  it("shows a bounded release notes load failure and retries from its own control", async () => {
    const releaseNotesLoader = vi
      .fn<() => Promise<readonly LocalizedReleaseNoteEntry[]>>()
      .mockRejectedValueOnce(new Error("resource unavailable"))
      .mockResolvedValueOnce([RELEASE_NOTE]);
    const onCheckForUpdates = vi.fn();
    await renderTemplate(
      <AboutPageTemplate
        onCheckForUpdates={onCheckForUpdates}
        productName="毕方内容生产助手"
        releaseNotesLoader={releaseNotesLoader}
        update={{ currentVersion: "0.1.0", status: "not-configured" }}
        version="0.1.0"
      />,
    );

    const checkButton = screen.getByRole("button", { name: "检查更新" });
    expect(
      screen.getByRole("heading", { name: "毕方内容生产助手" }),
    ).toBeInTheDocument();
    expect(checkButton).toBeDisabled();
    fireEvent.click(screen.getByTestId("about-update-section"));
    expect(onCheckForUpdates).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "更新日志" }));
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() =>
      expect(screen.getByText("优化导航体验")).toBeInTheDocument(),
    );
    expect(releaseNotesLoader).toHaveBeenCalledTimes(2);
    expect(screen.getByText(/守城工作室/)).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(5);
  });

  /** 更新日志使用当前 i18next locale 选择英文，而不是混入中文资源。 */
  it("selects English release-note translations from the active locale", async () => {
    const releaseNotesLoader = vi.fn().mockResolvedValue([RELEASE_NOTE]);
    await renderTemplate(
      <AboutPageTemplate
        onCheckForUpdates={vi.fn()}
        productName="毕方内容生产助手"
        releaseNotesLoader={releaseNotesLoader}
        update={{ currentVersion: "0.1.0", status: "not-configured" }}
        version="0.1.0"
      />,
      "en-US",
    );

    fireEvent.click(screen.getByRole("button", { name: "Release notes" }));
    await waitFor(() =>
      expect(screen.getByText("Improved navigation")).toBeInTheDocument(),
    );
    expect(screen.getByText("Fixed startup")).toBeInTheDocument();
    expect(screen.queryByText("优化导航体验")).not.toBeInTheDocument();
    expect(releaseNotesLoader).toHaveBeenCalledOnce();
  });

  /** 赞助页渲染完整三档事实、三张档位图与两张具名支付码。 */
  it("renders the complete local sponsor profile in dark mode", async () => {
    await renderTemplate(<SponsorPageTemplate />, "zh-CN", "dark");

    expect(screen.getByText("19")).toBeInTheDocument();
    expect(screen.getByText("199")).toBeInTheDocument();
    expect(screen.getByText("1999")).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(3);
    expect(
      screen.getByRole("img", { name: "微信支付赞助二维码" }),
    ).toHaveAttribute("src", "/brand-support/sponsor/pay1.png");
    expect(
      screen.getByRole("img", { name: "支付宝赞助二维码" }),
    ).toHaveAttribute("src", "/brand-support/sponsor/pay2.png");
    expect(screen.getByTestId("brand-sponsor-page")).toHaveAttribute(
      "data-color-scheme",
      "dark",
    );
  });
});
