import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createStore } from "jotai";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

import {
  ArticleWorkbenchPage,
  deriveVisibleArticles,
} from "./ArticleWorkbenchPage";
import { workbenchSortAtom } from "./workbenchState";

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

  /** 新进程默认按创建时间倒序，同一时间以稳定 ID 倒序打破平局。 */
  it("defaults_to_created_descending_with_a_stable_id_tie_break", () => {
    expect(createStore().get(workbenchSortAtom)).toBe("created");
    const oldest = draftArticle("article-old", "Old");
    oldest.article.createdAtMs = 100;
    const tiedLow = draftArticle("article-a", "A");
    tiedLow.article.createdAtMs = 200;
    const tiedHigh = draftArticle("article-z", "Z");
    tiedHigh.article.createdAtMs = 200;

    expect(
      deriveVisibleArticles(
        [oldest, tiedLow, tiedHigh],
        "",
        "all",
        "created",
        "zh-CN",
      ).map((view) => view.article.id),
    ).toEqual(["article-z", "article-a", "article-old"]);
  });

  /** 只把用户明确选中的子集送入只读预检，并在二次确认后才启动。 */
  it("previews_and_runs_only_the_selected_subset", async () => {
    commandMocks.previewSelectedBatch.mockResolvedValue({
      batchId: null,
      queuedCount: 1,
      queuedTaskIds: ["article-a"],
      skipped: [],
      skippedCount: 0,
    });
    commandMocks.startSelectedBatch.mockResolvedValue({
      batchId: "batch-1",
      queuedCount: 1,
      queuedTaskIds: ["article-a"],
      skipped: [],
      skippedCount: 0,
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await screen.findByText("企业知识库搭建指南");
    await user.click(
      screen.getByRole("checkbox", { name: /企业知识库搭建指南/ }),
    );
    await user.click(screen.getByRole("button", { name: "批量制作（1 篇）" }));

    expect(commandMocks.previewSelectedBatch).toHaveBeenCalledWith([
      "article-a",
    ]);
    expect(commandMocks.startSelectedBatch).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("dialog", { name: "开始制作这些文章？" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "确认并开始生成" }));
    expect(commandMocks.startSelectedBatch).toHaveBeenCalledWith(["article-a"]);
  });

  /** Grid 正文按钮只调用带行版本保护的单步命令，不代理批量或旧重生成动作。 */
  it("runs_only_the_clicked_text_stage_from_the_grid", async () => {
    commandMocks.generateArticleStage.mockResolvedValue({
      batchId: "stage-article-1",
      queuedCount: 1,
      queuedTaskIds: ["article-a"],
      skipped: [],
      skippedCount: 0,
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "生成《企业知识库搭建指南》的正文",
      }),
    );

    expect(commandMocks.generateArticleStage).toHaveBeenCalledWith(
      "article-a",
      "article",
      1,
      1,
      false,
      null,
      null,
    );
    expect(commandMocks.previewSelectedBatch).not.toHaveBeenCalled();
    expect(
      await screen.findByText("《企业知识库搭建指南》的正文已开始单步生成。"),
    ).toBeInTheDocument();
  });

  /** 顶部说明准确列出缺失阶段，正文服务齐全时正文按钮仍能发起单步。 */
  it("lists_missing_summary_and_image_while_allowing_article_generation", async () => {
    const [provider] = await commandMocks.listProviderConfigs();
    commandMocks.listProviderConfigs.mockResolvedValue([
      { ...provider, defaultForSummary: false, defaultForImage: false, isTestFixture: false, kind: "openAiCompatible" },
    ]);
    commandMocks.generateArticleStage.mockResolvedValue({ batchId: "article-only", queuedCount: 1, queuedTaskIds: ["article-a"], skipped: [], skippedCount: 0 });
    const user = userEvent.setup();
    await renderWorkbench();
    expect(await screen.findByText(/还需要设置：简介、配图/u)).toBeInTheDocument();
    const generate = screen.getByRole("button", { name: "生成《企业知识库搭建指南》的正文" });
    expect(generate).toBeEnabled();
    await user.click(generate);
    expect(commandMocks.generateArticleStage).toHaveBeenCalledWith("article-a", "article", 1, 1, false, null, null);
    expect(commandMocks.startSelectedBatch).not.toHaveBeenCalled();
  });

  /** 同一阶段按钮的同步锁在首个 Promise 完成前拒绝快速双击。 */
  it("deduplicates_rapid_stage_button_clicks", async () => {
    let finish!: (value: unknown) => void;
    commandMocks.generateArticleStage.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await renderWorkbench();
    const button = await screen.findByRole("button", {
      name: "生成《企业知识库搭建指南》的正文",
    });

    fireEvent.click(button);
    fireEvent.click(button);
    expect(commandMocks.generateArticleStage).toHaveBeenCalledTimes(1);

    finish({
      batchId: "stage-article-1",
      queuedCount: 1,
      queuedTaskIds: ["article-a"],
      skipped: [],
      skippedCount: 0,
    });
    await screen.findByText("《企业知识库搭建指南》的正文已开始单步生成。");
  });

  /** 单步 IPC 未完成时，批量运行和精确重试共享同一同步锁，不能交叉启动。 */
  it("locks_batch_and_retry_actions_while_a_stage_command_is_pending", async () => {
    let finish!: (value: unknown) => void;
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [failedArticle()],
      runHistory: [],
    });
    commandMocks.generateArticleStage.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    await renderWorkbench();
    await user.click(
      await screen.findByRole("checkbox", { name: /企业知识库搭建指南/ }),
    );
    const runSelected = screen.getByRole("button", { name: "批量制作（1 篇）" });
    const retrySelected = screen.getByRole("button", {
      name: "重试所选失败项",
    });
    expect(runSelected).toBeEnabled();
    expect(retrySelected).toBeEnabled();

    fireEvent.click(
      screen.getByRole("button", {
        name: "重新生成《企业知识库搭建指南》的简介",
      }),
    );
    await waitFor(() => {
      expect(runSelected).toBeDisabled();
      expect(retrySelected).toBeDisabled();
    });
    fireEvent.click(runSelected);
    fireEvent.click(retrySelected);
    expect(commandMocks.previewSelectedBatch).not.toHaveBeenCalled();
    expect(commandMocks.retrySelectedArticles).not.toHaveBeenCalled();

    finish({
      batchId: "stage-summary-1",
      queuedCount: 1,
      queuedTaskIds: ["article-a"],
      skipped: [],
      skippedCount: 0,
    });
    await screen.findByText("《企业知识库搭建指南》的简介已开始单步生成。");
  });

  /** 配图按钮先确认所见服务修订；取消零调用，确认后只发一次单步命令。 */
  it("confirms_the_exact_image_provider_before_single_stage_generation", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [failedArticle()],
      runHistory: [],
    });
    commandMocks.generateArticleStage.mockResolvedValue({
      batchId: "stage-image-1",
      queuedCount: 1,
      queuedTaskIds: ["article-a"],
      skipped: [],
      skippedCount: 0,
    });
    const user = userEvent.setup();
    await renderWorkbench();

    const imageAction = await screen.findByRole("button", {
      name: "重新生成《企业知识库搭建指南》的配图",
    });
    await user.click(imageAction);
    expect(commandMocks.generateArticleStage).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("dialog", { name: "确认单步生成配图" }),
    ).toHaveTextContent("E2E 测试夹具（非真实 AI）");
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(commandMocks.generateArticleStage).not.toHaveBeenCalled();

    await user.click(imageAction);
    const confirmImage = await screen.findByRole("button", {
      name: "确认并生成配图",
    });
    fireEvent.click(confirmImage);
    fireEvent.click(confirmImage);
    expect(commandMocks.generateArticleStage).toHaveBeenCalledOnce();
    expect(commandMocks.generateArticleStage).toHaveBeenCalledWith(
      "article-a",
      "image",
      1,
      1,
      true,
      "provider-e2e-fixture",
      1,
    );
  });

  /** 只有结果确实可能已经提交的稳定错误码才显示潜在重复计费警告。 */
  it("warns_when_the_previous_image_outcome_is_uncertain", async () => {
    const article = failedArticle("PROVIDER_INVALID_RESPONSE", false);
    article.article.stages.image.status = "stale";
    article.article.stages.image.failure = null;
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [article],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "重新生成《企业知识库搭建指南》的配图",
      }),
    );

    expect(
      await screen.findByText(
        "上次图片结果可能已经生成并计费。请先核对服务商控制台，再决定是否重新生成。",
      ),
    ).toBeInTheDocument();
  });

  /** 鉴权失败虽然不可自动重试，但并不表示图片请求已经产生费用。 */
  it("does_not_warn_about_unknown_outcome_for_authentication_failure", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [failedArticle("PROVIDER_AUTHENTICATION", false)],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "重新生成《企业知识库搭建指南》的配图",
      }),
    );

    expect(
      screen.queryByText(
        "上次图片结果可能已经生成并计费。请先核对服务商控制台，再决定是否重新生成。",
      ),
    ).not.toBeInTheDocument();
  });

  /** 更晚的成功配图已经收口旧的不确定尝试，后续重生成不再误报未知结果。 */
  it("stops_unknown_outcome_warning_at_a_later_successful_image_attempt", async () => {
    const article = pendingArticle();
    const [failedImageAttempt] = article.article.attemptHistory;
    if (!failedImageAttempt) {
      throw new Error("测试夹具必须包含首次配图失败尝试");
    }
    failedImageAttempt.failure = {
      code: "PROVIDER_UNKNOWN_OUTCOME",
      message: "结果无法确认",
      retryAfterMs: null,
      retryable: true,
    };
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [article],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await user.click(
      await screen.findByRole("button", {
        name: "重新生成《企业知识库搭建指南》的配图",
      }),
    );

    expect(
      screen.queryByText(
        "上次图片结果可能已经生成并计费。请先核对服务商控制台，再决定是否重新生成。",
      ),
    ).not.toBeInTheDocument();
  });

  /** 纯键盘提交 20 个有效标题，保留重复、忽略空行并在关闭后恢复触发器焦点。 */
  it("adds_twenty_titles_by_keyboard_and_restores_focus", async () => {
    const titleLines = Array.from(
      { length: 19 },
      (_, index) => `SEO 标题 ${index + 1}`,
    );
    titleLines.push("SEO 标题 1");
    titleLines.splice(7, 0, "");
    const titleText = titleLines.join("\n");
    const keyboardText = titleText.replaceAll("\n", "{Enter}");
    commandMocks.addArticleTitles.mockResolvedValue({
      createdCount: 20,
      duplicateCount: 1,
      ignoredBlankCount: 1,
      taskIds: Array.from({ length: 20 }, (_, index) => `article-${index}`),
    });
    const user = userEvent.setup();
    await renderWorkbench();

    const addButton = await screen.findByRole("button", {
      name: "导入现有标题",
    });
    addButton.focus();
    await user.keyboard("{Enter}");
    await screen.findByRole("dialog", { name: "批量添加文章标题" });
    const input = screen.getByRole("textbox", { name: "文章标题列表" });
    expect(input).toHaveFocus();
    await user.type(input, keyboardText);
    const submit = screen.getByRole("button", { name: "创建文章任务" });
    submit.focus();
    await user.keyboard("{Enter}");

    expect(commandMocks.addArticleTitles).toHaveBeenCalledWith(titleText);
    expect(
      await screen.findByText(/已创建 20 条任务.*1 条标题重复/),
    ).toBeInTheDocument();
    await waitFor(() => expect(addButton).toHaveFocus());
  });

  /** 全选、取消全选、部分选择和取消运行预检均可只用键盘完成。 */
  it("supports_keyboard_selection_and_preview_cancellation", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: Array.from({ length: 20 }, (_, index) =>
        draftArticle(`article-${index + 1}`, `SEO 标题 ${index + 1}`),
      ),
      runHistory: [],
    });
    commandMocks.previewSelectedBatch.mockResolvedValue({
      batchId: null,
      queuedCount: 2,
      queuedTaskIds: ["article-1", "article-2"],
      skipped: [],
      skippedCount: 0,
    });
    const user = userEvent.setup();
    await renderWorkbench();

    const selectAll = await screen.findByRole("checkbox", {
      name: "全选当前结果（20）",
    });
    selectAll.focus();
    await user.keyboard("[Space]");
    expect(
      screen.getByRole("button", { name: "批量制作（20 篇）" }),
    ).toBeEnabled();
    await user.keyboard("[Space]");
    expect(
      screen.getByRole("button", { name: "批量制作（0 篇）" }),
    ).toBeDisabled();

    for (const title of ["SEO 标题 1", "SEO 标题 2"]) {
      const checkbox = screen.getByRole("checkbox", {
        name: `选择《${title}》`,
      });
      checkbox.focus();
      await user.keyboard("[Space]");
    }
    const runButton = screen.getByRole("button", { name: "批量制作（2 篇）" });
    runButton.focus();
    await user.keyboard("{Enter}");
    expect(
      await screen.findByRole("dialog", { name: "开始制作这些文章？" }),
    ).toBeInTheDocument();
    const cancelButton = screen.getByRole("button", { name: "取消" });
    cancelButton.focus();
    await user.keyboard("{Enter}");

    expect(commandMocks.previewSelectedBatch).toHaveBeenCalledWith([
      "article-1",
      "article-2",
    ]);
    expect(commandMocks.startSelectedBatch).not.toHaveBeenCalled();
    await waitFor(() => expect(runButton).toHaveFocus());
  });

  /** 搜索、总体状态筛选和标题排序均直接改变当前 Grid 的可见顺序。 */
  it("filters_by_search_and_status_then_sorts_visible_rows", async () => {
    const newestDraft = draftArticle("article-zulu", "Zulu SEO");
    newestDraft.article.createdAtMs = 300;
    newestDraft.article.updatedAtMs = 300;
    const failed = failedArticle();
    failed.article.id = "article-beta";
    failed.article.title = "Beta Failure";
    failed.article.createdAtMs = 200;
    failed.article.updatedAtMs = 200;
    const pending = pendingArticle();
    pending.article.id = "article-alpha";
    pending.article.title = "Alpha Review";
    pending.article.createdAtMs = 100;
    pending.article.updatedAtMs = 100;
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [pending, newestDraft, failed],
      runHistory: [],
    });
    const user = userEvent.setup();
    await renderWorkbench();

    await screen.findByText("Zulu SEO");
    expect(screen.getByRole("combobox", { name: "排序" })).toHaveValue(
      "创建时间（最新优先）",
    );
    const rowIds = () =>
      screen
        .getAllByTestId(/^article-row-/)
        .map((row) => row.getAttribute("data-testid"));
    expect(rowIds()).toEqual([
      "article-row-article-zulu",
      "article-row-article-beta",
      "article-row-article-alpha",
    ]);

    await user.click(screen.getByRole("combobox", { name: "排序" }));
    await user.click(await screen.findByRole("option", { name: "标题" }));
    expect(rowIds()).toEqual([
      "article-row-article-alpha",
      "article-row-article-beta",
      "article-row-article-zulu",
    ]);

    const search = screen.getByRole("textbox", { name: "搜索标题" });
    await user.type(search, "alpha");
    expect(rowIds()).toEqual(["article-row-article-alpha"]);

    await user.clear(search);
    await user.click(screen.getByRole("combobox", { name: "总体状态" }));
    await user.click(await screen.findByRole("option", { name: "部分失败" }));
    expect(rowIds()).toEqual(["article-row-article-beta"]);
  });

  /** 筛选后的全选只加入当前可见任务，不会把被筛掉的任务加入批次预检。 */
  it("selects_only_the_currently_filtered_results", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: [
        draftArticle("article-visible-a", "目标文章 A"),
        draftArticle("article-hidden", "无关文章"),
        draftArticle("article-visible-b", "目标文章 B"),
      ],
      runHistory: [],
    });
    commandMocks.previewSelectedBatch.mockResolvedValue({
      batchId: null,
      queuedCount: 2,
      queuedTaskIds: ["article-visible-a", "article-visible-b"],
      skipped: [],
      skippedCount: 0,
    });
    const user = userEvent.setup();
    await renderWorkbench();

    const search = await screen.findByRole("textbox", { name: "搜索标题" });
    await user.type(search, "目标");
    await user.click(
      screen.getByRole("checkbox", { name: "全选当前结果（2）" }),
    );
    expect(screen.getByRole("button", { name: "批量制作（2 篇）" })).toBeEnabled();

    await user.clear(search);
    expect(
      screen.getByRole("checkbox", { name: "选择《目标文章 A》" }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "选择《目标文章 B》" }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "选择《无关文章》" }),
    ).not.toBeChecked();

    await user.click(screen.getByRole("button", { name: "批量制作（2 篇）" }));
    expect(commandMocks.previewSelectedBatch).toHaveBeenCalledWith([
      "article-visible-b",
      "article-visible-a",
    ]);
  });
});
