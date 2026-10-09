import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createInstance } from "i18next";
import { createStore, Provider } from "jotai";
import type { ReactElement } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import zhCN from "./i18n/zh-CN.json";
import type {
  BenchmarkWork,
  ResearchData,
  ResearchDraft,
} from "./researchTypes";

/** 只模拟宿主边界，保留页面真实交互与Query契约。 */
const mocks = vi.hoisted(() => ({
  getResearchLibrary: vi.fn(),
  newDeepseekTextProviderDraft: vi.fn(),
  saveProviderConfig: vi.fn(),
  listConnectorConfigs: vi.fn(),
  prepareDraftArticle: vi.fn(),
  getCollectionStatus: vi.fn(),
  startCollection: vi.fn(),
  listProviderConfigs: vi.fn(),
  generateContent: vi.fn(),
  analyzeStyle: vi.fn(),
  updateStyle: vi.fn(),
  updatePrompts: vi.fn(),
  updateDraft: vi.fn(),
  exportDraft: vi.fn(),
  navigate: vi.fn(),
}));
vi.mock("./researchCommands", () => ({
  ...mocks,
  researchError: () => "测试错误",
}));
vi.mock("../workbench/workbenchCommands", () => ({
  listProviderConfigs: mocks.listProviderConfigs,
  newDeepseekTextProviderDraft: mocks.newDeepseekTextProviderDraft,
  saveProviderConfig: mocks.saveProviderConfig,
  listConnectorConfigs: mocks.listConnectorConfigs,
}));
vi.mock("@tanstack/react-router", () => ({
  useNavigate: () => mocks.navigate,
}));
import { BenchmarkPage } from "./BenchmarkPage";
import { StyleLibraryPage } from "./StyleLibraryPage";
import { WriterPage } from "./WriterPage";
import { DraftEditor } from "./DraftEditor";
import { articleDetailsTabAtom } from "../workbench/workbenchState";
import {
  IS_XHS_EDITION,
  benchmarkSessionAtom,
  composerSessionAtom,
  platformOptions,
  writerStepAtom,
} from "./researchState";

/** 同一交互回归分别运行在固定的文章产品和小红书产品中。 */
const generationLabel = IS_XHS_EDITION ? "生成笔记文字" : "生成文章";
const topicLabel = IS_XHS_EDITION ? "这篇图文笔记的主题" : "这篇文章的主题";
const bodyLabel = IS_XHS_EDITION ? "笔记文字" : "文章正文";
const continueLabel = IS_XHS_EDITION ? "继续制作图文笔记" : "继续制作简介和配图";

/** 构造隔离的单元测试作品，不作为实际采集结果或产品演示数据。 */
function testWork(id: string, views: number | null): BenchmarkWork {
  return {
    id,
    platform: IS_XHS_EDITION ? "xiaohongshu" : "csdn",
    title: `作品${id}`,
    url: IS_XHS_EDITION ? `https://www.xiaohongshu.com/explore/${id}` : `https://blog.csdn.net/test/article/details/${id}`,
    author: "测试作者",
    authorUrl: IS_XHS_EDITION ? "https://www.xiaohongshu.com/user/profile/test" : "https://blog.csdn.net/test",
    publishedAt: "",
    excerpt: "",
    body: "正文材料".repeat(40),
    metrics: { views, likes: 0, favorites: null, comments: null, shares: null },
    collectedAtMs: 100,
  };
}

/** 在同一根store下渲染页面，验证跨路由选择状态。 */
async function renderPage(page: ReactElement, store = createStore()) {
  const i18n = createInstance();
  await i18n.init({ lng: "zh-CN", resources: { "zh-CN": { research: zhCN } } });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return {
    ...render(
      <I18nextProvider i18n={i18n}>
        <Provider store={store}>
          <QueryClientProvider client={client}>
            <MantineProvider env="test">{page}</MantineProvider>
          </QueryClientProvider>
        </Provider>
      </I18nextProvider>,
    ),
    store,
    client,
  };
}

beforeEach(() => {
  for (const mock of Object.values(mocks)) mock.mockReset();
  const data: ResearchData = {
    works: [testWork("123", null), testWork("124", 1200), testWork("125", 20)],
    styles: [],
    drafts: [],
    prompts: {
      seoRewrite: "原创改写",
      xhsRewrite: "原创笔记",
      styleAnalysis: "分析风格",
    },
  };
  mocks.getResearchLibrary.mockResolvedValue(data);
  mocks.listConnectorConfigs.mockResolvedValue([]);
  mocks.getCollectionStatus.mockResolvedValue({
    pageUrl: "",
    message: "",
    collectedCount: 0,
    updatedAtMs: 0,
  });
  mocks.listProviderConfigs.mockResolvedValue([
    {
      id: "text-provider",
      name: "已保存文本服务",
      enabled: true,
      kind: "openAiCompatible",
      hasApiKey: true,
      articleModel: "test-model",
      defaultForArticle: true,
    },
  ]);
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn().mockImplementation(() => ({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { addEventListener: vi.fn(), removeEventListener: vi.fn() },
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("研究页面契约", () => {
  it("offers_only_the_platforms_of_the_fixed_product", () => {
    expect(platformOptions().map(({ value }) => value)).toEqual(
      IS_XHS_EDITION ? ["xiaohongshu"] : ["csdn", "baijiahao"],
    );
  });
  it("行和单元格不代理勾选，最低指标排除缺失值", async () => {
    await renderPage(<BenchmarkPage />);
    const checkbox = await screen.findByRole("checkbox", {
      name: "选择 作品124",
    });
    fireEvent.click(checkbox.closest("tr")!);
    expect(checkbox).not.toBeChecked();
    fireEvent.click(checkbox);
    expect(checkbox).toBeChecked();
    fireEvent.change(screen.getByLabelText("最低阅读／播放"), {
      target: { value: "100" },
    });
    await waitFor(() =>
      expect(
        screen.queryByRole("checkbox", { name: "选择 作品123" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      screen.queryByRole("checkbox", { name: "选择 作品125" }),
    ).not.toBeInTheDocument();
    expect(checkbox).toBeInTheDocument();
  });

  it("同一根store跨页面保留选择，新进程store恢复默认", async () => {
    const first = await renderPage(<BenchmarkPage />);
    fireEvent.click(
      await screen.findByRole("checkbox", { name: "选择 作品124" }),
    );
    first.unmount();
    first.client.clear();
    const second = await renderPage(<BenchmarkPage />, first.store);
    expect(
      await screen.findByRole("checkbox", { name: "选择 作品124" }),
    ).toBeChecked();
    second.unmount();
    second.client.clear();
    await renderPage(<BenchmarkPage />);
    expect(
      await screen.findByRole("checkbox", { name: "选择 作品124" }),
    ).not.toBeChecked();
  });

  it("使用原有扁平ProviderConfigView支持生成与三篇风格分析", async () => {
    const store = createStore();
    store.set(composerSessionAtom, {
      ...store.get(composerSessionAtom),
      keyword: "Python 自动化",
    });
    const writer = await renderPage(<WriterPage />, store);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: `下一步：${generationLabel}` }),
      ).toBeEnabled(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: `下一步：${generationLabel}` }),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: generationLabel }),
      ).toBeEnabled(),
    );
    writer.unmount();
    writer.client.clear();
    store.set(benchmarkSessionAtom, {
      ...store.get(benchmarkSessionAtom),
      selected: ["123", "124", "125"],
    });
    await renderPage(<StyleLibraryPage />, store);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "分析并保存风格" }),
      ).toBeEnabled(),
    );
  });
  it("blocks_writing_without_a_key_and_offers_setup", async () => {
    mocks.listProviderConfigs.mockResolvedValue([
      {
        id: "no-key",
        name: "未接入",
        enabled: true,
        kind: "openAiCompatible",
        hasApiKey: false,
        articleModel: "test-model",
      },
    ]);
    await renderPage(<WriterPage />);
    await screen.findByText(`先接入 AI，才能${generationLabel}`);
    fireEvent.change(screen.getByLabelText(new RegExp(topicLabel)), {
      target: { value: "运营主题" },
    });
    expect(
      screen.getByRole("button", { name: `下一步：${generationLabel}` }),
    ).toBeDisabled();
    expect(mocks.generateContent).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "接入 AI" }));
    expect(await screen.findByLabelText("你的 API Key")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "保存并继续生成" }),
    ).toBeDisabled();
  });

  it("does_not_treat_a_failed_config_read_as_an_empty_account", async () => {
    mocks.listProviderConfigs.mockRejectedValue(new Error("offline"));
    await renderPage(<WriterPage />);
    await screen.findByText("AI 设置读取失败，请重试。原有配置不会被清空。");
    expect(
      screen.queryByText(`先接入 AI，才能${generationLabel}`),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: `下一步：${generationLabel}` }),
    ).toBeDisabled();
    expect(mocks.generateContent).not.toHaveBeenCalled();
  });

  it("retains_material_and_does_not_retry_a_failed_paid_request", async () => {
    const store = createStore();
    store.set(composerSessionAtom, {
      ...store.get(composerSessionAtom),
      keyword: "运营主题",
      userMaterial: "用户的真实材料",
    });
    mocks.generateContent.mockRejectedValue(new Error("quota"));
    await renderPage(<WriterPage />, store);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: `下一步：${generationLabel}` }),
      ).toBeEnabled(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: `下一步：${generationLabel}` }),
    );
    fireEvent.click(screen.getByRole("button", { name: generationLabel }));
    await screen.findByRole("alert");
    expect(mocks.generateContent).toHaveBeenCalledTimes(1);
    expect(store.get(composerSessionAtom).userMaterial).toBe("用户的真实材料");
    expect(store.get(writerStepAtom)).toBe(1);
  });

  it("retains_preparation_and_step_when_returning_from_settings", async () => {
    const first = await renderPage(<WriterPage />);
    fireEvent.change(screen.getByLabelText(new RegExp(topicLabel)), {
      target: { value: "运营主题" },
    });
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: `下一步：${generationLabel}` }),
      ).toBeEnabled(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: `下一步：${generationLabel}` }),
    );
    first.unmount();
    first.client.clear();
    const second = await renderPage(<WriterPage />, first.store);
    expect(await screen.findByText("运营主题")).toBeInTheDocument();
    expect(first.store.get(writerStepAtom)).toBe(1);
    second.unmount();
    second.client.clear();
    await renderPage(<WriterPage />);
    expect(screen.getByLabelText(new RegExp(topicLabel))).toHaveValue("");
  });

  it("retains_unsaved_edits_and_blocks_export_until_saved", async () => {
    const draft: ResearchDraft = {
      id: "draft-test",
      platform: IS_XHS_EDITION ? "xiaohongshu" : "csdn",
      keyword: "主题",
      title: "原标题",
      titles: ["原标题", "候选标题"],
      body: "已保存正文",
      tags: [],
      sourceIds: [],
      styleId: null,
      prompt: "原指令",
      providerId: "text-provider",
      model: "test-model",
      createdAtMs: 100,
      tokenUsage: null,
    };
    const first = await renderPage(<DraftEditor draft={draft} mode="review" />);
    fireEvent.change(screen.getByLabelText(bodyLabel), {
      target: { value: "用户尚未保存的修改" },
    });
    first.unmount();
    first.client.clear();
    const second = await renderPage(
      <DraftEditor draft={draft} mode="export" />,
      first.store,
    );
    expect(screen.getByLabelText(bodyLabel)).toHaveValue("用户尚未保存的修改");
    expect(
      screen.getByRole("combobox", { name: "选择二次标题" }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "HTML" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: continueLabel }),
    ).toBeDisabled();
    expect(mocks.exportDraft).not.toHaveBeenCalled();
    expect(mocks.prepareDraftArticle).not.toHaveBeenCalled();
    second.unmount();
    second.client.clear();
  });

  it("guides a Xiaohongshu note through summary before image production", async () => {
    const note: ResearchDraft = {
      id: "note-test",
      platform: "xiaohongshu",
      keyword: "图文主题",
      title: "笔记标题",
      titles: ["笔记标题"],
      body: "已保存的笔记文字",
      tags: ["图文"],
      sourceIds: [],
      styleId: null,
      prompt: "原创笔记指令",
      providerId: "text-provider",
      model: "test-model",
      createdAtMs: 100,
      tokenUsage: null,
    };
    mocks.prepareDraftArticle.mockResolvedValue("article-note");
    const page = await renderPage(<DraftEditor draft={note} />);
    expect(screen.getByLabelText("笔记文字")).toHaveValue(note.body);
    expect(
      screen.getByText(
        "文字已保存。要完成图文笔记，请继续制作简介和配图；现在导出的文件只有文字。",
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "继续制作图文笔记" }));
    await waitFor(() =>
      expect(mocks.prepareDraftArticle).toHaveBeenCalledWith(note.id),
    );
    await waitFor(() =>
      expect(page.store.get(articleDetailsTabAtom)).toBe("summary"),
    );
  });

  it("saves_a_key_without_automatically_calling_the_provider", async () => {
    mocks.listProviderConfigs.mockResolvedValue([]);
    mocks.newDeepseekTextProviderDraft.mockResolvedValue({
      id: "quick-text",
      name: "DeepSeek",
      kind: "openAiCompatible",
      enabled: true,
      baseUrl: "https://api.deepseek.com",
      textModel: "deepseek-flash",
      articleModel: "deepseek-flash",
      summaryModel: "deepseek-flash",
      imageModel: "",
      defaultForArticle: true,
      defaultForSummary: true,
      defaultForImage: false,
    });
    mocks.saveProviderConfig.mockResolvedValue({
      providers: [
        {
          id: "quick-text",
          name: "DeepSeek",
          kind: "openAiCompatible",
          enabled: true,
          hasApiKey: true,
          articleModel: "deepseek-flash",
          defaultForArticle: true,
        },
      ],
    });
    const page = await renderPage(<WriterPage />);
    fireEvent.click(await screen.findByRole("button", { name: "接入 AI" }));
    fireEvent.change(await screen.findByLabelText("你的 API Key"), {
      target: { value: "unit-test-only-key" },
    });
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "保存并继续生成" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "保存并继续生成" }));
    await waitFor(() =>
      expect(screen.queryByLabelText("你的 API Key")).not.toBeInTheDocument(),
    );
    expect(mocks.saveProviderConfig).toHaveBeenCalledTimes(1);
    expect(mocks.saveProviderConfig.mock.calls[0]?.[0].defaultForImage).toBe(
      false,
    );
    expect(
      JSON.stringify(page.client.getQueryData(["provider-configs"])),
    ).not.toContain("unit-test-only-key");
    expect(mocks.generateContent).not.toHaveBeenCalled();
  });
});
