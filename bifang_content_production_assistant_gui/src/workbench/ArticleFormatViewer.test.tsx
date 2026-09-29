import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createInstance } from "i18next";
import { createStore, Provider as JotaiProvider } from "jotai";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";

const commandMocks = vi.hoisted(() => ({
  listConnectorConfigs: vi.fn(),
  loadArticleImage: vi.fn(),
  syncArticleToConnector: vi.fn(),
}));

vi.mock("./workbenchCommands", () => commandMocks);

import { ArticleFormatViewer } from "./ArticleFormatViewer";
import { articleFormatTabAtom, articleFormatViewAtom } from "./workbenchState";

const IMAGE = "data:image/png;base64,aGVsbG8=";

/** Mantine 指示器在 jsdom 中不需要真实尺寸回调。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}
  /** 测试不执行真实布局测量。 */
  observe(): void {}
  /** 测试允许停止观察但不维护宿主状态。 */
  unobserve(): void {}
}

/** 在真实根 store 与双语环境下渲染格式查看器。 */
async function renderViewer(
  overrides: Partial<React.ComponentProps<typeof ArticleFormatViewer>> = {},
): Promise<{
  onCopy: ReturnType<typeof vi.fn>;
  rerenderViewer: (
    next: Partial<React.ComponentProps<typeof ArticleFormatViewer>>,
  ) => void;
  store: ReturnType<typeof createStore>;
}> {
  const i18n = createInstance();
  await i18n.init({
    fallbackLng: "en-US",
    lng: "zh-CN",
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  const store = createStore();
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const onCopy = vi.fn().mockResolvedValue(undefined);
  let props: React.ComponentProps<typeof ArticleFormatViewer> = {
    active: true,
    formatReady: true,
    hasImage: true,
    html: "<h1>文章标题</h1><p>正文</p>",
    imageRefreshKey: "1:0:attempt-image-1",
    markdown: "# 文章标题\n\n正文",
    onCopy,
    packageVersion: 3,
    rowVersion: 7,
    taskId: "article-a",
    title: "文章标题",
    ...overrides,
  };
  const view = (): React.ReactElement => (
    <I18nextProvider i18n={i18n}>
      <JotaiProvider store={store}>
        <QueryClientProvider client={queryClient}>
          <MantineProvider env="test">
            <ArticleFormatViewer {...props} />
          </MantineProvider>
        </QueryClientProvider>
      </JotaiProvider>
    </I18nextProvider>
  );
  const rendered = render(view());
  return {
    onCopy,
    rerenderViewer: (next) => {
      props = { ...props, ...next };
      rendered.rerender(view());
    },
    store,
  };
}

describe("article format viewer", () => {
  beforeEach(() => {
    commandMocks.listConnectorConfigs.mockReset();
    commandMocks.listConnectorConfigs.mockResolvedValue([]);
    commandMocks.loadArticleImage.mockReset();
    commandMocks.loadArticleImage.mockResolvedValue(IMAGE);
    commandMocks.syncArticleToConnector.mockReset();
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
  });
  afterEach(() => cleanup());

  /** Markdown 与 HTML 是同级选项卡，两种格式都能解释并复制完整配图。 */
  it("switches_between_parallel_formats_and_embeds_the_image", async () => {
    const user = userEvent.setup();
    const { onCopy } = await renderViewer();
    const formatTabs = screen.getByRole("tablist", { name: "选择格式类型" });

    expect(within(formatTabs).getAllByRole("tab")).toHaveLength(2);
    expect(
      within(formatTabs).getByRole("tab", { name: "Markdown" }),
    ).toHaveAttribute("aria-selected", "true");
    await waitFor(() =>
      expect(commandMocks.loadArticleImage).toHaveBeenCalledWith("article-a"),
    );

    const markdownPreview = await screen.findByTitle("Markdown 渲染预览");
    await waitFor(() =>
      expect(markdownPreview.getAttribute("srcdoc")).toContain(IMAGE),
    );
    expect(markdownPreview).toHaveAttribute("sandbox", "");
    expect(markdownPreview.getAttribute("srcdoc")).toContain(
      "Content-Security-Policy",
    );

    await user.click(screen.getByRole("radio", { name: "源码" }));
    expect(
      screen.getByText(/data:image\/png;base64,…/, { selector: "pre" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "复制 Markdown" }));
    expect(onCopy).toHaveBeenLastCalledWith(
      `# 文章标题\n\n![文章标题](${IMAGE})\n\n正文`,
      "Markdown",
    );

    await user.click(within(formatTabs).getByRole("tab", { name: "HTML" }));
    expect(screen.getByRole("radio", { name: "源码" })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "复制 HTML" }));
    expect(onCopy).toHaveBeenLastCalledWith(
      `<h1>文章标题</h1>\n<figure><img src="${IMAGE}" alt="文章标题"><figcaption>文章标题</figcaption></figure>\n<p>正文</p>`,
      "HTML",
    );
  });

  /** 没有配图时不读取图片，现有文本格式仍可查看和复制。 */
  it("does_not_load_an_image_for_an_image_free_article", async () => {
    const user = userEvent.setup();
    const { onCopy } = await renderViewer({ hasImage: false });

    expect(commandMocks.loadArticleImage).not.toHaveBeenCalled();
    await user.click(screen.getByRole("radio", { name: "源码" }));
    await user.click(screen.getByRole("button", { name: "复制 Markdown" }));
    expect(onCopy).toHaveBeenCalledWith("# 文章标题\n\n正文", "Markdown");
  });

  /** 配图读取失败时不得静默复制缺图内容，重新读取仍只调用读取命令。 */
  it("blocks_copy_until_the_managed_image_can_be_reloaded", async () => {
    commandMocks.loadArticleImage
      .mockRejectedValueOnce(new Error("read failed"))
      .mockResolvedValueOnce(IMAGE);
    const user = userEvent.setup();
    await renderViewer();

    expect(await screen.findByText("配图读取失败")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "复制 Markdown" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "复制 Markdown" }),
      ).toBeEnabled(),
    );
    expect(commandMocks.loadArticleImage).toHaveBeenCalledTimes(2);
  });

  /** 格式和显示模式保留在同一个应用根 store，而新 store 回到默认值。 */
  it("keeps_format_choices_in_the_root_session_store", async () => {
    const user = userEvent.setup();
    const { store } = await renderViewer({ hasImage: false });

    await user.click(screen.getByRole("tab", { name: "HTML" }));
    await user.click(screen.getByRole("radio", { name: "源码" }));
    expect(store.get(articleFormatTabAtom)).toBe("html");
    expect(store.get(articleFormatViewAtom)).toBe("source");

    const nextProgramStore = createStore();
    expect(nextProgramStore.get(articleFormatTabAtom)).toBe("markdown");
    expect(nextProgramStore.get(articleFormatViewAtom)).toBe("rendered");
  });

  /** 用户确认后只同步当前格式，并把文章与连接器乐观锁完整交给后端。 */
  it("syncs_the_selected_format_to_an_enabled_connector_after_confirmation", async () => {
    commandMocks.listConnectorConfigs.mockResolvedValue([
      {
        cliProfile: "team-content",
        defaultFormat: "markdown",
        enabled: true,
        folderToken: "fld_test",
        id: "connector-feishu",
        isDefault: true,
        kind: "feishuDrive",
        name: "飞书内容库",
        revision: 4,
        timeoutMs: 30_000,
      },
    ]);
    commandMocks.syncArticleToConnector.mockResolvedValue({
      connectorId: "connector-feishu",
      connectorRevision: 4,
      fileName: "文章标题-v3.md",
      fileToken: "file_test",
      format: "markdown",
      packageVersion: 3,
      reused: false,
      syncedAtMs: 1,
      taskId: "article-a",
    });
    const user = userEvent.setup();
    await renderViewer({ hasImage: false });

    const syncButton = await screen.findByRole("button", {
      name: "同步 Markdown 到飞书",
    });
    await waitFor(() => expect(syncButton).toBeEnabled());
    await user.click(syncButton);
    expect(screen.getByText(/将《文章标题》的 Markdown/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认上传" }));

    await waitFor(() =>
      expect(commandMocks.syncArticleToConnector).toHaveBeenCalledWith(
        "article-a",
        "connector-feishu",
        "markdown",
        7,
        3,
        4,
      ),
    );
    expect(await screen.findByText(/文章标题-v3\.md/)).toBeInTheDocument();
  });

  /** 确认弹窗冻结用户当时看到的版本；刷新后的新包不能借用旧确认上传。 */
  it("freezes_the_article_and_connector_identity_when_confirmation_opens", async () => {
    commandMocks.listConnectorConfigs.mockResolvedValue([
      {
        cliProfile: "team-content",
        defaultFormat: "markdown",
        enabled: true,
        folderToken: "fld_test",
        id: "connector-feishu",
        isDefault: true,
        kind: "feishuDrive",
        name: "飞书内容库",
        revision: 4,
        timeoutMs: 30_000,
      },
    ]);
    commandMocks.syncArticleToConnector.mockResolvedValue({
      connectorId: "connector-feishu",
      connectorRevision: 4,
      fileName: "文章标题-v3.md",
      fileToken: "file_test",
      format: "markdown",
      packageVersion: 3,
      reused: false,
      syncedAtMs: 1,
      taskId: "article-a",
    });
    const user = userEvent.setup();
    const { rerenderViewer } = await renderViewer({ hasImage: false });

    const syncButton = await screen.findByRole("button", {
      name: "同步 Markdown 到飞书",
    });
    await waitFor(() => expect(syncButton).toBeEnabled());
    await user.click(syncButton);
    rerenderViewer({ packageVersion: 4, rowVersion: 8, title: "刷新后的标题" });

    expect(screen.getByText(/将《文章标题》的 Markdown/)).toBeInTheDocument();
    expect(screen.queryByText(/刷新后的标题/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认上传" }));
    await waitFor(() =>
      expect(commandMocks.syncArticleToConnector).toHaveBeenCalledWith(
        "article-a",
        "connector-feishu",
        "markdown",
        7,
        3,
        4,
      ),
    );
  });

  /** 旧格式仍可预览，但未完成或过期的格式阶段不能发起外部同步。 */
  it("blocks_sync_when_the_current_format_stage_is_not_ready", async () => {
    commandMocks.listConnectorConfigs.mockResolvedValue([
      {
        cliProfile: "team-content",
        defaultFormat: "markdown",
        enabled: true,
        folderToken: "fld_test",
        id: "connector-feishu",
        isDefault: true,
        kind: "feishuDrive",
        name: "飞书内容库",
        revision: 4,
        timeoutMs: 30_000,
      },
    ]);
    await renderViewer({ formatReady: false, hasImage: false });

    const syncButton = await screen.findByRole("button", {
      name: "同步 Markdown 到飞书",
    });
    await waitFor(() => expect(syncButton).toBeDisabled());
    expect(screen.getByText(/格式阶段尚未完成或已过期/)).toBeInTheDocument();
    expect(commandMocks.syncArticleToConnector).not.toHaveBeenCalled();
  });
});
