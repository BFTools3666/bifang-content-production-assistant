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

import { PromptTemplatesPage } from "./PromptTemplatesPage";

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

/** 建立页面必须完整接收的三类 Prompt 视图。 */
function promptTemplateViews() {
  return [
    {
      allowedVariables: ["title", "keywords", "requirements"],
      content: "撰写 {{title}}",
      id: "article",
      kind: "article",
      name: "正文",
      requiredVariables: ["title"],
      version: 1,
    },
    {
      allowedVariables: ["title", "article"],
      content: "压缩 {{article}}",
      id: "summary",
      kind: "summary",
      name: "简介",
      requiredVariables: ["article"],
      version: 1,
    },
    {
      allowedVariables: ["title", "article", "summary"],
      content: "绘制 {{title}} 与 {{summary}}",
      id: "image",
      kind: "image",
      name: "配图",
      requiredVariables: ["title", "summary"],
      version: 1,
    },
  ];
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

  /** Prompt 预览只调用本地预览命令，保存则明确创建后端新版本。 */
  it("previews_locally_and_saves_a_new_prompt_version", async () => {
    const articleTemplate = {
      allowedVariables: ["title", "keywords", "requirements"],
      content: "撰写 {{title}}",
      id: "article",
      kind: "article",
      name: "正文",
      requiredVariables: ["title"],
      version: 1,
    };
    const templates = [
      articleTemplate,
      {
        allowedVariables: ["title", "article"],
        content: "压缩 {{article}}",
        id: "summary",
        kind: "summary",
        name: "简介",
        requiredVariables: ["article"],
        version: 1,
      },
      {
        allowedVariables: ["title", "article", "summary"],
        content: "绘制 {{title}} 与 {{summary}}",
        id: "image",
        kind: "image",
        name: "配图",
        requiredVariables: ["title", "summary"],
        version: 1,
      },
    ];
    configMocks.listPromptTemplates.mockResolvedValue(templates);
    configMocks.previewPromptTemplate.mockResolvedValue(
      "撰写 批量内容工作台示例",
    );
    configMocks.savePromptTemplate.mockImplementation(
      async (template: { version: number }) => ({
        ...template,
        version: template.version + 1,
      }),
    );
    const user = userEvent.setup();
    await renderConfiguration(<PromptTemplatesPage />);

    const requiredVariables = await screen.findByRole("group", {
      name: "必需变量",
    });
    const optionalVariables = screen.getByRole("group", { name: "可选变量" });
    expect(
      within(requiredVariables).getByText("{{title}}"),
    ).toBeInTheDocument();
    expect(
      within(optionalVariables).getByText("{{keywords}}"),
    ).toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "本地预览" }));
    expect(configMocks.previewPromptTemplate).toHaveBeenCalledOnce();
    expect(configMocks.previewPromptTemplate).toHaveBeenCalledWith(
      {
        content: "撰写 {{title}}",
        id: "article",
        kind: "article",
        name: "正文",
        version: 1,
      },
      {
        article: "# 示例正文\n\n这里是用于本地预览的内容。",
        keywords: "内容运营、批量生成",
        requirements: "结构清晰，使用简体中文",
        summary: "这是一段用于本地预览的内容简介。",
        title: "批量内容工作台示例",
      },
    );
    expect(configMocks.verifyProviderConnection).not.toHaveBeenCalled();
    expect(
      await screen.findByText("撰写 批量内容工作台示例"),
    ).toBeInTheDocument();
    const articleContent = screen.getByLabelText(/Prompt 内容/u);
    await user.clear(articleContent);
    await user.click(articleContent);
    await user.paste("未保存 {{title}} 草稿");
    expect(
      screen.queryByText("撰写 批量内容工作台示例"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("本地预览已生成，没有调用 AI。"),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /简介 · v1/u }));
    await user.click(screen.getByRole("tab", { name: /正文 · v1/u }));
    expect(screen.getByLabelText(/Prompt 内容/u)).toHaveValue(
      "未保存 {{title}} 草稿",
    );
    await user.click(screen.getByRole("button", { name: "保存为新版本" }));
    expect(configMocks.savePromptTemplate).toHaveBeenCalledWith({
      content: "未保存 {{title}} 草稿",
      id: "article",
      kind: "article",
      name: "正文",
      version: 1,
    });
    await user.click(screen.getByRole("tab", { name: /简介 · v1/u }));
    await user.click(screen.getByRole("button", { name: "保存为新版本" }));
    await waitFor(() =>
      expect(configMocks.savePromptTemplate).toHaveBeenCalledWith(
        expect.objectContaining({ id: "summary", version: 1 }),
      ),
    );
    await user.click(screen.getByRole("tab", { name: /配图 · v1/u }));
    await user.click(screen.getByRole("button", { name: "保存为新版本" }));
    await waitFor(() =>
      expect(configMocks.savePromptTemplate).toHaveBeenCalledWith(
        expect.objectContaining({ id: "image", version: 1 }),
      ),
    );
  });

  /** Prompt 领域拒绝使用稳定错误翻译展示，并在继续编辑时清除旧错误。 */
  it("shows_and_clears_a_required_prompt_variable_error", async () => {
    configMocks.listPromptTemplates.mockResolvedValue([
      {
        allowedVariables: ["title", "keywords", "requirements"],
        content: "撰写 {{title}}",
        id: "article",
        kind: "article",
        name: "正文",
        requiredVariables: ["title"],
        version: 1,
      },
      {
        allowedVariables: ["title", "article"],
        content: "压缩 {{article}}",
        id: "summary",
        kind: "summary",
        name: "简介",
        requiredVariables: ["article"],
        version: 1,
      },
      {
        allowedVariables: ["title", "article", "summary"],
        content: "绘制 {{title}} 与 {{summary}}",
        id: "image",
        kind: "image",
        name: "配图",
        requiredVariables: ["title", "summary"],
        version: 1,
      },
    ]);
    configMocks.savePromptTemplate.mockRejectedValue({
      code: "PROMPT_VARIABLE_REQUIRED",
    });
    const user = userEvent.setup();
    await renderConfiguration(<PromptTemplatesPage />);

    const content = await screen.findByLabelText(/Prompt 内容/u);
    await user.clear(content);
    await user.type(content, "没有必需变量");
    await user.click(screen.getByRole("button", { name: "保存为新版本" }));
    const message = await screen.findByText(
      "Prompt 缺少当前阶段必需变量，请对照页面变量列表补齐。",
    );
    expect(message.closest('[role="alert"]')).not.toBeNull();

    await user.clear(content);
    await user.click(content);
    await user.paste("恢复 {{title}}");
    expect(
      screen.queryByText(
        "Prompt 缺少当前阶段必需变量，请对照页面变量列表补齐。",
      ),
    ).not.toBeInTheDocument();
  });

  /** 英文界面的本地预览变量也来自英文资源，不混入硬编码中文。 */
  it("uses_localized_prompt_preview_values", async () => {
    configMocks.listPromptTemplates.mockResolvedValue([
      {
        allowedVariables: ["title", "keywords", "requirements"],
        content: "Write {{title}}",
        id: "article",
        kind: "article",
        name: "Article",
        requiredVariables: ["title"],
        version: 1,
      },
      {
        allowedVariables: ["title", "article"],
        content: "Summarize {{article}}",
        id: "summary",
        kind: "summary",
        name: "Summary",
        requiredVariables: ["article"],
        version: 1,
      },
      {
        allowedVariables: ["title", "article", "summary"],
        content: "Illustrate {{title}} and {{summary}}",
        id: "image",
        kind: "image",
        name: "Image",
        requiredVariables: ["title", "summary"],
        version: 1,
      },
    ]);
    configMocks.previewPromptTemplate.mockResolvedValue(
      "Write Batch Content Workbench Example",
    );
    const user = userEvent.setup();
    await renderConfiguration(<PromptTemplatesPage />, "en-US");

    await user.click(
      await screen.findByRole("button", { name: "Local preview" }),
    );

    expect(configMocks.previewPromptTemplate).toHaveBeenCalledWith(
      expect.objectContaining({ id: "article" }),
      {
        article:
          "# Example article\n\nThis content is used only for local prompt preview.",
        keywords: "content operations, batch generation",
        requirements: "Use a clear structure and concise English",
        summary: "This is a sample article summary for local preview.",
        title: "Batch Content Workbench Example",
      },
    );
  });

  /** Prompt 后台刷新保留旧模板，并明确呈现进度、失败和重试入口。 */
  it("keeps_prompt_data_visible_during_a_failed_background_refresh", async () => {
    const templates = promptTemplateViews();
    configMocks.listPromptTemplates.mockResolvedValueOnce(templates);
    const queryClient = await renderConfiguration(<PromptTemplatesPage />);
    await screen.findByDisplayValue("撰写 {{title}}");

    let rejectRefresh!: (reason: unknown) => void;
    configMocks.listPromptTemplates.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          rejectRefresh = reject;
        }),
    );
    const refresh = queryClient.refetchQueries({
      queryKey: ["prompt-templates"],
    });
    expect(
      await screen.findByText("正在刷新 Prompt 模板…"),
    ).toBeInTheDocument();
    expect(screen.getByDisplayValue("撰写 {{title}}")).toBeInTheDocument();
    rejectRefresh(new Error("refresh failed"));
    await refresh;

    const refreshAlert = (
      await screen.findByText(
        "刷新 Prompt 模板失败；仍显示上次成功读取的本地数据。",
      )
    ).closest('[role="alert"]');
    expect(refreshAlert).not.toBeNull();
    expect(screen.getByDisplayValue("撰写 {{title}}")).toBeInTheDocument();
    configMocks.listPromptTemplates.mockResolvedValueOnce(templates);
    await userEvent
      .setup()
      .click(within(refreshAlert as HTMLElement).getByRole("button"));
    await waitFor(() => expect(refreshAlert).not.toBeInTheDocument());
  });
});
