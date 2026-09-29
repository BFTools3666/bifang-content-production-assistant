import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "@testing-library/jest-dom/vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createInstance, type i18n } from "i18next";
import { createStore, Provider as JotaiProvider } from "jotai";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import type { ArticleView, StageProgress } from "./workbenchTypes";

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
  resumeActiveBatch: vi.fn(),
  retryArticle: vi.fn(),
  retrySelectedArticles: vi.fn(),
  returnArticleToDraft: vi.fn(),
  saveArticleImageEdit: vi.fn(),
  startSelectedBatch: vi.fn(),
  syncArticleToConnector: vi.fn(),
  updateArticleContent: vi.fn(),
  updateArticleInputs: vi.fn(),
}));

vi.mock("@tanstack/react-router", () => ({ useNavigate: () => vi.fn() }));
vi.mock("./workbenchCommands", () => ({ ...commandMocks }));

import { ArticleWorkbenchPage } from "./ArticleWorkbenchPage";
import { workbenchPageSession } from "./workbenchState";

/** 为 Mantine 提供不执行几何测量的观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}
  /** jsdom 不执行真实布局测量。 */
  observe(): void {}
  /** jsdom 允许停止观察但没有持久状态。 */
  unobserve(): void {}
}

/** 构造尚未开始的阶段进度。 */
function emptyStage(): StageProgress {
  return {
    activeAttemptId: null,
    artifactVersion: null,
    attempts: 0,
    failure: null,
    status: "notStarted",
  };
}

/** 构造能够进入真实 Grid 的草稿内容。 */
function draftArticle(index: number): ArticleView {
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
      attemptHistory: [],
      createdAtMs: 1_789_123_456_000 + index,
      id: `article-${String(index).padStart(2, "0")}`,
      keywords: [],
      packageHistory: [],
      packageVersion: 1,
      requirements: "",
      reviewStatus: "draft",
      rowVersion: 1,
      stages: {
        article: emptyStage(),
        format: emptyStage(),
        image: emptyStage(),
        summary: emptyStage(),
      },
      title: `内容 ${String(index).padStart(2, "0")}`,
      updatedAtMs: 1_789_123_456_000 + index,
    },
    hasImage: false,
    overallStatus: "draft",
  };
}

/** 创建一次可受测试控制的异步工作台读取。 */
function deferredWorkbench() {
  let resolve!: (value: unknown) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

/** 在可复用的根 store 和 QueryClient 下模拟一次 route 挂载。 */
function renderWorkbench(
  instance: i18n,
  store: ReturnType<typeof createStore>,
  queryClient: QueryClient,
) {
  return render(
    <I18nextProvider i18n={instance}>
      <JotaiProvider store={store}>
        <QueryClientProvider client={queryClient}>
          <MantineProvider env="test">
            <ArticleWorkbenchPage />
          </MantineProvider>
        </QueryClientProvider>
      </JotaiProvider>
    </I18nextProvider>,
  );
}

describe("workbench Grid standards", () => {
  let instance: i18n;

  beforeEach(async () => {
    for (const mock of Object.values(commandMocks)) mock.mockReset();
    commandMocks.listConnectorConfigs.mockResolvedValue([]);
    instance = createInstance();
    await instance.init({
      fallbackLng: "en-US",
      lng: "zh-CN",
      resources: {
        "en-US": { app: enUS },
        "zh-CN": { app: zhCN },
      },
    });
    Object.defineProperty(globalThis, "ResizeObserver", {
      configurable: true,
      value: TestResizeObserver,
    });
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      value: undefined,
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
    commandMocks.listProviderConfigs.mockResolvedValue([]);
    commandMocks.loadArticleThumbnail.mockResolvedValue(
      "data:image/png;base64,aGVsbG8=",
    );
  });

  afterEach(() => cleanup());

  /** 默认只显示 20 行，路由重挂保留页码，全选仍覆盖全部筛选结果。 */
  it("paginates a full snapshot and selects every matching row", async () => {
    const articles = Array.from({ length: 25 }, (_, index) =>
      draftArticle(index),
    );
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles,
      runHistory: [],
    });
    const store = createStore();
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const user = userEvent.setup();
    const firstRoute = renderWorkbench(instance, store, queryClient);

    await screen.findByTestId("article-row-article-24");
    expect(screen.getAllByTestId(/^article-row-/)).toHaveLength(20);
    expect(
      screen.getByRole("combobox", {
        name: zhCN.workbench.pagination.page_size_label,
      }),
    ).toHaveValue("20");
    await user.click(screen.getByRole("button", { name: "2" }));
    await waitFor(() =>
      expect(store.get(workbenchPageSession.stateAtom).page).toBe(2),
    );
    expect(screen.getAllByTestId(/^article-row-/)).toHaveLength(5);

    firstRoute.unmount();
    renderWorkbench(instance, store, queryClient);
    await screen.findByTestId("article-row-article-04");
    expect(store.get(workbenchPageSession.stateAtom).page).toBe(2);
    expect(screen.getAllByTestId(/^article-row-/)).toHaveLength(5);

    await user.click(
      screen.getByRole("checkbox", { name: "全选当前结果（25）" }),
    );
    expect(
      screen.getByRole("button", { name: "运行所选（25）" }),
    ).toBeInTheDocument();

    const pageSize = screen.getByRole("combobox", {
      name: zhCN.workbench.pagination.page_size_label,
    });
    await user.click(pageSize);
    expect(screen.getByRole("option", { name: "20" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "50" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "100" })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: "50" }));
    await waitFor(() =>
      expect(store.get(workbenchPageSession.stateAtom)).toMatchObject({
        page: 1,
        pageSize: 50,
      }),
    );
    expect(screen.getAllByTestId(/^article-row-/)).toHaveLength(25);
  });

  /** 成功快照让越界空页只回到第一页，不为客户端页码重新请求后端。 */
  it("returns from a successful out-of-range page without refetching", async () => {
    commandMocks.getWorkbench.mockResolvedValue({
      activeRun: null,
      articles: Array.from({ length: 40 }, (_, index) => draftArticle(index)),
      runHistory: [],
    });
    const store = createStore();
    store.set(workbenchPageSession.setPaginationAtom, {
      page: 3,
      pageSize: 20,
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    renderWorkbench(instance, store, queryClient);

    await waitFor(() =>
      expect(store.get(workbenchPageSession.stateAtom).page).toBe(1),
    );
    expect(
      await screen.findByTestId("article-row-article-39"),
    ).toBeInTheDocument();
    expect(screen.getAllByTestId(/^article-row-/)).toHaveLength(20);
    expect(commandMocks.getWorkbench).toHaveBeenCalledOnce();
  });

  /** 首次等待不伪装成空工作区，首次失败提供可重试错误。 */
  it("separates initial loading from a retryable initial error", async () => {
    const deferred = deferredWorkbench();
    commandMocks.getWorkbench.mockReturnValue(deferred.promise);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    renderWorkbench(instance, createStore(), queryClient);

    expect(await screen.findByText(zhCN.workbench.loading)).toBeInTheDocument();
    expect(
      screen.queryByText(zhCN.workbench.empty.title),
    ).not.toBeInTheDocument();

    await act(async () => deferred.reject(new Error("offline")));
    expect(
      await screen.findByText(zhCN.workbench.load_failed),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: zhCN.common.retry }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(zhCN.workbench.empty.title),
    ).not.toBeInTheDocument();
  });

  /** 后台刷新及其失败都保留上次成功快照和当前 Grid。 */
  it("keeps stale rows during background refresh and refresh failure", async () => {
    const snapshot = {
      activeRun: null,
      articles: [draftArticle(1)],
      runHistory: [],
    };
    const deferred = deferredWorkbench();
    commandMocks.getWorkbench.mockReturnValue(deferred.promise);
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    queryClient.setQueryData(["workbench"], snapshot);
    renderWorkbench(instance, createStore(), queryClient);

    expect(await screen.findByText("内容 01")).toBeInTheDocument();
    expect(screen.getByText(zhCN.workbench.refreshing)).toBeInTheDocument();

    await act(async () => deferred.reject(new Error("refresh offline")));
    expect(
      await screen.findByText(zhCN.workbench.refresh_failed),
    ).toBeInTheDocument();
    expect(screen.getByText("内容 01")).toBeInTheDocument();
  });
});
