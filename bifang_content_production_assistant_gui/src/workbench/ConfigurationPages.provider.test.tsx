import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createInstance } from "i18next";
import { createStore, Provider as JotaiProvider } from "jotai";
import type { ReactElement } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";

const configMocks = vi.hoisted(() => ({
  deleteProviderConfig: vi.fn(),
  listPromptTemplates: vi.fn(),
  listProviderConfigs: vi.fn(),
  newProviderConfigDraft: vi.fn(),
  newVolcengineArkImageProviderConfigDraft: vi.fn(),
  previewPromptTemplate: vi.fn(),
  savePromptTemplate: vi.fn(),
  saveProviderConfig: vi.fn(),
  setProviderEnabled: vi.fn(),
  setProviderStageAssignment: vi.fn(),
  testProviderImage: vi.fn(),
  verifyProviderConnection: vi.fn(),
}));

vi.mock("./workbenchCommands", () => ({
  ...configMocks,
}));

vi.mock("./workbenchErrors", () => ({
  localizedCommandError: (reason: unknown, t: (key: string) => string) => {
    const code =
      typeof reason === "object" && reason !== null
        ? Reflect.get(reason, "code")
        : null;
    return typeof code === "string" ? t(`errors.${code}`) : t("common.failed");
  },
}));

import { AiServicesPage } from "./AiServicesPage";

/** 建立三阶段显式模型与参数，避免测试退回旧共享文本模型。 */
function stageConfiguration(
  articleModel: string,
  summaryModel: string,
  imageModel: string,
) {
  return {
    articleModel,
    articleParameters: {
      maxOutputTokens: 4_096,
      temperatureMilli: 700,
      timeoutMs: 60_000,
    },
    imageModel,
    imageParameters: {
      size: "1536x1024" as const,
      timeoutMs: 90_000,
      watermark: false,
    },
    enabled: true,
    summaryModel,
    summaryParameters: {
      maxOutputTokens: 512,
      temperatureMilli: 200,
      timeoutMs: 45_000,
    },
    textModel: articleModel || summaryModel,
  };
}

/** 模拟 core 返回的新服务表单默认值。 */
function providerDraft() {
  return {
    articleModel: "",
    articleParameters: {
      maxOutputTokens: null,
      temperatureMilli: null,
      timeoutMs: 60_000,
    },
    baseUrl: "https://api.openai.com/v1",
    defaultForArticle: true,
    defaultForImage: true,
    defaultForSummary: true,
    enabled: true,
    id: "",
    imageModel: "",
    imageParameters: {
      size: "1536x1024" as const,
      timeoutMs: 90_000,
      watermark: false,
    },
    kind: "openAiCompatible" as const,
    name: "OpenAI Compatible",
    revision: 1,
    summaryModel: "",
    summaryParameters: {
      maxOutputTokens: null,
      temperatureMilli: null,
      timeoutMs: 60_000,
    },
    textModel: "",
  };
}

/** 模拟 core 返回的火山方舟图片服务默认值。 */
function arkImageProviderDraft() {
  return {
    ...providerDraft(),
    baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
    defaultForArticle: false,
    defaultForSummary: false,
    imageModel: "ep-20260318093407-x5qm5",
    imageParameters: {
      size: "2K" as const,
      timeoutMs: 90_000,
      watermark: true,
    },
    kind: "volcengineArkImage" as const,
    name: "火山方舟图片",
  };
}

/** 为 Mantine 测试提供不执行真实测量的观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}
  /** jsdom 不执行真实布局测量。 */
  observe(): void {}
  /** jsdom 允许停止观察但没有持久状态。 */
  unobserve(): void {}
}

/** 在真实前端 Provider 组合下渲染配置页面。 */
async function renderConfiguration(
  node: ReactElement,
  language = "zh-CN",
): Promise<QueryClient> {
  const instance = createInstance();
  await instance.init({
    fallbackLng: "en-US",
    lng: language,
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={instance}>
      <JotaiProvider store={createStore()}>
        <QueryClientProvider client={queryClient}>
          <MantineProvider env="test">{node}</MantineProvider>
        </QueryClientProvider>
      </JotaiProvider>
    </I18nextProvider>,
  );
  return queryClient;
}

describe("prompt and provider configuration pages", () => {
  beforeEach(() => {
    for (const mock of Object.values(configMocks)) mock.mockReset();
    configMocks.newProviderConfigDraft.mockResolvedValue(providerDraft());
    configMocks.newVolcengineArkImageProviderConfigDraft.mockResolvedValue(
      arkImageProviderDraft(),
    );
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
    Object.defineProperty(Element.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: {
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      },
    });
  });
  afterEach(() => cleanup());

  /** AI 服务后台刷新失败时保留旧卡片，并允许就地重新读取。 */
  it("keeps_provider_data_visible_during_a_failed_background_refresh", async () => {
    const provider = {
      baseUrl: "https://api.example.com/v1",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: true,
      id: "provider-refresh",
      ...stageConfiguration("article-model", "", ""),
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "刷新保留服务",
      revision: 1,
    };
    configMocks.listProviderConfigs.mockResolvedValueOnce([provider]);
    const queryClient = await renderConfiguration(<AiServicesPage />);
    await screen.findAllByText("刷新保留服务");

    let rejectRefresh!: (reason: unknown) => void;
    configMocks.listProviderConfigs.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          rejectRefresh = reject;
        }),
    );
    const refresh = queryClient.refetchQueries({
      queryKey: ["provider-configs"],
    });
    expect(
      await screen.findByText("正在刷新 AI 服务配置…"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("刷新保留服务")).toHaveLength(2);
    rejectRefresh(new Error("refresh failed"));
    await refresh;

    const refreshAlert = (
      await screen.findByText(
        "刷新 AI 服务配置失败；仍显示上次成功读取的本地数据。",
      )
    ).closest('[role="alert"]');
    expect(refreshAlert).not.toBeNull();
    expect(screen.getAllByText("刷新保留服务")).toHaveLength(2);
    configMocks.listProviderConfigs.mockResolvedValueOnce([provider]);
    await userEvent
      .setup()
      .click(within(refreshAlert as HTMLElement).getByRole("button"));
    await waitFor(() => expect(refreshAlert).not.toBeInTheDocument());
  });

  /** 启停只响应独立开关，停用后禁止连接测试，并经确认删除。 */
  it("toggles_and_confirm_deletes_one_saved_provider", async () => {
    const provider = {
      baseUrl: "https://api.openai.com/v1",
      defaultForArticle: true,
      defaultForImage: true,
      defaultForSummary: true,
      hasApiKey: true,
      id: "provider-lifecycle",
      ...stageConfiguration("gpt-4o-mini", "gpt-4o-mini", "gpt-image"),
      isTestFixture: false,
      kind: "openAiCompatible" as const,
      name: "可切换服务",
      revision: 1,
    };
    const disabled = {
      ...provider,
      defaultForArticle: false,
      defaultForImage: false,
      defaultForSummary: false,
      enabled: false,
      revision: 2,
    };
    configMocks.listProviderConfigs.mockResolvedValue([provider]);
    configMocks.setProviderEnabled.mockResolvedValue({
      providers: [disabled],
      saved: disabled,
    });
    configMocks.deleteProviderConfig.mockResolvedValue({
      credentialCleanupPending: false,
      historicalBindingsDetached: 7,
      providers: [],
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);
    const card = await screen.findByRole("article", {
      name: "已保存服务：可切换服务",
    });

    await user.click(within(card).getByText("可切换服务"));
    expect(configMocks.setProviderEnabled).not.toHaveBeenCalled();
    await user.click(
      within(card).getByRole("switch", {
        name: "启用或停用服务连接：可切换服务",
      }),
    );
    await waitFor(() =>
      expect(configMocks.setProviderEnabled).toHaveBeenCalledWith(
        "provider-lifecycle",
        1,
        false,
      ),
    );
    await waitFor(() =>
      expect(within(card).getByText("已停用")).toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("article", {
        name: "正文 AI 服务：可切换服务",
      }),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "删除 AI 服务：可切换服务" }),
    );
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(configMocks.deleteProviderConfig).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "删除 AI 服务：可切换服务" }),
    );
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    await waitFor(() =>
      expect(configMocks.deleteProviderConfig).toHaveBeenCalledWith(
        "provider-lifecycle",
        2,
      ),
    );
    expect(
      screen.queryByRole("article", { name: "已保存服务：可切换服务" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText(
        "可切换服务 已删除；保留了 7 个历史阶段快照并解除其密钥引用，关联系统凭据已安全清理。",
      ),
    ).toBeInTheDocument();
  });

  /** 删除失败必须在确认弹窗内直接说明原因，并保留候选供用户处理后重试。 */
  it("keeps_delete_dialog_open_and_shows_the_failure_in_context", async () => {
    const provider = {
      baseUrl: "https://api.openai.com/v1",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: true,
      id: "provider-delete-failure",
      ...stageConfiguration("gpt-4o-mini", "", ""),
      isTestFixture: false,
      kind: "openAiCompatible" as const,
      name: "仍用于正文",
      revision: 3,
    };
    configMocks.listProviderConfigs.mockResolvedValue([provider]);
    configMocks.deleteProviderConfig.mockRejectedValue({
      code: "PROVIDER_DELETE_DEFAULT_IN_USE",
      message: "ignored",
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await user.click(
      await screen.findByRole("button", {
        name: "删除 AI 服务：仍用于正文",
      }),
    );
    await user.click(screen.getByRole("button", { name: "确认删除" }));

    const dialog = await screen.findByRole("dialog", { name: "删除 AI 服务" });
    expect(
      within(dialog).getByText("该服务仍被生成步骤使用，请先从对应步骤移除。"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("article", { name: "已保存服务：仍用于正文" }),
    ).toBeInTheDocument();
  });

  /** 成功后台刷新只更新卡片，不得覆盖显式编辑会话中的草稿或新密钥。 */
  it("preserves_unsaved_provider_editor_and_api_key_after_background_refresh", async () => {
    const provider = {
      baseUrl: "https://api.example.com/v1",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: true,
      id: "provider-refresh-success",
      ...stageConfiguration("article-model", "", ""),
      isTestFixture: false,
      kind: "openAiCompatible" as const,
      name: "刷新前服务",
      revision: 1,
    };
    configMocks.listProviderConfigs.mockResolvedValueOnce([provider]);
    const user = userEvent.setup();
    const queryClient = await renderConfiguration(<AiServicesPage />);
    await user.click(
      await screen.findByRole("button", {
        name: "编辑服务连接：刷新前服务",
      }),
    );
    const nameInput = await screen.findByLabelText(/服务名称/u);
    const endpointInput = screen.getByLabelText(/API 根地址/u);
    const keyInput = screen.getByLabelText("API 密钥");

    let resolveRefresh!: (providers: Array<typeof provider>) => void;
    configMocks.listProviderConfigs.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveRefresh = resolve;
        }),
    );
    const refresh = queryClient.refetchQueries({
      queryKey: ["provider-configs"],
    });
    await screen.findByText("正在刷新 AI 服务配置…");
    await user.clear(nameInput);
    await user.type(nameInput, "未保存本地名称");
    await user.clear(endpointInput);
    await user.type(endpointInput, "https://unsaved.example.com/v1");
    expect(keyInput).toBeRequired();
    await user.type(keyInput, "unsaved-key");
    resolveRefresh([
      {
        ...provider,
        hasApiKey: false,
        name: "服务器刷新名称",
        revision: 2,
      },
    ]);
    await refresh;

    expect(await screen.findAllByText("服务器刷新名称")).toHaveLength(2);
    expect(nameInput).toHaveValue("未保存本地名称");
    expect(endpointInput).toHaveValue("https://unsaved.example.com/v1");
    expect(keyInput).toHaveValue("unsaved-key");
    expect(keyInput).not.toBeRequired();
    expect(screen.getByText("当前尚未保存密钥。")).toBeInTheDocument();
  });

  /** 已保存密钥只显示存在状态，密码字段保持空白且连接测试只由按钮触发。 */
  it("never_refills_api_keys_or_tests_connections_implicitly", async () => {
    const provider = {
      baseUrl: "https://api.example.com/v1",
      defaultForImage: true,
      defaultForArticle: true,
      defaultForSummary: true,
      hasApiKey: true,
      id: "provider-1",
      ...stageConfiguration("article-model", "summary-model", "image-model"),
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "真实服务",
      revision: 1,
    };
    configMocks.listProviderConfigs.mockResolvedValue([provider]);
    configMocks.verifyProviderConnection.mockResolvedValue({
      message: "连接成功",
      ok: true,
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await screen.findAllByText("真实服务");
    expect(configMocks.verifyProviderConnection).not.toHaveBeenCalled();

    const articleVerifyButton = screen.getByRole("button", {
      name: "测试正文连接：真实服务（已保存配置）",
    });
    await user.click(articleVerifyButton);
    await waitFor(() =>
      expect(configMocks.verifyProviderConnection).toHaveBeenCalledWith(
        "provider-1",
        "article",
      ),
    );
    await user.click(screen.getByRole("tab", { name: "简介 AI" }));
    await user.click(
      screen.getByRole("button", {
        name: "测试简介连接：真实服务（已保存配置）",
      }),
    );
    await waitFor(() =>
      expect(configMocks.verifyProviderConnection).toHaveBeenCalledWith(
        "provider-1",
        "summary",
      ),
    );

    await user.click(
      screen.getByRole("button", { name: "编辑服务连接：真实服务" }),
    );
    const keyInput = screen.getByLabelText("API 密钥");
    expect(keyInput).toHaveValue("");
    const endpoint = screen.getByLabelText(/API 根地址/u);
    await user.clear(endpoint);
    await user.type(endpoint, "https://new.example.com/v1");
    expect(keyInput).toHaveValue("");
    expect(
      screen.getByText(/API 根地址已改变；保存时必须重新输入密钥/u),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存服务连接" })).toBeDisabled();
    await user.type(keyInput, "replacement-key");
    expect(screen.getByRole("button", { name: "保存服务连接" })).toBeEnabled();
  });

  /** 没有旧密钥的配置可以只改地址，不能虚构一项密钥轮换义务。 */
  it("allows_an_endpoint_change_without_a_key_when_none_is_saved", async () => {
    const provider = {
      baseUrl: "https://api.example.com/v1",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: false,
      id: "provider-without-key",
      ...stageConfiguration("article-model", "", ""),
      isTestFixture: false,
      kind: "openAiCompatible" as const,
      name: "未配置密钥服务",
      revision: 1,
    };
    configMocks.listProviderConfigs.mockResolvedValue([provider]);
    configMocks.saveProviderConfig.mockImplementation(async (config) => {
      const saved = {
        ...config,
        hasApiKey: false,
        isTestFixture: false,
        revision: 2,
      };
      return { providers: [saved], saved };
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);
    await user.click(
      await screen.findByRole("button", {
        name: "编辑服务连接：未配置密钥服务",
      }),
    );
    const endpoint = screen.getByLabelText(/API 根地址/u);
    const keyInput = screen.getByLabelText("API 密钥");
    await user.clear(endpoint);
    await user.type(endpoint, "https://new.example.com/v1");

    expect(keyInput).not.toBeRequired();
    expect(screen.getByText("当前尚未保存密钥。")).toBeInTheDocument();
    const save = screen.getByRole("button", { name: "保存服务连接" });
    expect(save).toBeEnabled();
    await user.click(save);
    await waitFor(() =>
      expect(configMocks.saveProviderConfig).toHaveBeenCalledWith(
        expect.objectContaining({ baseUrl: "https://new.example.com/v1" }),
        null,
      ),
    );
  });

  /** 文本连接失败会保留具体阶段，避免同卡两个测试入口产生歧义。 */
  it("labels_a_failed_connection_with_its_text_stage", async () => {
    configMocks.listProviderConfigs.mockResolvedValue([
      {
        baseUrl: "https://api.example.com/v1",
        defaultForArticle: true,
        defaultForImage: false,
        defaultForSummary: true,
        hasApiKey: true,
        id: "provider-1",
        ...stageConfiguration("article-model", "summary-model", ""),
        isTestFixture: false,
        kind: "openAiCompatible",
        name: "真实服务",
        revision: 1,
      },
    ]);
    configMocks.verifyProviderConnection.mockRejectedValue({
      code: "PROVIDER_AUTHENTICATION",
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await user.click(await screen.findByRole("tab", { name: "简介 AI" }));
    await user.click(
      screen.getByRole("button", {
        name: "测试简介连接：真实服务（已保存配置）",
      }),
    );
    const alert = (await screen.findByText("简介连接测试失败")).closest(
      '[role="alert"]',
    );
    expect(alert).not.toBeNull();
    expect(alert).toHaveTextContent("简介连接测试失败");
    expect(alert).toHaveTextContent("AI 服务鉴权失败，请检查凭据。");
  });

  /** 已解析但 `ok=false` 的连接结果仍是红色错误，不得伪装成成功状态。 */
  it("renders_a_resolved_unsuccessful_connection_result_as_an_alert", async () => {
    configMocks.listProviderConfigs.mockResolvedValue([
      {
        baseUrl: "https://api.example.com/v1",
        defaultForArticle: true,
        defaultForImage: false,
        defaultForSummary: false,
        hasApiKey: true,
        id: "provider-resolved-failure",
        ...stageConfiguration("article-model", "", ""),
        isTestFixture: false,
        kind: "openAiCompatible",
        name: "返回失败服务",
        revision: 1,
      },
    ]);
    configMocks.verifyProviderConnection.mockResolvedValue({
      message: "安全失败说明",
      ok: false,
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await user.click(
      await screen.findByRole("button", {
        name: "测试正文连接：返回失败服务（已保存配置）",
      }),
    );
    const alert = (await screen.findByText("安全失败说明")).closest(
      '[role="alert"]',
    );
    expect(alert).not.toBeNull();
    expect(alert).toHaveTextContent("正文连接测试失败");
    expect(screen.queryByText(/连接成功/u)).not.toBeInTheDocument();
  });

  /** 正文、简介和配图可以由不同 Provider 承担，保存时三个默认位不会合并。 */
  it("saves_independent_provider_defaults_for_each_stage", async () => {
    const articleProvider = {
      baseUrl: "https://article.example.com/v1",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: true,
      id: "provider-article",
      ...stageConfiguration("article-model", "", ""),
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "正文服务",
      revision: 2,
    } as const;
    const downstreamProvider = {
      baseUrl: "https://downstream.example.com/v1",
      defaultForArticle: false,
      defaultForImage: true,
      defaultForSummary: true,
      hasApiKey: true,
      id: "provider-downstream",
      ...stageConfiguration("", "summary-model", "image-model"),
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "简介与配图服务",
      revision: 4,
    } as const;
    configMocks.listProviderConfigs.mockResolvedValue([
      articleProvider,
      downstreamProvider,
    ]);
    let currentProviders: Array<Record<string, unknown>> = [
      articleProvider,
      downstreamProvider,
    ];
    configMocks.saveProviderConfig.mockImplementation(
      async (config: Record<string, unknown>) => {
        const saved = {
          ...config,
          hasApiKey: true,
          isTestFixture: false,
          revision: Number(config.revision) + 1,
        } as Record<string, unknown>;
        currentProviders = currentProviders.map((provider) => {
          if (provider.id === saved.id) return saved;
          const disableArticle =
            saved.defaultForArticle === true &&
            provider.defaultForArticle === true;
          const disableSummary =
            saved.defaultForSummary === true &&
            provider.defaultForSummary === true;
          const disableImage =
            saved.defaultForImage === true && provider.defaultForImage === true;
          if (!disableArticle && !disableSummary && !disableImage) {
            return provider;
          }
          return {
            ...provider,
            defaultForArticle: disableArticle
              ? false
              : provider.defaultForArticle,
            defaultForImage: disableImage ? false : provider.defaultForImage,
            defaultForSummary: disableSummary
              ? false
              : provider.defaultForSummary,
            revision: Number(provider.revision) + 1,
          };
        });
        return { providers: currentProviders, saved };
      },
    );
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await screen.findAllByText("正文服务");
    expect(screen.getByText("当前使用：正文服务")).toBeInTheDocument();
    const editArticleProvider = screen.getByRole("button", {
      name: "配置正文：正文服务",
    });
    await user.click(editArticleProvider);
    expect(
      screen.queryByRole("switch", { name: /设为正文的默认服务/u }),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/简介模型/u)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/图片模型/u)).not.toBeInTheDocument();

    const articleModel = screen.getByLabelText(/正文模型/u);
    await user.clear(articleModel);
    await user.type(articleModel, " article-model-v2 ");
    const articleTemperature = screen.getByLabelText("正文温度（0–2，可留空）");
    await user.clear(articleTemperature);
    await user.click(articleTemperature);
    await user.paste("1.25");
    const articleTokens = screen.getByLabelText("正文最大输出 token（可留空）");
    await user.clear(articleTokens);
    await user.type(articleTokens, "2048");
    const articleTimeout = screen.getByLabelText(/正文超时（秒）/u);
    await user.clear(articleTimeout);
    await user.type(articleTimeout, "75");

    await user.click(screen.getByRole("button", { name: "保存正文配置" }));

    expect(configMocks.saveProviderConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        articleModel: "article-model-v2",
        articleParameters: {
          maxOutputTokens: 2_048,
          temperatureMilli: 1_250,
          timeoutMs: 75_000,
        },
        defaultForArticle: true,
        defaultForImage: false,
        defaultForSummary: false,
        id: "provider-article",
        imageModel: "",
        imageParameters: {
          size: "1536x1024",
          timeoutMs: 90_000,
          watermark: false,
        },
        summaryModel: "",
        summaryParameters: {
          maxOutputTokens: 512,
          temperatureMilli: 200,
          timeoutMs: 45_000,
        },
        textModel: "article-model-v2",
      }),
      null,
    );
    await screen.findByText("正文服务的正文配置已保存；另外两个步骤保持不变。");

    await user.click(screen.getByRole("tab", { name: "简介 AI" }));
    await user.click(
      screen.getByRole("button", { name: "配置简介：简介与配图服务" }),
    );
    expect(
      screen.queryByRole("switch", { name: /设为简介的默认服务/u }),
    ).not.toBeInTheDocument();
    const summaryModel = screen.getByLabelText(/简介模型/u);
    await user.clear(summaryModel);
    await user.type(summaryModel, " summary-model-v2 ");
    const summaryTemperature = screen.getByLabelText("简介温度（0–2，可留空）");
    await user.clear(summaryTemperature);
    await user.click(summaryTemperature);
    await user.paste("0.4");
    const summaryTokens = screen.getByLabelText("简介最大输出 token（可留空）");
    await user.clear(summaryTokens);
    await user.type(summaryTokens, "600");
    const summaryTimeout = screen.getByLabelText(/简介超时（秒）/u);
    await user.clear(summaryTimeout);
    await user.type(summaryTimeout, "50");
    expect(screen.queryByLabelText(/正文模型/u)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/图片模型/u)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保存简介配置" }));

    expect(configMocks.saveProviderConfig).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        articleModel: "",
        defaultForArticle: false,
        defaultForImage: true,
        defaultForSummary: true,
        id: "provider-downstream",
        imageModel: "image-model",
        imageParameters: {
          size: "1536x1024",
          timeoutMs: 90_000,
          watermark: false,
        },
        summaryModel: "summary-model-v2",
        summaryParameters: {
          maxOutputTokens: 600,
          temperatureMilli: 400,
          timeoutMs: 50_000,
        },
        textModel: "summary-model-v2",
      }),
      null,
    );
    await screen.findByText(
      "简介与配图服务的简介配置已保存；另外两个步骤保持不变。",
    );
  });

  /** 图片测试必须先确认费用，并只在确认后发出一次绑定修订的请求。 */
  it("confirms_one_paid_image_test_and_shows_only_a_temporary_preview", async () => {
    configMocks.listProviderConfigs.mockResolvedValue([
      {
        baseUrl: "https://image.example.com/v1",
        defaultForArticle: false,
        defaultForImage: true,
        defaultForSummary: false,
        hasApiKey: true,
        id: "provider-image",
        ...stageConfiguration("", "", "gpt-image-1"),
        isTestFixture: false,
        kind: "openAiCompatible",
        name: "纯配图服务",
        revision: 1,
      },
    ]);
    const imageTestResult = {
      byteLength: 8,
      imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
      mimeType: "image/png",
      providerId: "provider-image",
      providerRevision: 1,
    };
    let resolveImageTest!: (result: typeof imageTestResult) => void;
    configMocks.testProviderImage.mockReturnValue(
      new Promise((resolve) => {
        resolveImageTest = resolve;
      }),
    );
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await screen.findAllByText("纯配图服务");
    await user.click(screen.getByRole("tab", { name: "图片 AI" }));
    const open = screen.getByRole("button", {
      name: "生成测试图片：纯配图服务（已保存配置）",
    });
    await user.click(open);

    expect(configMocks.testProviderImage).not.toHaveBeenCalled();
    expect(
      screen.getByRole("dialog", { name: "生成图片测试" }),
    ).toHaveTextContent("该操作可能产生一次图片生成费用");
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(configMocks.testProviderImage).not.toHaveBeenCalled();

    await user.click(open);
    const confirm = screen.getByRole("button", {
      name: "确认并生成 1 张",
    });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() =>
      expect(configMocks.testProviderImage).toHaveBeenCalledWith(
        "provider-image",
        1,
      ),
    );
    expect(configMocks.testProviderImage).toHaveBeenCalledTimes(1);
    resolveImageTest(imageTestResult);
    expect(
      await screen.findByRole("img", {
        name: "纯配图服务生成的临时测试图片",
      }),
    ).toHaveAttribute("src", "data:image/png;base64,iVBORw0KGgo=");
    expect(
      screen.queryByRole("button", { name: "确认并生成 1 张" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(
      screen.queryByRole("img", { name: /临时测试图片/u }),
    ).not.toBeInTheDocument();
    expect(configMocks.verifyProviderConnection).not.toHaveBeenCalled();
  });

  /** 图片测试结果不确定时弹窗保留，且不能在同一次确认中再次付费请求。 */
  it("keeps_an_uncertain_image_test_visible_without_an_inline_retry", async () => {
    configMocks.listProviderConfigs.mockResolvedValue([
      {
        baseUrl: "https://image.example.com/v1",
        defaultForArticle: false,
        defaultForImage: true,
        defaultForSummary: false,
        hasApiKey: true,
        id: "provider-image-uncertain",
        ...stageConfiguration("", "", "gpt-image-1"),
        isTestFixture: false,
        kind: "openAiCompatible",
        name: "结果不明图片服务",
        revision: 4,
      },
    ]);
    configMocks.testProviderImage.mockRejectedValue({
      code: "PROVIDER_IMAGE_TEST_RESULT_UNCERTAIN",
    });
    const user = userEvent.setup();
    await renderConfiguration(<AiServicesPage />);

    await user.click(await screen.findByRole("tab", { name: "图片 AI" }));
    await user.click(
      screen.getByRole("button", {
        name: "生成测试图片：结果不明图片服务（已保存配置）",
      }),
    );
    await user.click(screen.getByRole("button", { name: "确认并生成 1 张" }));

    const dialog = screen.getByRole("dialog", { name: "生成图片测试" });
    const errorText = await within(dialog).findByText(/图片测试结果无法确认/u);
    const alert = errorText.closest('[role="alert"]');
    expect(alert).not.toBeNull();
    expect(alert).toHaveTextContent("可能已经产生费用");
    expect(dialog).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "确认并生成 1 张" }),
    ).not.toBeInTheDocument();
    expect(configMocks.testProviderImage).toHaveBeenCalledTimes(1);
  });
});
