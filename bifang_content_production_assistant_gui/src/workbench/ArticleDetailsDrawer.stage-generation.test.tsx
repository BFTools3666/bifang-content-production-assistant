import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createInstance } from "i18next";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import { ArticleDetailsDrawer } from "./ArticleDetailsDrawer";
import type {
  ArticleView,
  StageProgress,
  WorkflowStage,
} from "./workbenchTypes";

const { listConnectorsMock, loadEditorMock } = vi.hoisted(() => ({
  listConnectorsMock: vi.fn(),
  loadEditorMock: vi.fn(),
}));

vi.mock("./workbenchCommands", () => ({
  listConnectorConfigs: listConnectorsMock,
  loadArticleImageEditor: loadEditorMock,
}));

const readyProviders = {
  article: "ready",
  format: "ready",
  image: "ready",
  summary: "ready",
} as const;

/** 为详情专项测试提供不执行真实测量的观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}
  /** jsdom 不执行真实布局测量。 */
  observe(): void {}
  /** jsdom 允许停止观察但没有持久状态。 */
  unobserve(): void {}
}

/** 创建指定状态的最小阶段进度。 */
function stage(
  status: StageProgress["status"] = "notStarted",
  artifactVersion: number | null = null,
): StageProgress {
  return {
    activeAttemptId: null,
    artifactVersion,
    attempts: status === "notStarted" ? 0 : 1,
    failure: null,
    status,
  };
}

/** 创建可逐步补齐产物的详情文章。 */
function article(): ArticleView {
  return {
    article: {
      approvedHistory: [],
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
      attemptHistory: [],
      id: "article-detail-stage",
      keywords: [],
      packageHistory: [],
      packageVersion: 4,
      requirements: "",
      reviewStatus: "draft",
      rowVersion: 9,
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

/** 使用真实翻译和 Mantine 渲染一个详情阶段。 */
async function renderStage(
  view: ArticleView,
  activeTab: "article" | "summary" | "image" | "formats",
  onGenerateStage = vi.fn(),
  options: {
    actionError?: { message: string; stage: WorkflowStage; taskId: string };
    actionTarget?: { stage: WorkflowStage; taskId: string };
    providers?: typeof readyProviders;
  } = {},
): Promise<void> {
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
  render(
    <I18nextProvider i18n={instance}>
      <QueryClientProvider client={queryClient}>
        <MantineProvider env="test">
          <ArticleDetailsDrawer
            activeTab={activeTab}
            actionsLocked={false}
            article={view}
            imageEditorActionsLocked={false}
            onActiveTabChange={() => undefined}
            onApprove={() => Promise.resolve()}
            onClose={() => undefined}
            onGenerateStage={onGenerateStage}
            onRegenerate={() => Promise.resolve()}
            onRetry={() => Promise.resolve()}
            onReturnToDraft={() => Promise.resolve()}
            onSave={() => Promise.resolve()}
            onSaveImage={() => Promise.resolve()}
            onSaveInputs={() => Promise.resolve()}
            opened
            runHistory={[]}
            stageActionError={options.actionError ?? null}
            stageActionTarget={options.actionTarget ?? null}
            stageProviderState={options.providers ?? readyProviders}
          />
        </MantineProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

describe("article details single-stage generation", () => {
  beforeEach(() => {
    listConnectorsMock.mockReset();
    listConnectorsMock.mockResolvedValue([]);
    loadEditorMock.mockReset();
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
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: {
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      },
    });
  });

  afterEach(() => cleanup());

  /** 正文按钮携带当前行与包版本，只请求一个目标阶段。 */
  it("requests_only_the_current_article_stage_with_optimistic_versions", async () => {
    const onGenerateStage = vi.fn();
    await renderStage(article(), "article", onGenerateStage);

    const action = screen.getByRole("button", {
      name: "生成《企业知识库搭建指南》的正文",
    });
    expect(action).toHaveTextContent("生成正文");
    expect(action).toHaveAccessibleDescription(
      "只生成正文，完成后不会自动继续下一步。",
    );
    fireEvent.click(action);

    expect(onGenerateStage).toHaveBeenCalledWith(
      "article-detail-stage",
      "article",
      9,
      4,
    );
  });

  /** 简介在正文状态与产物未同时成功时禁用，并就近说明最早依赖。 */
  it("blocks_summary_until_the_article_output_is_current", async () => {
    await renderStage(article(), "summary");

    const action = screen.getByRole("button", {
      name: "生成《企业知识库搭建指南》的简介",
    });
    expect(action).toBeDisabled();
    expect(action).toHaveAccessibleDescription("先生成正文");
    expect(
      document.getElementById(
        "detail-stage-action-hint-article-detail-stage-summary",
      ),
    ).toHaveTextContent("先生成正文");
  });

  /** 格式阶段按固定 SOP 依次检查正文、简介和配图，而不是只看 Markdown。 */
  it("shows_the_earliest_missing_format_dependency", async () => {
    const view = article();
    view.article.artifacts.markdown = "# 正文";
    view.article.artifacts.summary = "简介";
    view.article.stages.article = stage("succeeded", 4);
    view.article.stages.summary = stage("succeeded", 4);
    await renderStage(view, "formats");

    const action = screen.getByRole("button", {
      name: "生成《企业知识库搭建指南》的格式",
    });
    expect(action).toBeDisabled();
    expect(action).toHaveAccessibleDescription("先生成配图");
    expect(screen.getByText("先生成配图")).toBeInTheDocument();
  });

  /** 目标阶段独占 loading，稳定错误仍显示在当前阶段按钮附近。 */
  it("shows_target_loading_and_local_stage_error", async () => {
    const view = article();
    view.article.stages.article = stage("failed");
    await renderStage(view, "article", vi.fn(), {
      actionError: {
        message: "正文阶段启动失败",
        stage: "article",
        taskId: view.article.id,
      },
      actionTarget: { stage: "article", taskId: view.article.id },
    });

    const action = screen.getByRole("button", {
      name: "重新生成《企业知识库搭建指南》的正文",
    });
    expect(action).toHaveAttribute("data-loading");
    expect(screen.getByRole("alert")).toHaveTextContent("正文阶段启动失败");
  });
});
