import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { createInstance } from "i18next";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const commandMocks = vi.hoisted(() => ({
  loadArticleThumbnail: vi.fn(),
}));

vi.mock("./workbenchCommands", () => commandMocks);

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import { ArticleGrid, type ArticleGridEmptyState } from "./ArticleGrid";
import type { ArticleView, StageProgress } from "./workbenchTypes";

const readyProviders = {
  article: "ready",
  format: "ready",
  image: "ready",
  summary: "ready",
} as const;

/** 为 Grid 测试提供 Mantine 所需的空布局观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}
  /** jsdom 不执行真实布局测量。 */
  observe(): void {}
  /** jsdom 允许停止观察但没有持久状态。 */
  unobserve(): void {}
}

/** 创建一个处于指定阶段状态的最小进度。 */
function stage(status: StageProgress["status"] = "notStarted"): StageProgress {
  return {
    activeAttemptId: null,
    artifactVersion: null,
    attempts: 0,
    failure: null,
    status,
  };
}

/** 创建一条满足真实 IPC 结构的草稿文章。 */
function article(): ArticleView {
  return {
    article: {
      approvedHistory: [],
      attemptHistory: [],
      artifacts: {
        articleLineage: null,
        formatLineage: null,
        html: null,
        imageEditLineage: null,
        imageEditRevision: 0,
        imageLineage: null,
        markdown: null,
        summary: null,
        summaryLineage: null,
      },
      createdAtMs: 1_789_123_456_000,
      id: "article-1",
      keywords: [],
      packageHistory: [],
      packageVersion: 1,
      requirements: "",
      reviewStatus: "draft",
      rowVersion: 1,
      stages: {
        article: stage(),
        format: stage(),
        image: stage(),
        summary: stage(),
      },
      title: "企业知识库搭建指南",
      updatedAtMs: 1_789_123_456_000,
    },
    hasImage: false,
    overallStatus: "draft",
  };
}

/** 创建带有正文、简介和受管配图的已生成行。 */
function articleWithPreviews(): ArticleView {
  const view = article();
  view.article.artifacts.markdown =
    "# 企业知识库搭建指南\n\n这是用于 Grid 缩略展示的正文内容。";
  view.article.artifacts.summary = "用一句话介绍企业知识库的搭建方法。";
  view.article.artifacts.imageLineage = {
    attemptId: "attempt-image-1",
    model: "image-v1",
    packageVersion: 1,
    providerId: "provider-image",
    providerRevision: 1,
    sourceArticleVersion: 1,
    sourceSummaryVersion: 1,
    templateId: "prompt-image",
    templateVersion: 1,
  };
  view.article.stages.article = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 1,
    failure: null,
    status: "succeeded",
  };
  view.article.stages.summary = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 1,
    failure: null,
    status: "succeeded",
  };
  view.article.stages.image = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 1,
    failure: null,
    status: "succeeded",
  };
  view.hasImage = true;
  view.overallStatus = "draft";
  return view;
}

describe("article Grid interactions", () => {
  beforeEach(() => {
    commandMocks.loadArticleThumbnail.mockReset();
    commandMocks.loadArticleThumbnail.mockResolvedValue(
      "data:image/png;base64,aGVsbG8=",
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
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      value: undefined,
    });
  });
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  /** 工作区真空、筛选无匹配与页码越界分别提供匹配当前恢复路径的空态。 */
  it("renders_workspace_filtered_and_page_empty_states_separately", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    const onClearFilters = vi.fn();
    const onOpenAdd = vi.fn();
    const emptyGrid = (emptyState: ArticleGridEmptyState) => (
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[]}
            emptyState={emptyState}
            loading={false}
            onClearFilters={onClearFilters}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={onOpenAdd}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>
    );
    const rendered = render(emptyGrid("workspace"));

    expect(screen.getByText(zhCN.workbench.empty.title)).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", {
        name: zhCN.workbench.actions.add_titles,
      }),
    );
    expect(onOpenAdd).toHaveBeenCalledOnce();
    expect(onClearFilters).not.toHaveBeenCalled();

    rendered.rerender(emptyGrid("filtered"));
    expect(
      screen.getByText(zhCN.workbench.empty.filtered_title),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: zhCN.workbench.actions.add_titles,
      }),
    ).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", {
        name: zhCN.workbench.empty.clear_filters,
      }),
    );
    expect(onClearFilters).toHaveBeenCalledOnce();
    expect(onOpenAdd).toHaveBeenCalledOnce();

    rendered.rerender(emptyGrid("page"));
    expect(
      screen.getByText(zhCN.workbench.empty.page_title),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: zhCN.workbench.empty.clear_filters,
      }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: zhCN.workbench.actions.add_titles,
      }),
    ).not.toBeInTheDocument();
  });

  /** 只有排队/运行阶段显示转圈，运行计时逐秒变化，终态固定真实耗时与 token。 */
  it("shows_generation_motion_live_time_and_terminal_metrics_from_attempts", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    vi.useFakeTimers();
    vi.setSystemTime(5_000);
    const view = article();
    view.article.stages.article = {
      activeAttemptId: "attempt-article",
      artifactVersion: null,
      attempts: 1,
      failure: null,
      status: "running",
    };
    view.article.stages.summary.status = "queued";
    view.article.stages.image.status = "succeeded";
    view.article.stages.format.status = "succeeded";
    view.article.attemptHistory = [
      {
        failure: null,
        finishedAtMs: null,
        id: "attempt-article",
        lineage: null,
        packageVersion: 1,
        stage: "article",
        startedAtMs: 2_000,
        status: "running",
        tokenUsage: null,
      },
      {
        failure: null,
        finishedAtMs: 900,
        id: "attempt-image",
        lineage: null,
        packageVersion: 1,
        stage: "image",
        startedAtMs: 100,
        status: "succeeded",
        tokenUsage: null,
      },
      {
        failure: null,
        finishedAtMs: 1_200,
        id: "attempt-format",
        lineage: null,
        packageVersion: 1,
        stage: "format",
        startedAtMs: 1_000,
        status: "succeeded",
        tokenUsage: {
          inputTokens: 0,
          outputTokens: 0,
          totalTokens: 0,
        },
      },
    ];
    const renderGrid = (current: ArticleView) => (
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[current]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={{ taskId: "article-1", stage: "image" }}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>
    );
    const rendered = render(renderGrid(view));

    expect(screen.getByLabelText("正文正在生成")).toBeInTheDocument();
    expect(screen.getByLabelText("简介等待生成")).toBeInTheDocument();
    expect(screen.getAllByRole("status")).toHaveLength(2);
    expect(screen.getByText("已用 3 秒")).toBeInTheDocument();
    expect(screen.getByText("Token 未提供")).toBeInTheDocument();
    expect(screen.getByText("0 Token · 本地处理")).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "重新生成《企业知识库搭建指南》的配图",
      }),
    ).toHaveAttribute("aria-busy", "false");

    act(() => vi.advanceTimersByTime(2_000));
    expect(screen.getByText("已用 5 秒")).toBeInTheDocument();

    view.article.stages.article.status = "succeeded";
    view.article.stages.article.activeAttemptId = null;
    view.article.stages.summary.status = "notStarted";
    view.article.stages.image.status = "notStarted";
    const articleAttempt = view.article.attemptHistory[0];
    if (!articleAttempt) throw new Error("缺少正文尝试夹具");
    articleAttempt.status = "succeeded";
    articleAttempt.finishedAtMs = 6_500;
    articleAttempt.tokenUsage = {
      inputTokens: 40,
      outputTokens: 83,
      totalTokens: 123,
    };
    rendered.rerender(renderGrid(view));

    expect(screen.queryByLabelText("正文正在生成")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("简介等待生成")).not.toBeInTheDocument();
    expect(screen.queryAllByRole("status")).toHaveLength(0);
    expect(
      screen.queryByTestId("stage-metrics-article-1-image"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("耗时 4.5 秒")).toBeInTheDocument();
    expect(screen.getByText("123 Token")).toBeInTheDocument();
  });

  /** Checkbox、表格父区域与详情按钮各自只触发自己的语义动作。 */
  it("keeps_checkbox_row_and_details_actions_separate", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    const onInspect = vi.fn();
    const onGenerateStage = vi.fn();
    const onSelectionChange = vi.fn();
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[article()]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={onGenerateStage}
            onInspect={onInspect}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={onSelectionChange}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: /企业知识库/ }));
    expect(onSelectionChange).toHaveBeenCalledWith("article-1", true);
    expect(onInspect).not.toHaveBeenCalled();

    const row = screen.getByTestId("article-row-article-1");
    fireEvent.click(row);
    expect(onSelectionChange).toHaveBeenCalledOnce();
    expect(onInspect).not.toHaveBeenCalled();
    expect(onGenerateStage).not.toHaveBeenCalled();

    const titleCell = within(row).getAllByRole("cell")[1];
    if (!titleCell) throw new Error("缺少文章标题单元格");
    fireEvent.click(titleCell);
    expect(onSelectionChange).toHaveBeenCalledOnce();
    expect(onInspect).not.toHaveBeenCalled();
    expect(onGenerateStage).not.toHaveBeenCalled();

    fireEvent.click(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的正文",
      }),
    );
    expect(onGenerateStage).toHaveBeenCalledWith("article-1", "article", 1, 1);
    expect(onInspect).not.toHaveBeenCalled();
    expect(onSelectionChange).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByRole("button", { name: /查看.*详情/ }));
    expect(onInspect).toHaveBeenCalledWith("article-1");
    expect(onSelectionChange).toHaveBeenCalledOnce();
  });

  /** 六列 Grid 在固定布局中保留状态、时间和详情动作，不再创建横向滚动容器。 */
  it("fits_complete_article_information_into_a_six_column_grid", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[article()]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    const container = screen.getByTestId("article-grid-container");
    const table = screen.getByTestId("article-grid-table");
    expect(container).toContainElement(table);
    expect(getComputedStyle(container).overflowX).not.toBe("auto");
    expect(table.style.getPropertyValue("--table-layout")).toBe("fixed");
    expect(
      screen.queryByTestId("article-grid-scroll-container"),
    ).not.toBeInTheDocument();
    expect(
      screen.getAllByRole("columnheader").map((header) => header.textContent),
    ).toEqual(["选择", "文章信息", "正文", "简介", "配图", "格式"]);

    const row = screen.getByTestId("article-row-article-1");
    const cells = within(row).getAllByRole("cell");
    expect(cells).toHaveLength(6);
    const informationCell = cells[1];
    if (!informationCell) throw new Error("缺少文章信息单元格");
    expect(
      within(informationCell).getByText("企业知识库搭建指南"),
    ).toBeInTheDocument();
    expect(within(informationCell).getByText("文章版本 1")).toBeInTheDocument();
    expect(within(informationCell).getByText("草稿")).toBeInTheDocument();
    expect(within(informationCell).getByText("创建时间")).toBeInTheDocument();
    expect(within(informationCell).getByText("更新时间")).toBeInTheDocument();
    expect(informationCell.querySelectorAll("time")).toHaveLength(2);
    expect(
      within(informationCell).getByRole("button", { name: /查看.*详情/ }),
    ).toBeInTheDocument();
  });

  /** 依赖与目标 AI 未就绪时只禁用对应语义按钮，不把状态徽标变成动作。 */
  it("explains_stage_dependencies_and_provider_availability", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[article()]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={{ ...readyProviders, article: "missing" }}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    const articleAction = screen.getByRole("button", {
      name: "生成《企业知识库搭建指南》的正文",
    });
    expect(articleAction).toBeDisabled();
    expect(articleAction).toHaveTextContent("设置正文 AI");
    expect(articleAction).toHaveAccessibleDescription("设置正文 AI");
    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的简介",
      }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的简介",
      }),
    ).toHaveTextContent("先生成正文");
  });

  /** 简介依赖正文，配图还需简介，格式需前三阶段，镜像 core 的真实依赖矩阵。 */
  it("enables_only_summary_after_article_until_later_dependencies_finish", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    const view = article();
    view.article.artifacts.markdown = "# 企业知识库搭建指南";
    view.article.stages.article = {
      activeAttemptId: null,
      artifactVersion: 1,
      attempts: 1,
      failure: null,
      status: "succeeded",
    };
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[view]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的简介",
      }),
    ).toBeEnabled();
    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的格式",
      }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的配图",
      }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的配图",
      }),
    ).toHaveAccessibleDescription("先生成简介");
    expect(
      screen.getByRole("button", {
        name: "生成《企业知识库搭建指南》的格式",
      }),
    ).toHaveAccessibleDescription("先生成简介");
  });

  /** 正文、简介和配图预览只打开各自页签，不代理生成、勾选或通用详情动作。 */
  it("shows_artifact_previews_with_isolated_inspection_actions", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    const onGenerateStage = vi.fn();
    const onInspect = vi.fn();
    const onInspectArtifact = vi.fn();
    const onSelectionChange = vi.fn();
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <StrictMode>
            <ArticleGrid
              actionLocked={false}
              articles={[articleWithPreviews()]}
              emptyState="workspace"
              loading={false}
              onClearFilters={vi.fn()}
              onGenerateStage={onGenerateStage}
              onInspect={onInspect}
              onInspectArtifact={onInspectArtifact}
              onOpenAdd={vi.fn()}
              onSelectionChange={onSelectionChange}
              selectedIds={new Set()}
              stageActionError={null}
              stageActionTarget={null}
              stageProviderState={readyProviders}
            />
          </StrictMode>
        </MantineProvider>
      </I18nextProvider>,
    );

    const articlePreview = screen.getByRole("button", {
      name: /打开《企业知识库搭建指南》的正文进行编辑/,
    });
    const summaryPreview = screen.getByRole("button", {
      name: /打开《企业知识库搭建指南》的简介进行编辑/,
    });
    expect(articlePreview).toHaveTextContent("Grid 缩略展示的正文内容");
    expect(summaryPreview).toHaveTextContent("企业知识库的搭建方法");

    fireEvent.click(articlePreview);
    fireEvent.click(summaryPreview);
    const imagePreview = await screen.findByRole("button", {
      name: /查看《企业知识库搭建指南》的配图/,
    });
    fireEvent.click(imagePreview);

    expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledOnce();
    expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledWith("article-1");
    expect(onInspectArtifact.mock.calls).toEqual([
      ["article-1", "article"],
      ["article-1", "summary"],
      ["article-1", "image"],
    ]);
    expect(onGenerateStage).not.toHaveBeenCalled();
    expect(onInspect).not.toHaveBeenCalled();
    expect(onSelectionChange).not.toHaveBeenCalled();

    fireEvent.click(
      screen.getByRole("button", {
        name: "重新生成《企业知识库搭建指南》的正文",
      }),
    );
    expect(onGenerateStage).toHaveBeenCalledWith("article-1", "article", 1, 1);
    expect(onInspectArtifact).toHaveBeenCalledTimes(3);
  });

  /** 无配图的行绝不调用缩略图 IPC。 */
  it("does_not_load_a_thumbnail_without_an_image", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[article()]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    await Promise.resolve();
    expect(commandMocks.loadArticleThumbnail).not.toHaveBeenCalled();
  });

  /** 配图只在进入视口时读取，离开后清除 data URL 以限制多行内存。 */
  it("loads_and_releases_thumbnails_at_the_viewport_boundary", async () => {
    /** 由测试主动推送可见性变化，避免依赖 jsdom 不存在的真实布局。 */
    class ControlledIntersectionObserver {
      static callback: IntersectionObserverCallback = () => undefined;

      /** 保存当前行注册的观察回调，供测试精确模拟进入和离开视口。 */
      constructor(callback: IntersectionObserverCallback) {
        ControlledIntersectionObserver.callback = callback;
      }
      /** 测试替身不持有真实浏览器观察资源。 */
      disconnect(): void {}
      /** 测试通过静态回调控制状态，无需保存被观察节点。 */
      observe(): void {}
    }
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      value: ControlledIntersectionObserver,
    });
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[articleWithPreviews()]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    expect(commandMocks.loadArticleThumbnail).not.toHaveBeenCalled();
    ControlledIntersectionObserver.callback(
      [{ isIntersecting: true } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    );
    await waitFor(() =>
      expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledOnce(),
    );
    expect(
      await screen.findByRole("button", { name: /查看.*配图/ }),
    ).toBeEnabled();

    ControlledIntersectionObserver.callback(
      [{ isIntersecting: false } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: /查看.*配图/ }),
      ).not.toBeInTheDocument(),
    );
  });

  /** AI 尝试或人工编辑修订变化时刷新同一行缩略图，避免继续展示旧产物。 */
  it("reloads_the_thumbnail_when_the_image_identity_changes", async () => {
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    const initial = articleWithPreviews();
    const grid = (view: ArticleView) => (
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[view]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={vi.fn()}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>
    );
    const rendered = render(grid(initial));
    await waitFor(() =>
      expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledOnce(),
    );

    const refreshed = structuredClone(initial);
    if (refreshed.article.artifacts.imageLineage) {
      refreshed.article.artifacts.imageLineage.attemptId = "attempt-image-2";
    }
    rendered.rerender(grid(refreshed));

    await waitFor(() =>
      expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledTimes(2),
    );
    expect(commandMocks.loadArticleThumbnail).toHaveBeenNthCalledWith(
      2,
      "article-1",
    );

    const manuallyEdited = structuredClone(refreshed);
    manuallyEdited.article.artifacts.imageEditRevision = 1;
    rendered.rerender(grid(manuallyEdited));
    await waitFor(() =>
      expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledTimes(3),
    );
    expect(commandMocks.loadArticleThumbnail).toHaveBeenNthCalledWith(
      3,
      "article-1",
    );
  });

  /** 缩略图读取失败只在当前格提供重试，不发起任何 AI 生成。 */
  it("retries_a_failed_thumbnail_without_triggering_generation", async () => {
    commandMocks.loadArticleThumbnail
      .mockRejectedValueOnce(new Error("read failed"))
      .mockResolvedValueOnce("data:image/png;base64,c2Vjb25k");
    const instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    const onGenerateStage = vi.fn();
    render(
      <I18nextProvider i18n={instance}>
        <MantineProvider env="test">
          <ArticleGrid
            actionLocked={false}
            articles={[articleWithPreviews()]}
            emptyState="workspace"
            loading={false}
            onClearFilters={vi.fn()}
            onGenerateStage={onGenerateStage}
            onInspect={vi.fn()}
            onInspectArtifact={vi.fn()}
            onOpenAdd={vi.fn()}
            onSelectionChange={vi.fn()}
            selectedIds={new Set()}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={readyProviders}
          />
        </MantineProvider>
      </I18nextProvider>,
    );

    fireEvent.click(
      await screen.findByRole("button", {
        name: /重新读取《企业知识库搭建指南》的配图缩略图/,
      }),
    );
    await waitFor(() =>
      expect(commandMocks.loadArticleThumbnail).toHaveBeenCalledTimes(2),
    );
    expect(
      await screen.findByRole("button", { name: /查看.*配图/ }),
    ).toBeEnabled();
    expect(onGenerateStage).not.toHaveBeenCalled();
  });
});
