import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createInstance } from "i18next";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";

const commandMocks = vi.hoisted(() => ({
  deleteConnectorConfig: vi.fn(),
  inspectLarkCliSetup: vi.fn(),
  listConnectorConfigs: vi.fn(),
  newFeishuConnectorConfigDraft: vi.fn(),
  saveConnectorConfig: vi.fn(),
  setConnectorEnabled: vi.fn(),
  verifyConnectorConnection: vi.fn(),
}));

vi.mock("./workbenchCommands", () => commandMocks);

import { ConnectorSettingsPage } from "./ConnectorSettingsPage";

const SAVED_CONNECTOR = {
  cliProfile: "team-content",
  defaultFormat: "markdown" as const,
  enabled: true,
  folderToken: "fld_test",
  id: "connector-feishu",
  isDefault: true,
  kind: "feishuDrive" as const,
  name: "飞书内容库",
  revision: 4,
  timeoutMs: 30_000,
};

/** Mantine 弹窗滚动区在 jsdom 中不需要真实尺寸观察。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有宿主观察资源。 */
  disconnect(): void {}
  /** 测试不执行真实布局测量。 */
  observe(): void {}
  /** 测试允许停止观察但不维护目标集合。 */
  unobserve(): void {}
}

/** 以真实 QueryClient、Mantine 和双语资源渲染连接器页面。 */
async function renderPage(): Promise<void> {
  const i18n = createInstance();
  await i18n.init({
    fallbackLng: "en-US",
    lng: "zh-CN",
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <MantineProvider env="test">
          <ConnectorSettingsPage />
        </MantineProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

describe("connector settings page", () => {
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
    Object.values(commandMocks).forEach((mock) => mock.mockReset());
    commandMocks.listConnectorConfigs.mockResolvedValue([SAVED_CONNECTOR]);
    commandMocks.newFeishuConnectorConfigDraft.mockResolvedValue({
      cliProfile: "",
      defaultFormat: "markdown",
      enabled: true,
      folderToken: "",
      id: "",
      isDefault: true,
      kind: "feishuDrive",
      name: "飞书 CLI 发布目标",
      revision: 1,
      timeoutMs: 30_000,
    });
    commandMocks.saveConnectorConfig.mockResolvedValue({
      connectors: [SAVED_CONNECTOR],
      saved: SAVED_CONNECTOR,
    });
    commandMocks.verifyConnectorConnection.mockResolvedValue({
      message: "ok",
      ok: true,
    });
    commandMocks.inspectLarkCliSetup.mockResolvedValue({
      authorizationIssueCode: "CONNECTOR_CLI_AUTH_REQUIRED",
      profiles: [],
      selectedProfileAuthorized: false,
      selectedProfileConfigured: false,
      selectedUserName: null,
      version: "1.0.96",
    });
    commandMocks.deleteConnectorConfig.mockResolvedValue({
      connectors: [],
      credentialCleanupPending: false,
    });
  });
  afterEach(() => cleanup());

  /** 编辑发布目标时页面没有 App 凭据字段，保存只提交非秘密 profile。 */
  it("saves_only_a_non_secret_cli_profile", async () => {
    const user = userEvent.setup();
    await renderPage();
    await user.click(await screen.findByRole("button", { name: "编辑" }));

    expect(screen.queryByLabelText(/App Secret/)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/App ID/)).not.toBeInTheDocument();
    expect(screen.getByLabelText(/CLI profile/i)).toHaveValue("team-content");
    await user.click(screen.getByRole("button", { name: "保存发布目标" }));

    await waitFor(() =>
      expect(commandMocks.saveConnectorConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "connector-feishu",
          revision: 4,
          cliProfile: "team-content",
        }),
      ),
    );
  });

  /** 空配置首次进入会打开分步向导，并把完整文件夹链接安全回填为 token。 */
  it("guides_first_time_setup_and_prefills_the_target_form", async () => {
    commandMocks.listConnectorConfigs.mockResolvedValue([]);
    commandMocks.inspectLarkCliSetup.mockResolvedValue({
      authorizationIssueCode: null,
      profiles: [
        { name: "seo-publisher", tokenStatus: "valid", userName: "测试用户" },
      ],
      selectedProfileAuthorized: true,
      selectedProfileConfigured: true,
      selectedUserName: "测试用户",
      version: "1.0.96",
    });
    const user = userEvent.setup();
    await renderPage();

    expect(
      await screen.findByRole("dialog", { name: "飞书 CLI 只读检测" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "检测 CLI" }));
    await waitFor(() =>
      expect(commandMocks.inspectLarkCliSetup).toHaveBeenCalledWith(
        "seo-publisher",
      ),
    );
    await user.click(screen.getByRole("button", { name: "下一步" }));
    await user.type(
      screen.getByLabelText("飞书文件夹链接或 Token"),
      "https://example.feishu.cn/drive/folder/fldcnABC123?from=copy",
    );
    expect(
      screen.getByText(/已识别文件夹 Token：fldcnABC123/),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "填写发布目标" }));

    expect(screen.getByLabelText(/CLI profile/i)).toHaveValue("seo-publisher");
    expect(screen.getByLabelText(/目标文件夹 Token/)).toHaveValue(
      "fldcnABC123",
    );
  });

  /** 未登录的 profile 无法回填，应用不暴露创建或扫码入口。 */
  it("keeps_unverified_profile_out_of_the_target_form", async () => {
    commandMocks.listConnectorConfigs.mockResolvedValue([]);
    commandMocks.inspectLarkCliSetup.mockResolvedValue({
      authorizationIssueCode: "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED",
      profiles: [],
      selectedProfileAuthorized: false,
      selectedProfileConfigured: false,
      selectedUserName: null,
      version: "1.0.96",
    });
    const user = userEvent.setup();
    await renderPage();

    await user.click(await screen.findByRole("button", { name: "检测 CLI" }));
    expect(await screen.findByText(/所选 Profile 尚未配置/)).toBeVisible();
    expect(screen.getByRole("button", { name: "下一步" })).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: /扫码|创建 Profile/ }),
    ).toBeNull();
  });

  /** v11 迁移目标在补充 profile 前只能编辑，不能被启用或测试。 */
  it("keeps_a_migrated_target_disabled_until_a_profile_is_selected", async () => {
    commandMocks.listConnectorConfigs.mockResolvedValue([
      {
        ...SAVED_CONNECTOR,
        cliProfile: "",
        enabled: false,
        isDefault: false,
        revision: 5,
      },
    ]);
    const user = userEvent.setup();
    await renderPage();

    expect(await screen.findByText(/升级后待选择/)).toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: "允许用于文章发布" }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "测试连接" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "编辑" }));
    expect(screen.getByLabelText(/CLI profile/i)).toHaveValue("");
  });

  /** 向导会修复并显式启用迁移目标，保存后由用户手动测试连接。 */
  it("repairs_the_migrated_target_in_place_from_the_guide", async () => {
    const migrated = {
      ...SAVED_CONNECTOR,
      cliProfile: "",
      enabled: false,
      folderToken: "fld_old",
      isDefault: false,
      revision: 5,
    };
    commandMocks.listConnectorConfigs.mockResolvedValue([migrated]);
    commandMocks.inspectLarkCliSetup.mockResolvedValue({
      authorizationIssueCode: null,
      profiles: [
        { name: "seo-publisher", tokenStatus: "valid", userName: "测试用户" },
      ],
      selectedProfileAuthorized: true,
      selectedProfileConfigured: true,
      selectedUserName: "测试用户",
      version: "1.0.96",
    });
    const user = userEvent.setup();
    await renderPage();

    await user.click(await screen.findByRole("button", { name: "检测 CLI" }));
    await user.click(screen.getByRole("button", { name: "下一步" }));
    await user.type(
      screen.getByLabelText("飞书文件夹链接或 Token"),
      "https://example.feishu.cn/drive/folder/fldcnMigrated123",
    );
    await user.click(screen.getByRole("button", { name: "填写发布目标" }));

    expect(
      screen.getByRole("heading", { name: "编辑发布目标" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText(/CLI profile/i)).toHaveValue("seo-publisher");
    expect(screen.getByRole("switch", { name: "启用此连接器" })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "保存发布目标" }));
    await waitFor(() =>
      expect(commandMocks.saveConnectorConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          cliProfile: "seo-publisher",
          enabled: true,
          folderToken: "fldcnMigrated123",
          id: "connector-feishu",
          revision: 5,
        }),
      ),
    );
    expect(commandMocks.verifyConnectorConnection).not.toHaveBeenCalled();
    const testButton = await screen.findByRole("button", { name: "测试连接" });
    expect(testButton).toBeEnabled();
    await user.click(testButton);
    await waitFor(() =>
      expect(commandMocks.verifyConnectorConnection).toHaveBeenCalledWith(
        "connector-feishu",
      ),
    );
  });

  /** 手工编辑与只读引导共用同一组 profile 安全约束。 */
  it("blocks_profile_names_that_the_setup_guide_cannot_use", async () => {
    const user = userEvent.setup();
    await renderPage();
    await user.click(await screen.findByRole("button", { name: "编辑" }));

    const profileInput = screen.getByLabelText(/CLI profile/i);
    await user.clear(profileInput);
    await user.type(profileInput, "bad profile");

    expect(screen.getByText(/1–64 个不含空格/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存发布目标" })).toBeDisabled();
    expect(commandMocks.saveConnectorConfig).not.toHaveBeenCalled();
  });

  /** 连接测试保持只读，删除则必须经过独立确认弹窗。 */
  it("tests_read_only_and_requires_confirmation_before_delete", async () => {
    const user = userEvent.setup();
    await renderPage();

    await user.click(await screen.findByRole("button", { name: "测试连接" }));
    await waitFor(() =>
      expect(commandMocks.verifyConnectorConnection).toHaveBeenCalledWith(
        "connector-feishu",
      ),
    );
    expect(await screen.findByText(/目标文件夹可访问/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "删除" }));
    expect(commandMocks.deleteConnectorConfig).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    await waitFor(() =>
      expect(commandMocks.deleteConnectorConfig).toHaveBeenCalledWith(
        "connector-feishu",
        4,
      ),
    );
  });

  /** 慢删除期间确认与取消都锁定，避免第二次请求覆盖第一次成功结果。 */
  it("prevents_duplicate_delete_submissions", async () => {
    let resolveDelete: ((value: unknown) => void) | undefined;
    commandMocks.deleteConnectorConfig.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveDelete = resolve;
        }),
    );
    const user = userEvent.setup();
    await renderPage();

    await user.click(await screen.findByRole("button", { name: "删除" }));
    const confirm = screen.getByRole("button", { name: "确认删除" });
    await user.click(confirm);
    await waitFor(() => expect(confirm).toBeDisabled());
    await user.click(confirm);
    expect(commandMocks.deleteConnectorConfig).toHaveBeenCalledTimes(1);

    resolveDelete?.({ connectors: [], credentialCleanupPending: false });
    expect(await screen.findByText(/飞书内容库 已删除/)).toBeInTheDocument();
  });
});
