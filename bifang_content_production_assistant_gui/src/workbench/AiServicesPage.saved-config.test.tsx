import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
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

const providerMocks = vi.hoisted(() => ({
  deleteProviderConfig: vi.fn(),
  listProviderConfigs: vi.fn(),
  newProviderConfigDraft: vi.fn(),
  newVolcengineArkImageProviderConfigDraft: vi.fn(),
  saveProviderConfig: vi.fn(),
  setProviderEnabled: vi.fn(),
  setProviderStageAssignment: vi.fn(),
  testProviderImage: vi.fn(),
  verifyProviderConnection: vi.fn(),
}));

vi.mock("./workbenchCommands", () => ({ ...providerMocks }));
vi.mock("./workbenchErrors", () => ({
  localizedCommandError: (_reason: unknown, t: (key: string) => string) =>
    t("common.failed"),
}));

import { AiServicesPage } from "./AiServicesPage";

/** 为 Mantine 测试环境提供无布局副作用的尺寸观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时无需释放真实浏览器资源。 */
  disconnect(): void {}
  /** 测试不执行真实元素尺寸观察。 */
  observe(): void {}
  /** 测试不维护真实元素观察状态。 */
  unobserve(): void {}
}

/** 提供未保存的新建表单草稿，验证其不会被已保存卡片污染。 */
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

/** 提供火山方舟图片协议的 core 默认草稿。 */
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

/** 在中文应用上下文中渲染 AI 服务页。 */
async function renderPage(
  node: ReactElement,
  store: ReturnType<typeof createStore> = createStore(),
): Promise<ReturnType<typeof render>> {
  const instance = createInstance();
  await instance.init({
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
  return render(
    <I18nextProvider i18n={instance}>
      <JotaiProvider store={store}>
        <QueryClientProvider client={queryClient}>
          <MantineProvider env="test">{node}</MantineProvider>
        </QueryClientProvider>
      </JotaiProvider>
    </I18nextProvider>,
  );
}

describe("saved AI service configuration", () => {
  beforeEach(() => {
    for (const mock of Object.values(providerMocks)) mock.mockReset();
    providerMocks.newProviderConfigDraft.mockResolvedValue(providerDraft());
    providerMocks.newVolcengineArkImageProviderConfigDraft.mockResolvedValue(
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
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  /** 已保存 Fixture 明示三阶段参数，同时不暴露或暗示凭据。 */
  it("shows_each_saved_fixture_stage_configuration_without_credentials", async () => {
    providerMocks.listProviderConfigs.mockResolvedValue([
      {
        articleModel: "fixture-article-v1",
        articleParameters: {
          maxOutputTokens: 3_072,
          temperatureMilli: 650,
          timeoutMs: 61_000,
        },
        baseUrl: "",
        defaultForArticle: true,
        defaultForImage: true,
        defaultForSummary: true,
        enabled: true,
        hasApiKey: true,
        id: "provider-e2e-fixture",
        imageModel: "fixture-image-v1",
        imageParameters: {
          size: "1024x1536",
          timeoutMs: 93_000,
          watermark: false,
        },
        isTestFixture: true,
        kind: "fixture",
        name: "E2E 测试夹具",
        revision: 1,
        summaryModel: "fixture-summary-v1",
        summaryParameters: {
          maxOutputTokens: 384,
          temperatureMilli: 1_250,
          timeoutMs: 47_000,
        },
        textModel: "fixture-article-v1",
      },
    ]);
    providerMocks.verifyProviderConnection.mockResolvedValue({
      message: "fixture ready",
      ok: true,
    });
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    const serviceCard = await screen.findByRole("article", {
      name: "已保存服务：E2E 测试夹具",
    });
    expect(
      within(serviceCard).getByText("测试夹具 · 非真实 AI"),
    ).toBeInTheDocument();
    expect(
      within(serviceCard).getByText(
        "仅在显式调试 E2E 环境启用，结果不能代表真实 AI。",
      ),
    ).toBeInTheDocument();

    const article = screen.getByRole("article", {
      name: "正文 AI 服务：E2E 测试夹具",
    });
    expect(within(article).getByText("fixture-article-v1")).toBeInTheDocument();
    expect(within(article).getByText("温度：0.65")).toBeInTheDocument();
    expect(
      within(article).getByText("最大输出 token：3,072"),
    ).toBeInTheDocument();
    expect(within(article).getByText("超时：61 秒")).toBeInTheDocument();

    expect(within(serviceCard).queryByText(/模型：/u)).not.toBeInTheDocument();
    expect(within(serviceCard).queryByText(/密钥/u)).not.toBeInTheDocument();
    expect(
      within(serviceCard).queryByRole("button", { name: /编辑服务连接/u }),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText("API 密钥")).not.toBeInTheDocument();

    await user.click(
      within(article).getByRole("button", {
        name: "测试正文连接：E2E 测试夹具（已保存配置）",
      }),
    );
    await waitFor(() =>
      expect(providerMocks.verifyProviderConnection).toHaveBeenCalledWith(
        "provider-e2e-fixture",
        "article",
      ),
    );
    expect(
      await within(article).findByText(
        "正文 E2E 测试夹具可用；这不是一次真实 AI 验证。",
      ),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "简介 AI" }));
    const summary = screen.getByRole("article", {
      name: "简介 AI 服务：E2E 测试夹具",
    });
    expect(within(summary).getByText("fixture-summary-v1")).toBeInTheDocument();
    expect(within(summary).getByText("温度：1.25")).toBeInTheDocument();
    expect(
      within(summary).getByText("最大输出 token：384"),
    ).toBeInTheDocument();
    expect(within(summary).getByText("超时：47 秒")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "图片 AI" }));
    const image = screen.getByRole("article", {
      name: "图片 AI 服务：E2E 测试夹具",
    });
    expect(within(image).getByText("fixture-image-v1")).toBeInTheDocument();
    expect(within(image).getByText("尺寸：1024x1536")).toBeInTheDocument();
    expect(within(image).getByText("超时：93 秒")).toBeInTheDocument();
    expect(
      within(image).queryByRole("button", { name: /测试.*连接/u }),
    ).not.toBeInTheDocument();
    expect(
      within(image).queryByRole("button", { name: /生成测试图片/u }),
    ).not.toBeInTheDocument();
  });

  /** 三个一级步骤只暴露当前步骤字段，避免长表单再次混合。 */
  it("shows_only_the_active_stage_fields", async () => {
    providerMocks.listProviderConfigs.mockResolvedValue([
      {
        articleModel: "article-model",
        articleParameters: {
          maxOutputTokens: 2_048,
          temperatureMilli: 600,
          timeoutMs: 70_000,
        },
        baseUrl: "https://api.example.com/v1",
        defaultForArticle: true,
        defaultForImage: true,
        defaultForSummary: true,
        enabled: true,
        hasApiKey: true,
        id: "provider-all-stages",
        imageModel: "image-model",
        imageParameters: {
          size: "1536x1024",
          timeoutMs: 180_000,
          watermark: false,
        },
        isTestFixture: false,
        kind: "openAiCompatible",
        name: "三阶段共享服务",
        revision: 3,
        summaryModel: "summary-model",
        summaryParameters: {
          maxOutputTokens: 512,
          temperatureMilli: 200,
          timeoutMs: 50_000,
        },
        textModel: "article-model",
      },
    ]);
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    expect(await screen.findAllByRole("tab")).toHaveLength(3);
    expect(screen.getByRole("tab", { name: "正文 AI" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.click(
      screen.getByRole("button", { name: "配置正文：三阶段共享服务" }),
    );
    expect(await screen.findByLabelText(/正文模型/u)).toBeInTheDocument();
    expect(screen.queryByLabelText(/简介模型/u)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/图片模型/u)).not.toBeInTheDocument();

    expect(screen.getByRole("tab", { name: "简介 AI" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    await user.click(screen.getByRole("tab", { name: "简介 AI" }));
    await user.click(
      screen.getByRole("button", { name: "配置简介：三阶段共享服务" }),
    );
    expect(screen.queryByLabelText(/正文模型/u)).not.toBeInTheDocument();
    expect(await screen.findByLabelText(/简介模型/u)).toBeInTheDocument();
    expect(screen.queryByLabelText(/图片模型/u)).not.toBeInTheDocument();

    expect(screen.getByRole("tab", { name: "图片 AI" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    await user.click(await screen.findByRole("tab", { name: "图片 AI" }));
    await user.click(
      screen.getByRole("button", { name: "配置图片：三阶段共享服务" }),
    );
    expect(screen.queryByLabelText(/正文模型/u)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/简介模型/u)).not.toBeInTheDocument();
    expect(await screen.findByLabelText(/图片模型/u)).toBeInTheDocument();
  });

  /** 异步保存或验证期间锁定步骤页签，避免旧步骤结果回写到新页面。 */
  it("locks_stage_navigation_while_a_stage_operation_is_running", async () => {
    providerMocks.listProviderConfigs.mockResolvedValue([
      {
        articleModel: "article-model",
        articleParameters: {
          maxOutputTokens: 2_048,
          temperatureMilli: 600,
          timeoutMs: 70_000,
        },
        baseUrl: "https://api.example.com/v1",
        defaultForArticle: true,
        defaultForImage: false,
        defaultForSummary: false,
        enabled: true,
        hasApiKey: true,
        id: "provider-busy",
        imageModel: "",
        imageParameters: {
          size: "1536x1024",
          timeoutMs: 90_000,
          watermark: false,
        },
        isTestFixture: false,
        kind: "openAiCompatible",
        name: "忙碌服务",
        revision: 1,
        summaryModel: "",
        summaryParameters: {
          maxOutputTokens: null,
          temperatureMilli: null,
          timeoutMs: 60_000,
        },
        textModel: "article-model",
      },
    ]);
    let finishVerification!: (result: { message: string; ok: boolean }) => void;
    providerMocks.verifyProviderConnection.mockImplementation(
      () =>
        new Promise((resolve) => {
          finishVerification = resolve;
        }),
    );
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    await user.click(
      await screen.findByRole("button", {
        name: "测试正文连接：忙碌服务（已保存配置）",
      }),
    );
    const summaryTab = screen.getByRole("tab", { name: "简介 AI" });
    const imageTab = screen.getByRole("tab", { name: "图片 AI" });
    expect(summaryTab).toBeDisabled();
    expect(imageTab).toBeDisabled();

    finishVerification({ message: "ready", ok: true });
    await waitFor(() => expect(summaryTab).toBeEnabled());
    expect(imageTab).toBeEnabled();
  });

  /** 已保存或全局启用不等于步骤分配，步骤只展示明确选择的单一服务。 */
  it("shows_only_explicit_stage_assignments_and_filters_existing_choices", async () => {
    const assignedArticle = {
      ...providerDraft(),
      articleModel: "article-active",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: true,
      id: "provider-article-active",
      imageModel: "image-ready",
      isTestFixture: false,
      name: "正文当前服务",
      revision: 2,
      summaryModel: "summary-ready",
    };
    const unassignedText = {
      ...providerDraft(),
      articleModel: "article-spare",
      defaultForArticle: false,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: false,
      id: "provider-text-spare",
      imageModel: "image-spare",
      isTestFixture: false,
      name: "备用文本服务",
      revision: 4,
      summaryModel: "summary-spare",
    };
    const unassignedArk = {
      ...arkImageProviderDraft(),
      defaultForImage: false,
      hasApiKey: true,
      id: "provider-ark-spare",
      isTestFixture: false,
      name: "备用方舟服务",
      revision: 3,
    };
    const disabledText = {
      ...unassignedText,
      enabled: false,
      id: "provider-disabled",
      name: "已停用服务",
    };
    providerMocks.listProviderConfigs.mockResolvedValue([
      assignedArticle,
      unassignedText,
      unassignedArk,
      disabledText,
    ]);
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    expect(
      await screen.findByRole("article", {
        name: "正文 AI 服务：正文当前服务",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("article", {
        name: "正文 AI 服务：备用文本服务",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(
        screen.getByRole("article", { name: "已保存服务：备用文本服务" }),
      ).getByText("尚未用于任何步骤"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "选择已有连接" }));
    expect(
      await screen.findByRole("menuitem", {
        name: "选择备用文本服务用于正文",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("缺少密钥；运行前请先在下方连接管理中保存"),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("menuitem", {
        name: "选择备用方舟服务用于正文",
      }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("menuitem", { name: /已停用服务/u }),
    ).not.toBeInTheDocument();

    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("tab", { name: "图片 AI" }));
    expect(
      screen.queryByRole("article", {
        name: "图片 AI 服务：备用方舟服务",
      }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "选择已有连接" }));
    expect(
      await screen.findByRole("menuitem", {
        name: "选择备用方舟服务用于图片",
      }),
    ).toBeInTheDocument();
  });

  /** 显式选择和移出步骤只改变绑定，连接卡片与已保存配置始终保留。 */
  it("assigns_and_removes_an_existing_connection_without_deleting_it", async () => {
    let current = {
      ...providerDraft(),
      articleModel: "article-ready",
      defaultForArticle: false,
      defaultForImage: false,
      defaultForSummary: false,
      hasApiKey: true,
      id: "provider-ready",
      imageModel: "image-ready",
      isTestFixture: false,
      name: "备用服务",
      revision: 1,
      summaryModel: "summary-ready",
    };
    providerMocks.listProviderConfigs.mockResolvedValue([current]);
    providerMocks.setProviderStageAssignment.mockImplementation(
      async (
        _providerId: string,
        expectedRevision: number,
        _stage: string,
        assigned: boolean,
      ) => {
        current = {
          ...current,
          defaultForArticle: assigned,
          revision: expectedRevision + 1,
        };
        return { providers: [current], saved: current };
      },
    );
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    expect(
      await screen.findByRole("article", { name: "已保存服务：备用服务" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("article", { name: "正文 AI 服务：备用服务" }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "选择已有连接" }));
    await user.click(
      await screen.findByRole("menuitem", {
        name: "选择备用服务用于正文",
      }),
    );
    await waitFor(() =>
      expect(providerMocks.setProviderStageAssignment).toHaveBeenCalledWith(
        "provider-ready",
        1,
        "article",
        true,
      ),
    );
    expect(
      await screen.findByRole("article", { name: "正文 AI 服务：备用服务" }),
    ).toBeInTheDocument();
    expect(screen.getByText("用于正文")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "从正文移除备用服务" }),
    );
    await waitFor(() =>
      expect(providerMocks.setProviderStageAssignment).toHaveBeenNthCalledWith(
        2,
        "provider-ready",
        2,
        "article",
        false,
      ),
    );
    expect(
      screen.queryByRole("article", { name: "正文 AI 服务：备用服务" }),
    ).not.toBeInTheDocument();
    expect(
      within(
        screen.getByRole("article", { name: "已保存服务：备用服务" }),
      ).getByText("尚未用于任何步骤"),
    ).toBeInTheDocument();
    expect(screen.queryByText("用于正文")).not.toBeInTheDocument();
    expect(providerMocks.deleteProviderConfig).not.toHaveBeenCalled();
    expect(providerMocks.setProviderEnabled).not.toHaveBeenCalled();
    expect(providerMocks.saveProviderConfig).not.toHaveBeenCalled();
  });

  /** 图片步骤默认使用火山方舟协议，且不隐式绑定文本步骤。 */
  it("creates_a_volcengine_ark_service_for_only_the_image_stage", async () => {
    providerMocks.listProviderConfigs.mockResolvedValue([]);
    providerMocks.saveProviderConfig.mockImplementation(async (config) => {
      const saved = {
        ...config,
        hasApiKey: false,
        id: "provider-new-image",
        isTestFixture: false,
      };
      return { providers: [saved], saved };
    });
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    await user.click(await screen.findByRole("tab", { name: "图片 AI" }));
    const addImageButton = screen.getByRole("button", {
      name: "新建并用于图片",
    });
    await user.click(addImageButton);
    expect(screen.queryByLabelText(/正文模型/u)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/简介模型/u)).not.toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "图片服务协议" })).toHaveValue(
      "火山方舟 Ark v3（URL 响应）",
    );
    expect(screen.getByRole("textbox", { name: /API 根地址/u })).toHaveValue(
      "https://ark.cn-beijing.volces.com/api/v3",
    );
    expect(screen.getByText(/实际请求：/u)).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: /图片尺寸/u })).toHaveValue(
      "2K",
    );
    expect(
      screen.getByRole("switch", { name: "添加“AI 生成”水印" }),
    ).toBeChecked();
    expect(screen.getByLabelText("ARK API 密钥")).toBeInTheDocument();
    expect(
      screen.getByText(/sequential_image_generation=disabled/u),
    ).toBeInTheDocument();
    const arkEndpointId =
      await screen.findByLabelText(/火山方舟推理接入点 ID/u);
    await user.clear(arkEndpointId);
    await user.type(arkEndpointId, "ep-image-test");
    await user.click(screen.getByRole("button", { name: "保存图片服务" }));

    await waitFor(() =>
      expect(providerMocks.saveProviderConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          articleModel: "",
          defaultForArticle: false,
          defaultForImage: true,
          defaultForSummary: false,
          baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
          imageModel: "ep-image-test",
          imageParameters: {
            size: "2K",
            timeoutMs: 90_000,
            watermark: true,
          },
          kind: "volcengineArkImage",
          summaryModel: "",
        }),
        null,
      ),
    );
  });

  /** 火山方舟纯图片服务不会混入正文或简介列表，并在两个相关卡片上明确标识。 */
  it("keeps_volcengine_ark_services_in_the_image_stage", async () => {
    const arkProvider = {
      ...arkImageProviderDraft(),
      hasApiKey: true,
      id: "provider-ark-image",
      isTestFixture: false,
      revision: 2,
    };
    providerMocks.listProviderConfigs.mockResolvedValue([arkProvider]);
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    const connectionCard = await screen.findByRole("article", {
      name: "已保存服务：火山方舟图片",
    });
    expect(within(connectionCard).getByText("火山方舟")).toBeInTheDocument();
    expect(within(connectionCard).getByText("仅图片")).toBeInTheDocument();
    expect(
      screen.queryByRole("article", {
        name: "正文 AI 服务：火山方舟图片",
      }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "简介 AI" }));
    expect(
      screen.queryByRole("article", {
        name: "简介 AI 服务：火山方舟图片",
      }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "图片 AI" }));
    const imageCard = screen.getByRole("article", {
      name: "图片 AI 服务：火山方舟图片",
    });
    expect(within(imageCard).getByText("火山方舟")).toBeInTheDocument();
    expect(within(imageCard).getByText("仅图片")).toBeInTheDocument();
    expect(
      within(imageCard).getByText("火山方舟推理接入点 ID（model）"),
    ).toBeInTheDocument();
    expect(within(imageCard).getByText("尺寸：2K")).toBeInTheDocument();
    expect(within(imageCard).getByText("AI 水印：已启用")).toBeInTheDocument();
  });

  /** 新建图片服务切换协议时采用对应模板，但保留用户已选择的默认位。 */
  it("switches_image_protocol_templates_without_losing_the_default", async () => {
    providerMocks.listProviderConfigs.mockResolvedValue([]);
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    await user.click(await screen.findByRole("tab", { name: "图片 AI" }));
    await user.click(screen.getByRole("button", { name: "新建并用于图片" }));
    expect(
      screen.queryByRole("switch", { name: /设为图片的默认服务/u }),
    ).not.toBeInTheDocument();

    const protocol = screen.getByRole("combobox", { name: "图片服务协议" });
    await user.click(protocol);
    await user.click(
      await screen.findByRole("option", {
        name: "OpenAI Images（Base64 响应）",
      }),
    );
    expect(protocol).toHaveValue("OpenAI Images（Base64 响应）");
    expect(screen.getByRole("textbox", { name: /API 根地址/u })).toHaveValue(
      "https://api.openai.com/v1",
    );
    expect(screen.getByRole("combobox", { name: /图片尺寸/u })).toHaveValue(
      "1536x1024",
    );
    expect(screen.getByLabelText("API 密钥")).toBeInTheDocument();
    expect(
      screen.queryByRole("switch", { name: "添加“AI 生成”水印" }),
    ).not.toBeInTheDocument();

    await user.click(protocol);
    await user.click(
      await screen.findByRole("option", {
        name: "火山方舟 Ark v3（URL 响应）",
      }),
    );
    expect(protocol).toHaveValue("火山方舟 Ark v3（URL 响应）");
    expect(screen.getByRole("textbox", { name: /API 根地址/u })).toHaveValue(
      "https://ark.cn-beijing.volces.com/api/v3",
    );
    expect(screen.getByRole("combobox", { name: /图片尺寸/u })).toHaveValue(
      "2K",
    );
  });

  /** 图片步骤和共享连接分别保存时，都必须保留不属于当前编辑器的字段。 */
  it("preserves_hidden_stages_when_saving_image_and_connection_edits", async () => {
    const original = {
      articleModel: "article-stable",
      articleParameters: {
        maxOutputTokens: 2_048,
        temperatureMilli: 600,
        timeoutMs: 70_000,
      },
      baseUrl: "https://api.example.com/v1",
      defaultForArticle: true,
      defaultForImage: true,
      defaultForSummary: true,
      enabled: true,
      hasApiKey: true,
      id: "provider-preserve",
      imageModel: "image-old",
      imageParameters: {
        size: "1536x1024" as const,
        timeoutMs: 180_000,
        watermark: false,
      },
      isTestFixture: false,
      kind: "openAiCompatible" as const,
      name: "保留字段服务",
      revision: 4,
      summaryModel: "summary-stable",
      summaryParameters: {
        maxOutputTokens: 512,
        temperatureMilli: 200,
        timeoutMs: 50_000,
      },
      textModel: "article-stable",
    };
    providerMocks.listProviderConfigs.mockResolvedValue([original]);
    providerMocks.saveProviderConfig.mockImplementation(async (config) => {
      const saved = {
        ...config,
        hasApiKey: true,
        isTestFixture: false,
        revision: config.revision + 1,
      };
      return { providers: [saved], saved };
    });
    const user = userEvent.setup();
    await renderPage(<AiServicesPage />);

    await user.click(await screen.findByRole("tab", { name: "图片 AI" }));
    await user.click(
      screen.getByRole("button", { name: "配置图片：保留字段服务" }),
    );
    const imageModel = screen.getByLabelText(/图片模型/u);
    await user.clear(imageModel);
    await user.type(imageModel, "image-new");
    await user.click(screen.getByRole("button", { name: "保存图片配置" }));
    await waitFor(() =>
      expect(providerMocks.saveProviderConfig).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({
          articleModel: "article-stable",
          defaultForArticle: true,
          defaultForImage: true,
          defaultForSummary: true,
          imageModel: "image-new",
          summaryModel: "summary-stable",
        }),
        null,
      ),
    );
    expect(screen.getByRole("tab", { name: "正文 AI" })).toBeEnabled();

    await user.click(
      screen.getByRole("button", { name: "编辑服务连接：保留字段服务" }),
    );
    const name = screen.getByLabelText(/服务名称/u);
    await user.clear(name);
    await user.type(name, "连接改名");
    await user.click(screen.getByRole("button", { name: "保存服务连接" }));
    await waitFor(() =>
      expect(providerMocks.saveProviderConfig).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({
          articleModel: "article-stable",
          imageModel: "image-new",
          name: "连接改名",
          summaryModel: "summary-stable",
        }),
        null,
      ),
    );
  });

  /** 当前步骤在同一根 store 中跨挂载保留，新 store 恢复正文。 */
  it("keeps_the_active_stage_in_the_session_store", async () => {
    providerMocks.listProviderConfigs.mockResolvedValue([]);
    const store = createStore();
    const user = userEvent.setup();
    const first = await renderPage(<AiServicesPage />, store);
    await user.click(await screen.findByRole("tab", { name: "图片 AI" }));
    first.unmount();

    const second = await renderPage(<AiServicesPage />, store);
    expect(await screen.findByRole("tab", { name: "图片 AI" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    second.unmount();

    await renderPage(<AiServicesPage />, createStore());
    expect(await screen.findByRole("tab", { name: "正文 AI" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
});
