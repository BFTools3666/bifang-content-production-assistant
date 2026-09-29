import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createInstance } from "i18next";
import { createElement } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";

const commandMocks = vi.hoisted(() => ({
  inspectLarkCliSetup: vi.fn(),
}));

vi.mock("./workbenchCommands", () => commandMocks);

import {
  extractFeishuFolderToken,
  isSafeLarkCliProfile,
  LarkCliSetupGuide,
} from "./LarkCliSetupGuide";
import type { LarkCliSetupInspection } from "./workbenchTypes";

/** 测试弹窗不依赖真实宿主尺寸观察。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有宿主观察资源。 */
  disconnect(): void {}
  /** 测试不执行真实布局测量。 */
  observe(): void {}
  /** 测试允许停止观察但不维护目标集合。 */
  unobserve(): void {}
}

/** 构造不含凭据的 CLI 只读检测结果。 */
function inspection(
  configured: boolean,
  authorized: boolean,
  issueCode: string | null = null,
): LarkCliSetupInspection {
  return {
    authorizationIssueCode: issueCode,
    profiles: configured
      ? [
          {
            name: "seo-publisher",
            tokenStatus: authorized ? "valid" : "missing",
            userName: authorized ? "测试用户" : null,
          },
        ]
      : [],
    selectedProfileAuthorized: authorized,
    selectedProfileConfigured: configured,
    selectedUserName: authorized ? "测试用户" : null,
    version: "1.0.96",
  };
}

/** 用真实 Mantine 与双语资源渲染只读引导。 */
async function renderGuide(onUseConfiguration = vi.fn()) {
  const i18n = createInstance();
  await i18n.init({
    fallbackLng: "en-US",
    lng: "zh-CN",
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  const result = render(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(
        MantineProvider,
        { env: "test" },
        createElement(LarkCliSetupGuide, {
          onUseConfiguration,
          openInitially: true,
        }),
      ),
    ),
  );
  return { ...result, onUseConfiguration };
}

describe("lark CLI read-only guide inputs", () => {
  /** 只接受官方飞书云盘文件夹 URL 或合法 token。 */
  it("extracts_only_official_drive_folder_tokens", () => {
    expect(
      extractFeishuFolderToken(
        "https://tenant.feishu.cn/drive/folder/fldcnABC123?from=copy",
      ),
    ).toBe("fldcnABC123");
    expect(extractFeishuFolderToken("fldcnABC123")).toBe("fldcnABC123");
    expect(extractFeishuFolderToken("test")).toBeNull();
    expect(
      extractFeishuFolderToken("https://tenant.feishu.cn/wiki/wikcnABC123"),
    ).toBeNull();
    expect(
      extractFeishuFolderToken(
        "https://attacker.example/drive/folder/fldcnABC123",
      ),
    ).toBeNull();
  });

  /** profile 输入拒绝空白和特殊字符。 */
  it("accepts_only_safe_profile_names", () => {
    expect(isSafeLarkCliProfile("seo-publisher")).toBe(true);
    expect(isSafeLarkCliProfile("内容发布")).toBe(true);
    expect(isSafeLarkCliProfile("bad profile")).toBe(false);
    expect(isSafeLarkCliProfile("bad;command")).toBe(false);
    expect(isSafeLarkCliProfile("")).toBe(false);
  });
});

describe("lark CLI read-only guide lifecycle", () => {
  beforeEach(() => {
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
    Object.defineProperty(globalThis, "ResizeObserver", {
      configurable: true,
      value: TestResizeObserver,
    });
    commandMocks.inspectLarkCliSetup.mockReset();
    commandMocks.inspectLarkCliSetup.mockResolvedValue(
      inspection(false, false, "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED"),
    );
  });

  afterEach(() => cleanup());

  /** 页面不主动运行 CLI，也不提供应用内建 profile 或扫码登录入口。 */
  it("only_checks_after_explicit_click_and_never_offers_login_actions", async () => {
    await renderGuide();
    expect(commandMocks.inspectLarkCliSetup).not.toHaveBeenCalled();
    expect(screen.getByText(/在应用外按官方 lark-cli 文档安装/)).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /扫码|创建 Profile/ }),
    ).toBeNull();
    expect(screen.queryByText(/@latest/)).toBeNull();

    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "检测 CLI" }));
    await waitFor(() =>
      expect(commandMocks.inspectLarkCliSetup).toHaveBeenCalledExactlyOnceWith(
        "seo-publisher",
      ),
    );
    expect(screen.getByRole("button", { name: "下一步" })).toBeDisabled();
  });

  /** profile 变化立即使旧授权结果失效，读取中不能改名。 */
  it("invalidates_old_user_identity_when_profile_changes", async () => {
    commandMocks.inspectLarkCliSetup.mockResolvedValueOnce(
      inspection(true, true),
    );
    const user = userEvent.setup();
    await renderGuide();
    await user.click(screen.getByRole("button", { name: "检测 CLI" }));
    expect(await screen.findByText("用户身份已验证：测试用户")).toBeVisible();

    const profileInput = screen.getByLabelText("Profile 名称");
    await user.clear(profileInput);
    await user.type(profileInput, "another-profile");
    expect(screen.queryByText("用户身份已验证：测试用户")).toBeNull();
    expect(screen.getByRole("button", { name: "下一步" })).toBeDisabled();
    expect(commandMocks.inspectLarkCliSetup).toHaveBeenCalledTimes(1);

    let resolveInspection:
      ((value: LarkCliSetupInspection) => void) | undefined;
    commandMocks.inspectLarkCliSetup.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveInspection = resolve;
        }),
    );
    await user.click(
      screen.getByRole("button", { name: "我已完成，重新检测" }),
    );
    expect(profileInput).toBeDisabled();
    resolveInspection?.(inspection(false, false));
    await waitFor(() => expect(profileInput).toBeEnabled());
    expect(screen.getByRole("button", { name: "下一步" })).toBeDisabled();
  });

  /** 复检失败不得沿用先前授权或允许进入文件夹回填。 */
  it("invalidates_previous_authorization_when_recheck_fails", async () => {
    commandMocks.inspectLarkCliSetup.mockResolvedValueOnce(
      inspection(true, true),
    );
    const user = userEvent.setup();
    await renderGuide();
    await user.click(screen.getByRole("button", { name: "检测 CLI" }));
    expect(await screen.findByText("用户身份已验证：测试用户")).toBeVisible();

    commandMocks.inspectLarkCliSetup.mockRejectedValueOnce(
      new Error("CLI unavailable"),
    );
    await user.click(
      screen.getByRole("button", { name: "我已完成，重新检测" }),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "下一步" })).toBeDisabled(),
    );
    expect(screen.queryByText("用户身份已验证：测试用户")).toBeNull();
    expect(commandMocks.inspectLarkCliSetup).toHaveBeenCalledTimes(2);
  });

  /** 只读检测通过后回填 profile 和文件夹，保存与连接测试仍由主表单负责。 */
  it("fills_target_only_after_verified_user_and_folder", async () => {
    commandMocks.inspectLarkCliSetup.mockResolvedValueOnce(
      inspection(true, true),
    );
    const user = userEvent.setup();
    const { onUseConfiguration } = await renderGuide();
    await user.click(screen.getByRole("button", { name: "检测 CLI" }));
    expect(await screen.findByText("用户身份已验证：测试用户")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "下一步" }));
    const folderInput = screen.getByLabelText("飞书文件夹链接或 Token");
    await user.type(
      folderInput,
      "https://example.feishu.cn/drive/folder/fldcnABC123",
    );
    await user.click(screen.getByRole("button", { name: "填写发布目标" }));
    expect(onUseConfiguration).toHaveBeenCalledExactlyOnceWith(
      "seo-publisher",
      "fldcnABC123",
    );
    expect(commandMocks.inspectLarkCliSetup).toHaveBeenCalledTimes(1);
  });

  /** 卸载时迟到的只读检测不得继续更新页面。 */
  it("ignores_late_read_only_inspection_after_unmount", async () => {
    let resolveInspection:
      ((value: LarkCliSetupInspection) => void) | undefined;
    commandMocks.inspectLarkCliSetup.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveInspection = resolve;
        }),
    );
    const user = userEvent.setup();
    const { unmount } = await renderGuide();
    await user.click(screen.getByRole("button", { name: "检测 CLI" }));
    unmount();
    resolveInspection?.(inspection(true, true));
    await Promise.resolve();
    expect(commandMocks.inspectLarkCliSetup).toHaveBeenCalledTimes(1);
  });
});
