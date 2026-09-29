import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ArticleView, RunRecord } from "./workbenchTypes";
import {
  TestResizeObserver,
  draftArticle,
  failedArticle,
  pendingArticle,
  renderWorkbench as renderWorkbenchNode,
} from "./ArticleWorkbenchPageTestSupport";

const commandMocks = vi.hoisted(() => ({
  addArticleTitles: vi.fn(),
  approveArticle: vi.fn(),
  cancelActiveBatch: vi.fn(),
  generateArticleStage: vi.fn(),
  getWorkbench: vi.fn(),
  listConnectorConfigs: vi.fn(),
  listProviderConfigs: vi.fn(),
  loadArticleImage: vi.fn(),
  loadArticleImageEditor: vi.fn(),
  loadArticleThumbnail: vi.fn(),
  pauseActiveBatch: vi.fn(),
  previewSelectedBatch: vi.fn(),
  regenerateArticle: vi.fn(),
  saveArticleImageEdit: vi.fn(),
  resumeActiveBatch: vi.fn(),
  retryArticle: vi.fn(),
  retrySelectedArticles: vi.fn(),
  returnArticleToDraft: vi.fn(),
  startSelectedBatch: vi.fn(),
  syncArticleToConnector: vi.fn(),
  updateArticleContent: vi.fn(),
  updateArticleInputs: vi.fn(),
}));

vi.mock("@tanstack/react-router", () => ({ useNavigate: () => vi.fn() }));
vi.mock("./workbenchCommands", () => ({
  ...commandMocks,
}));

vi.mock("./workbenchErrors", () => ({
  localizedCommandError: (_reason: unknown, t: (key: string) => string) =>
    t("common.failed"),
  localizedFailure: (failure: { code: string }, t: (key: string) => string) =>
    t(`errors.${failure.code}`),
}));

import { ArticleWorkbenchPage } from "./ArticleWorkbenchPage";

const clipboardWrite = vi.fn<(value: string) => Promise<void>>();

/** 在页面测试入口保留命令模拟，再渲染真实工作台。 */
async function renderWorkbench(): Promise<void> {
  await renderWorkbenchNode(<ArticleWorkbenchPage />);
}

describe("batch content workbench", () => {
  beforeEach(() => {
    for (const mock of Object.values(commandMocks)) mock.mockReset();
    commandMocks.listConnectorConfigs.mockResolvedValue([]);
    clipboardWrite.mockReset();
    clipboardWrite.mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: clipboardWrite },
    });
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
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [
        draftArticle("article-a", "企业知识库搭建指南"),
        draftArticle("article-b", "内容营销趋势"),
      ],
      runHistory: [],
    });
    commandMocks.listProviderConfigs.mockResolvedValue([
      {
        articleModel: "fixture-article-v1",
        articleParameters: {
          maxOutputTokens: null,
          temperatureMilli: null,
          timeoutMs: 60_000,
        },
        baseUrl: "",
        defaultForImage: true,
        defaultForArticle: true,
        defaultForSummary: true,
        enabled: true,
        hasApiKey: true,
        id: "provider-e2e-fixture",
        imageModel: "fixture-image-v1",
        imageParameters: {
          size: "1536x1024",
          timeoutMs: 90_000,
          watermark: false,
        },
        isTestFixture: true,
        kind: "fixture",
        name: "E2E 测试夹具（非真实 AI）",
        revision: 1,
        summaryModel: "fixture-summary-v1",
        summaryParameters: {
          maxOutputTokens: null,
          temperatureMilli: null,
          timeoutMs: 60_000,
        },
        textModel: "fixture-text-v1",
      },
    ]);
    commandMocks.loadArticleImage.mockResolvedValue(
      "data:image/png;base64,aGVsbG8=",
    );
    commandMocks.loadArticleImageEditor.mockResolvedValue({
      canSave: true,
      currentSpec: {
        crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
        flipHorizontal: false,
        flipVertical: false,
        rotationQuarterTurns: 0,
      },
      editRevision: 0,
      imageAttemptId: "attempt-image-2",
      imageDataUrl: "data:image/png;base64,aGVsbG8=",
      packageVersion: 1,
      rowVersion: 1,
      saveBlockedReason: null,
      sourceHeight: 675,
      sourceWidth: 1200,
    });
    commandMocks.loadArticleThumbnail.mockResolvedValue(
      "data:image/png;base64,aGVsbG8=",
    );
  });
  afterEach(() => cleanup());

  /** 活动批次按钮调用各自 IPC，并在权威查询刷新后显示暂停、恢复和取消结果。 */
  it("pauses_resumes_and_cancels_the_active_batch", async () => {
    let activeStatus: RunRecord["status"] | null = "running";
    const activeRun = (status: RunRecord["status"]): RunRecord => ({
      completedCount: 1,
      failedCount: 0,
      finishedAtMs: null,
      id: "batch-active",
      isTestFixture: true,
      startedAtMs: 1_789_123_456_000,
      status,
      targetStage: null,
      taskIds: ["article-a", "article-b"],
    });
    commandMocks.getWorkbench.mockImplementation(() =>
      Promise.resolve({
        activeRun: activeStatus ? activeRun(activeStatus) : null,
        articles: [draftArticle("article-a", "企业知识库搭建指南")],
        runHistory: [],
      }),
    );
    commandMocks.pauseActiveBatch.mockImplementation(() => {
      activeStatus = "paused";
      return Promise.resolve("batch-active");
    });
    commandMocks.resumeActiveBatch.mockImplementation(() => {
      activeStatus = "running";
      return Promise.resolve("batch-active");
    });
    commandMocks.cancelActiveBatch.mockImplementation(() => {
      activeStatus = null;
      return Promise.resolve("batch-active");
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(await screen.findByRole("button", { name: "暂停批次" }));
    expect(commandMocks.pauseActiveBatch).toHaveBeenCalledTimes(1);
    const resumeButton = await screen.findByRole("button", {
      name: "继续批次",
    });
    expect(resumeButton.closest('[role="status"]')).toHaveTextContent("已暂停");

    await user.click(resumeButton);
    expect(commandMocks.resumeActiveBatch).toHaveBeenCalledTimes(1);
    const pauseButton = await screen.findByRole("button", {
      name: "暂停批次",
    });
    expect(pauseButton.closest('[role="status"]')).toHaveTextContent("运行中");

    await user.click(screen.getByRole("button", { name: "取消批次" }));
    expect(commandMocks.cancelActiveBatch).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "取消批次" }),
      ).not.toBeInTheDocument(),
    );
  });

  /** 失败行只调用精确重试，随后可在详情核对尝试与批次并批准。 */
  it("retries_only_the_failed_stage_then_shows_history_and_approves", async () => {
    let currentArticle = failedArticle();
    commandMocks.getWorkbench.mockImplementation(() =>
      Promise.resolve({
        activeRun: null,
        articles: [currentArticle],
        runHistory: [
          {
            completedCount:
              currentArticle.overallStatus === "pendingReview" ? 1 : 0,
            failedCount:
              currentArticle.overallStatus === "partialFailure" ? 1 : 0,
            finishedAtMs: 1_789_123_458_000,
            id: "run-retry-1",
            isTestFixture: true,
            startedAtMs: 1_789_123_456_000,
            status:
              currentArticle.overallStatus === "partialFailure"
                ? "partialFailure"
                : "succeeded",
            targetStage: null,
            taskIds: ["article-a"],
          },
        ],
      }),
    );
    commandMocks.retryArticle.mockImplementation(() => {
      currentArticle = pendingArticle();
      return Promise.resolve({
        batchId: "run-retry-1",
        queuedCount: 1,
        queuedTaskIds: ["article-a"],
        skipped: [],
        skippedCount: 0,
      });
    });
    commandMocks.approveArticle.mockImplementation(() => {
      currentArticle = pendingArticle();
      currentArticle.article.reviewStatus = "approved";
      currentArticle.overallStatus = "approved";
      return Promise.resolve(currentArticle);
    });
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: clipboardWrite },
    });
    await renderWorkbench();

    await screen.findByText("企业知识库搭建指南");
    expect(
      screen.getByText("AI 服务响应超时，请稍后重试。"),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "查看《企业知识库搭建指南》详情" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "仅重试失败阶段" }),
    );

    expect(commandMocks.retryArticle).toHaveBeenCalledWith("article-a");
    expect(commandMocks.regenerateArticle).not.toHaveBeenCalled();
    await screen.findByText("失败阶段已经进入重试队列。");
    await user.click(screen.getByRole("tab", { name: "历史" }));
    expect(
      await screen.findByTestId("attempt-attempt-image-2"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Token：输入 未提供 · 输出 50 · 总计 50"),
    ).toBeInTheDocument();
    expect(screen.getAllByText("实际耗时 0.5 秒").length).toBeGreaterThan(0);
    expect(screen.getAllByText("0 Token · 本地处理").length).toBeGreaterThan(0);
    expect(screen.getByText(/run-retry-1/)).toBeInTheDocument();

    const approve = screen.getByRole("button", { name: "批准文章包" });
    approve.focus();
    await user.keyboard("{Enter}");
    expect(commandMocks.approveArticle).toHaveBeenCalledWith("article-a");
    expect(
      await screen.findByText("当前文章包已经批准并保存为不可变历史。"),
    ).toBeInTheDocument();
  });

  /** 不可重试失败不显示普通重试入口，但保留需要用户明确确认的重新生成。 */
  it("requires_explicit_regeneration_for_a_non_retryable_failure", async () => {
    const currentArticle = failedArticle();
    const failure = currentArticle.article.stages.image.failure;
    if (!failure) throw new Error("missing image failure fixture");
    failure.retryable = false;
    failure.code = "PROVIDER_INVALID_RESPONSE";
    failure.message = "火山方舟生成请求结果无法确认";
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [currentArticle],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "查看《企业知识库搭建指南》详情",
      }),
    );

    expect(
      screen.queryByRole("button", { name: "仅重试失败阶段" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重新生成整篇" })).toBeEnabled();
  });

  /** 活动批次只锁定详情动作，不把打开已完成简介误报为新的生成或保存。 */
  it("opens_a_completed_summary_without_loading_or_starting_commands_during_an_active_run", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: {
        completedCount: 1,
        failedCount: 0,
        finishedAtMs: null,
        id: "batch-active",
        isTestFixture: true,
        startedAtMs: 1_789_123_456_000,
        status: "running",
        targetStage: null,
        taskIds: ["article-a"],
      },
      articles: [pendingArticle()],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: /打开《企业知识库搭建指南》的简介进行编辑/,
      }),
    );

    expect(screen.getByRole("textbox", { name: "文章简介" })).toHaveValue(
      "文章简介",
    );
    const saveSummary = screen.getByRole("button", {
      name: "保存简介修改",
    });
    expect(saveSummary).toBeDisabled();
    expect(saveSummary).not.toHaveAttribute("data-loading");
    expect(
      screen.queryByLabelText(/正在生成|等待生成/),
    ).not.toBeInTheDocument();
    expect(commandMocks.generateArticleStage).not.toHaveBeenCalled();
    expect(commandMocks.regenerateArticle).not.toHaveBeenCalled();
    expect(commandMocks.startSelectedBatch).not.toHaveBeenCalled();
    expect(commandMocks.retryArticle).not.toHaveBeenCalled();
    expect(commandMocks.retrySelectedArticles).not.toHaveBeenCalled();
    expect(commandMocks.updateArticleContent).not.toHaveBeenCalled();
    expect(commandMocks.updateArticleInputs).not.toHaveBeenCalled();
  });

  /** 保存简介未完成时只有该提交按钮展示 loading，其他详情动作仅同步锁定。 */
  it("shows_loading_only_on_the_pending_summary_save", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [pendingArticle()],
      runHistory: [],
    });
    let finishUpdate!: (article: ArticleView) => void;
    commandMocks.updateArticleContent.mockImplementation(
      () =>
        new Promise<ArticleView>((resolve) => {
          finishUpdate = resolve;
        }),
    );
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: /打开《企业知识库搭建指南》的简介进行编辑/,
      }),
    );
    const drawer = await screen.findByRole("dialog", {
      name: "文章详情与审核",
    });
    const saveSummary = within(drawer).getByRole("button", {
      name: "保存简介修改",
    });
    fireEvent.click(saveSummary);

    await waitFor(() => expect(saveSummary).toHaveAttribute("data-loading"));
    expect(saveSummary).toBeDisabled();
    expect(commandMocks.updateArticleContent).toHaveBeenCalledOnce();
    expect(commandMocks.updateArticleContent).toHaveBeenCalledWith(
      "article-a",
      null,
      "文章简介",
    );
    for (const name of [
      "重新生成整篇",
      "退回修改",
      "批准文章包",
      "重新生成《企业知识库搭建指南》的简介",
    ]) {
      const action = within(drawer).getByRole("button", { name });
      expect(action).toBeDisabled();
      expect(action).not.toHaveAttribute("data-loading");
    }

    finishUpdate(pendingArticle());

    await waitFor(() =>
      expect(saveSummary).not.toHaveAttribute("data-loading"),
    );
    expect(commandMocks.updateArticleContent).toHaveBeenCalledOnce();
  });

  /** 旧文章动作稍后完成时，不得把结果提示串到新打开的文章详情。 */
  it("keeps_a_pending_detail_action_feedback_scoped_to_its_article", async () => {
    const first = pendingArticle();
    const second = pendingArticle();
    second.article.id = "article-b";
    second.article.title = "内容营销趋势";
    second.article.artifacts.summary = "第二篇简介";
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [first, second],
      runHistory: [],
    });
    let finishUpdate!: (article: ArticleView) => void;
    commandMocks.updateArticleContent.mockImplementation(
      () =>
        new Promise<ArticleView>((resolve) => {
          finishUpdate = resolve;
        }),
    );
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: /打开《企业知识库搭建指南》的简介进行编辑/,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "保存简介修改" }));
    await waitFor(() =>
      expect(commandMocks.updateArticleContent).toHaveBeenCalledOnce(),
    );

    await user.keyboard("{Escape}");
    await user.click(
      await screen.findByRole("button", {
        name: /打开《内容营销趋势》的简介进行编辑/,
      }),
    );
    expect(screen.getByRole("textbox", { name: "文章简介" })).toHaveValue(
      "第二篇简介",
    );

    finishUpdate(first);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "保存简介修改" }),
      ).toBeEnabled(),
    );
    expect(
      screen.queryByText("简介已保存，依赖配图已标记为过期。"),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "文章简介" })).toHaveValue(
      "第二篇简介",
    );
  });

  /** Markdown 与 HTML 均通过并列页签复制包含当前配图的完整格式。 */
  it("copies_image_complete_markdown_and_html_projections", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [pendingArticle()],
      runHistory: [],
    });
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: clipboardWrite },
    });
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "查看《企业知识库搭建指南》详情",
      }),
    );
    await user.click(screen.getByRole("tab", { name: "格式" }));
    const markdownCopy = await screen.findByRole("button", {
      name: "复制 Markdown",
    });
    await waitFor(() => expect(markdownCopy).toBeEnabled());
    await user.click(markdownCopy);
    await user.click(screen.getByRole("tab", { name: "HTML" }));
    await user.click(screen.getByRole("button", { name: "复制 HTML" }));

    expect(clipboardWrite.mock.calls[0]?.[0]).toContain(
      "![企业知识库搭建指南](data:image/png;base64,aGVsbG8=)",
    );
    expect(clipboardWrite.mock.calls[1]?.[0]).toContain(
      '<img src="data:image/png;base64,aGVsbG8=" alt="企业知识库搭建指南">',
    );
    expect(await screen.findByText("已复制 HTML。")).toBeInTheDocument();
  });

  /** 剪贴板拒绝写入时展示明确失败，且保留页面中的原始 Markdown 产物。 */
  it("keeps_the_artifact_when_clipboard_write_is_rejected", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [pendingArticle()],
      runHistory: [],
    });
    const user = userEvent.setup();
    clipboardWrite.mockRejectedValueOnce(new Error("clipboard denied"));
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: clipboardWrite },
    });
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "查看《企业知识库搭建指南》详情",
      }),
    );
    await user.click(screen.getByRole("tab", { name: "格式" }));
    await user.click(screen.getByRole("radio", { name: "源码" }));
    const markdownArtifact = screen.getByText("# 企业知识库搭建指南", {
      exact: false,
      selector: "pre",
    });
    const markdownCopy = screen.getByRole("button", {
      name: "复制 Markdown",
    });
    await waitFor(() => expect(markdownCopy).toBeEnabled());
    await user.click(markdownCopy);

    expect(clipboardWrite.mock.calls[0]?.[0]).toContain(
      "![企业知识库搭建指南](data:image/png;base64,aGVsbG8=)",
    );
    expect(
      await screen.findByText("无法复制，请检查系统剪贴板权限后重试。"),
    ).toBeInTheDocument();
    expect(markdownArtifact).toHaveTextContent("# 企业知识库搭建指南");
    expect(screen.queryByText("已复制 Markdown。")).not.toBeInTheDocument();
    expect(commandMocks.updateArticleContent).not.toHaveBeenCalled();
  });

  /** 未生成格式产物时复制动作不可触发。 */
  it("disables_copy_actions_for_empty_artifacts", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [draftArticle("article-a", "企业知识库搭建指南")],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "查看《企业知识库搭建指南》详情",
      }),
    );
    await user.click(screen.getByRole("tab", { name: "格式" }));

    expect(
      screen.getByRole("button", { name: "复制 Markdown" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("tab", { name: "HTML" }));
    expect(screen.getByRole("button", { name: "复制 HTML" })).toBeDisabled();
    expect(clipboardWrite).not.toHaveBeenCalled();
  });

  /** 正文预览可直接编辑；配图编辑器只在配图页签读取，并随尝试刷新。 */
  it("edits_from_the_article_preview_and_loads_images_only_in_the_image_tab", async () => {
    let currentArticle = pendingArticle();
    commandMocks.getWorkbench.mockImplementation(() =>
      Promise.resolve({
        activeRun: null,
        articles: [currentArticle],
        runHistory: [],
      }),
    );
    commandMocks.loadArticleImageEditor
      .mockResolvedValueOnce({
        canSave: true,
        currentSpec: {
          crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
          flipHorizontal: false,
          flipVertical: false,
          rotationQuarterTurns: 0,
        },
        editRevision: 0,
        imageAttemptId: "attempt-image-2",
        imageDataUrl: "data:image/png;base64,b2xk",
        packageVersion: 1,
        rowVersion: 1,
        saveBlockedReason: null,
        sourceHeight: 675,
        sourceWidth: 1200,
      })
      .mockResolvedValueOnce({
        canSave: true,
        currentSpec: {
          crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
          flipHorizontal: false,
          flipVertical: false,
          rotationQuarterTurns: 0,
        },
        editRevision: 0,
        imageAttemptId: "attempt-image-3",
        imageDataUrl: "data:image/png;base64,bmV3",
        packageVersion: 1,
        rowVersion: 2,
        saveBlockedReason: null,
        sourceHeight: 675,
        sourceWidth: 1200,
      });
    commandMocks.loadArticleImage
      .mockResolvedValueOnce("data:image/png;base64,b2xk")
      .mockResolvedValueOnce("data:image/png;base64,bmV3");
    commandMocks.generateArticleStage.mockImplementation(() => {
      const next = pendingArticle();
      if (next.article.artifacts.imageLineage) {
        next.article.artifacts.imageLineage.attemptId = "attempt-image-3";
      }
      next.article.rowVersion += 1;
      currentArticle = next;
      return Promise.resolve({
        batchId: "batch-image-3",
        queuedCount: 1,
        queuedTaskIds: ["article-a"],
        skipped: [],
        skippedCount: 0,
      });
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: /打开《企业知识库搭建指南》的正文进行编辑/,
      }),
    );
    expect(screen.getByRole("tab", { name: "正文" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(commandMocks.loadArticleImageEditor).not.toHaveBeenCalled();
    const editor = screen.getByRole("textbox", { name: "Markdown 正文" });
    await user.clear(editor);
    await user.type(editor, "# 手工修改后的正文");
    await user.click(screen.getByRole("button", { name: "保存正文修改" }));
    expect(commandMocks.updateArticleContent).toHaveBeenCalledWith(
      "article-a",
      "# 手工修改后的正文",
      null,
    );
    await user.click(screen.getByRole("tab", { name: "配图" }));
    await waitFor(() =>
      expect(commandMocks.loadArticleImage).toHaveBeenCalledTimes(1),
    );
    expect(commandMocks.loadArticleImageEditor).not.toHaveBeenCalled();
    expect((await screen.findAllByText(/简介来源 1/)).length).toBeGreaterThan(
      0,
    );
    expect(
      screen.getByRole("img", {
        name: "《企业知识库搭建指南》的 AI 生成配图",
      }),
    ).toHaveAttribute("src", "data:image/png;base64,b2xk");

    await user.click(
      screen.getByRole("button", {
        name: "打开《企业知识库搭建指南》配图高级编辑",
      }),
    );
    const imageEditor = await screen.findByRole("dialog", {
      name: "《企业知识库搭建指南》配图高级编辑",
    });
    await waitFor(() =>
      expect(commandMocks.loadArticleImageEditor).toHaveBeenCalledTimes(1),
    );
    expect(
      within(imageEditor)
        .getByRole("img", {
          name: "《企业知识库搭建指南》的 AI 生成配图",
        })
        .querySelector("image"),
    ).toHaveAttribute("href", "data:image/png;base64,b2xk");
    await user.click(
      within(imageEditor).getByRole("button", {
        name: "关闭配图高级编辑",
      }),
    );

    const details = screen.getByRole("dialog", { name: "文章详情与审核" });
    await user.click(
      within(details).getByRole("button", {
        name: "重新生成《企业知识库搭建指南》的配图",
      }),
    );
    expect(commandMocks.generateArticleStage).not.toHaveBeenCalled();
    const confirmation = await screen.findByRole("dialog", {
      name: "确认单步生成配图",
    });
    await user.click(
      within(confirmation).getByRole("button", { name: "确认并生成配图" }),
    );
    expect(commandMocks.generateArticleStage).toHaveBeenCalledWith(
      "article-a",
      "image",
      1,
      1,
      true,
      "provider-e2e-fixture",
      1,
    );
    await waitFor(() =>
      expect(commandMocks.loadArticleImage).toHaveBeenCalledTimes(2),
    );
    expect(
      screen.getByRole("img", {
        name: "《企业知识库搭建指南》的 AI 生成配图",
      }),
    ).toHaveAttribute("src", "data:image/png;base64,bmV3");
    expect(commandMocks.loadArticleImageEditor).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "高级编辑配图" }));
    const refreshedEditor = await screen.findByRole("dialog", {
      name: "《企业知识库搭建指南》配图高级编辑",
    });
    await waitFor(() =>
      expect(commandMocks.loadArticleImageEditor).toHaveBeenCalledTimes(2),
    );
    expect(
      within(refreshedEditor)
        .getByRole("img", {
          name: "《企业知识库搭建指南》的 AI 生成配图",
        })
        .querySelector("image"),
    ).toHaveAttribute("href", "data:image/png;base64,bmV3");
  });
});
