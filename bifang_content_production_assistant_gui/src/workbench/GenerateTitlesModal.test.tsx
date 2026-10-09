import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  renderWorkbench,
  TestResizeObserver,
} from "./ArticleWorkbenchPageTestSupport";

const mocks = vi.hoisted(() => ({
  generate: vi.fn(),
  providers: vi.fn(),
  newProvider: vi.fn(),
  saveProvider: vi.fn(),
}));
vi.mock("../research/researchCommands", () => ({
  generateArticleTitles: mocks.generate,
  researchError: (reason: unknown) =>
    reason instanceof Error ? reason.message : "失败",
}));
vi.mock("./workbenchCommands", () => ({
  listProviderConfigs: mocks.providers,
  newDeepseekTextProviderDraft: mocks.newProvider,
  saveProviderConfig: mocks.saveProvider,
}));
vi.mock("@tanstack/react-router", () => ({ useNavigate: () => vi.fn() }));
import { GenerateTitlesModal } from "./GenerateTitlesModal";

describe("AI title generation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("ResizeObserver", TestResizeObserver);
    Object.defineProperty(document, "fonts", {
      configurable: true,
      value: { addEventListener: vi.fn(), removeEventListener: vi.fn() },
    });
    window.matchMedia = vi
      .fn()
      .mockImplementation((media: string) => ({
        matches: false,
        media,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));
    mocks.providers.mockResolvedValue([
      {
        id: "text",
        name: "已保存 AI",
        kind: "openAiCompatible",
        enabled: true,
        hasApiKey: true,
        articleModel: "configured-model",
        defaultForArticle: true,
      },
    ]);
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  /** 用户无需预填标题；只有勾选确认后才建立文章任务。 */
  it("generates from a business topic and imports only checked titles", async () => {
    const titles = [
      "客户跟进一",
      "客户跟进二",
      "客户跟进三",
      "客户跟进四",
      "客户跟进五",
    ];
    mocks.generate.mockResolvedValue(titles);
    const submit = vi
      .fn()
      .mockResolvedValue({ createdCount: 4, duplicateCount: 0, taskIds: [] });
    const close = vi.fn();
    await renderWorkbench(
      <GenerateTitlesModal opened onClose={close} onSubmit={submit} />,
    );
    fireEvent.change(screen.getByLabelText(/主题或关键词/), {
      target: { value: "外贸客户跟进" },
    });
    const generate = screen.getByRole("button", { name: "生成标题" });
    await waitFor(() => expect(generate).toBeEnabled());
    fireEvent.click(generate);
    await screen.findByRole("checkbox", { name: titles[1] });
    expect(mocks.generate).toHaveBeenCalledWith(
      {
        platform: "baijiahao",
        topic: "外贸客户跟进",
        requirements: "",
        count: 5,
      },
      "text",
    );
    expect(submit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("checkbox", { name: titles[1] }));
    fireEvent.click(screen.getByRole("button", { name: "加入工作台（4 个）" }));
    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith(
        [titles[0], ...titles.slice(2)].join("\n"),
      ),
    );
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
  });

  /** 没有 API Key 时呈现接入入口，不发起付费请求。 */
  it("blocks generation without credentials and presents setup", async () => {
    mocks.providers.mockResolvedValue([]);
    await renderWorkbench(
      <GenerateTitlesModal opened onClose={vi.fn()} onSubmit={vi.fn()} />,
    );
    await screen.findByRole("button", { name: "接入 AI" });
    fireEvent.change(screen.getByLabelText(/主题或关键词/), {
      target: { value: "收纳" },
    });
    expect(screen.getByRole("button", { name: "生成标题" })).toBeDisabled();
    expect(mocks.generate).not.toHaveBeenCalled();
  });

  /** 服务失败保留输入且不自动重试可能计费的请求。 */
  it("preserves inputs after AI failure without automatic retries", async () => {
    mocks.generate.mockRejectedValue(new Error("额度不足"));
    await renderWorkbench(
      <GenerateTitlesModal opened onClose={vi.fn()} onSubmit={vi.fn()} />,
    );
    fireEvent.change(screen.getByLabelText(/主题或关键词/), {
      target: { value: "产品使用" },
    });
    expect(screen.queryByLabelText("生成几个标题")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("受众和标题角度（选填）")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("使用的 AI 服务")).not.toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "生成标题" })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "生成标题" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("额度不足");
    expect(mocks.generate).toHaveBeenCalledOnce();
    expect(screen.getByLabelText(/主题或关键词/)).toHaveValue("产品使用");
    expect(mocks.generate).toHaveBeenCalledWith(
      { platform: "baijiahao", topic: "产品使用", requirements: "", count: 5 },
      "text",
    );
  });
});
