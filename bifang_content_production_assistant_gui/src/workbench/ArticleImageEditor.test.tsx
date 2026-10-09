import { MantineProvider } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createInstance } from "i18next";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import type { ArticleImageEditorView, ImageEditSpec } from "./workbenchTypes";

const { loadEditorMock } = vi.hoisted(() => ({ loadEditorMock: vi.fn() }));

vi.mock("./workbenchCommands", () => ({
  loadArticleImageEditor: loadEditorMock,
}));

import {
  ArticleImageEditor,
  type ImageEditorSaveRequest,
} from "./ArticleImageEditor";

const ORIGINAL_SPEC: ImageEditSpec = {
  crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
  flipHorizontal: false,
  flipVertical: false,
  rotationQuarterTurns: 0,
};

/** 创建不含路径且携带完整保存锁的只读编辑器视图。 */
function editorView(
  currentSpec: ImageEditSpec = ORIGINAL_SPEC,
): ArticleImageEditorView {
  return {
    canSave: true,
    currentSpec,
    editRevision: currentSpec === ORIGINAL_SPEC ? 0 : 1,
    imageAttemptId: "attempt-image-1",
    imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
    packageVersion: currentSpec === ORIGINAL_SPEC ? 2 : 3,
    rowVersion: currentSpec === ORIGINAL_SPEC ? 8 : 10,
    saveBlockedReason: null,
    sourceHeight: 675,
    sourceWidth: 1200,
  };
}

/** 在真实国际化和 Mantine 容器中渲染编辑器。 */
async function renderEditor({
  onDirtyChange = vi.fn(),
  onSave = vi.fn(() => Promise.resolve()),
}: {
  onDirtyChange?: (dirty: boolean) => void;
  onSave?: (request: ImageEditorSaveRequest) => Promise<void>;
} = {}): Promise<{ rerenderWithRefreshKey: (refreshKey: string) => void }> {
  const instance = createInstance();
  await instance.init({
    fallbackLng: "en-US",
    lng: "zh-CN",
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  const editor = (refreshKey: string) => (
    <I18nextProvider i18n={instance}>
      <MantineProvider env="test">
        <ArticleImageEditor
          actionLocked={false}
          active
          hasImage
          manualLineage={null}
          onDirtyChange={onDirtyChange}
          onSave={onSave}
          refreshKey={refreshKey}
          taskId="article-1"
          title="SEO 指南"
        />
      </MantineProvider>
    </I18nextProvider>
  );
  const rendered = render(editor("8:2:0:attempt-image-1"));
  return {
    rerenderWithRefreshKey: (refreshKey) =>
      rendered.rerender(editor(refreshKey)),
  };
}

describe("article image editor", () => {
  beforeEach(() => {
    loadEditorMock.mockReset();
    loadEditorMock.mockResolvedValue(editorView());
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
      value: class {
        /** 测试环境没有真实观察资源。 */
        disconnect(): void {}
        /** jsdom 不执行真实布局测量。 */
        observe(): void {}
        /** jsdom 不执行真实布局测量。 */
        unobserve(): void {}
      },
    });
  });

  afterEach(() => cleanup());

  /** 缩放、适应、全屏和重置只改变内存查看状态，不触发保存。 */
  it("keeps_all_view_operations_read_only", async () => {
    const onSave = vi.fn(() => Promise.resolve());
    const onDirtyChange = vi.fn();
    const user = userEvent.setup();
    await renderEditor({ onDirtyChange, onSave });
    await screen.findByRole("img", { name: "《SEO 指南》的 AI 生成配图" });

    await user.click(screen.getByRole("button", { name: "放大" }));
    expect(screen.getByText("125%")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "100%" }));
    await user.click(screen.getByRole("button", { name: "适应窗口" }));
    await user.click(screen.getByRole("button", { name: "重置查看" }));
    await user.click(screen.getByRole("button", { name: "全屏查看" }));
    const fullscreen = screen.getByRole("dialog", {
      name: "《SEO 指南》配图全屏查看",
    });
    expect(screen.getByTestId("image-editor-fullscreen")).toBeInTheDocument();
    await user.click(
      within(fullscreen).getByRole("button", { name: "重置查看" }),
    );
    await user.click(within(fullscreen).getByRole("button", { name: "关闭" }));

    expect(loadEditorMock).toHaveBeenCalledOnce();
    expect(loadEditorMock).toHaveBeenCalledWith("article-1");
    expect(onSave).not.toHaveBeenCalled();
    expect(onDirtyChange).not.toHaveBeenCalledWith(true);
  });

  /** 离散编辑支持撤销重做，并以一次点击携带全部锁和规范保存。 */
  it("saves_one_normalized_edit_with_undo_and_redo", async () => {
    let resolveSave: (() => void) | undefined;
    const onSave = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const editedSpec: ImageEditSpec = {
      crop: { height: 1_000_000, width: 562_500, x: 218_750, y: 0 },
      flipHorizontal: true,
      flipVertical: false,
      rotationQuarterTurns: 1,
    };
    loadEditorMock
      .mockResolvedValueOnce(editorView())
      .mockResolvedValueOnce(editorView(editedSpec));
    const onDirtyChange = vi.fn();
    const user = userEvent.setup();
    await renderEditor({ onDirtyChange, onSave });
    await screen.findByRole("img", { name: "《SEO 指南》的 AI 生成配图" });

    await user.click(screen.getByRole("button", { name: "1:1" }));
    await user.click(screen.getByRole("button", { name: "右转 90°" }));
    await user.click(screen.getByRole("button", { name: "水平翻转" }));
    await user.click(screen.getByRole("button", { name: "撤销" }));
    expect(screen.getByRole("button", { name: "重做" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "重做" }));
    expect(onDirtyChange).toHaveBeenCalledWith(true);

    const saveButton = screen.getByRole("button", { name: "保存新配图版本" });
    fireEvent.click(saveButton);
    fireEvent.click(saveButton);
    await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
    expect(onSave).toHaveBeenCalledWith({
      expectedEditRevision: 0,
      expectedImageAttemptId: "attempt-image-1",
      expectedPackageVersion: 2,
      expectedRowVersion: 8,
      spec: editedSpec,
      taskId: "article-1",
    });

    resolveSave?.();
    await screen.findByText("配图编辑已保存为新的文章版本。");
    await waitFor(() => expect(loadEditorMock).toHaveBeenCalledTimes(2));
    expect(onDirtyChange).toHaveBeenLastCalledWith(false);
  });

  /** 保存冲突保留当前草稿和可重试按钮，不会静默恢复旧规范。 */
  it("retains_the_draft_after_a_save_conflict", async () => {
    const onSave = vi.fn(() =>
      Promise.reject({
        code: "IMAGE_EDIT_CONFLICT",
        message: "must not be rendered directly",
      }),
    );
    const user = userEvent.setup();
    await renderEditor({ onSave });
    await screen.findByRole("img", { name: "《SEO 指南》的 AI 生成配图" });

    await user.click(screen.getByRole("button", { name: "垂直翻转" }));
    await user.click(screen.getByRole("button", { name: "保存新配图版本" }));

    expect(
      await screen.findByText("文章或配图已变化，请重新载入编辑器后再保存。"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "保存新配图版本" }),
    ).toBeEnabled();
    expect(
      screen.getByRole("button", { name: "重新载入并放弃当前草稿" }),
    ).toBeEnabled();
    expect(loadEditorMock).toHaveBeenCalledOnce();
  });

  /** 排队或运行中的文章仍显示原图和查看工具，但明确禁用保存。 */
  it("keeps_busy_article_images_viewable_and_disables_save", async () => {
    loadEditorMock.mockResolvedValue({
      ...editorView(),
      canSave: false,
      saveBlockedReason: {
        code: "IMAGE_EDIT_ARTICLE_BUSY",
        message: "must be localized by code",
      },
    });
    await renderEditor();

    expect(
      await screen.findByRole("img", { name: "《SEO 指南》的 AI 生成配图" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("当前文章正在排队或运行，暂时不能保存配图编辑。"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "保存新配图版本" }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "全屏查看" })).toBeEnabled();
  });

  /** 后台状态变化会禁用保存并保留尚未提交的本地草稿。 */
  it("preserves_a_dirty_draft_when_a_refresh_makes_the_article_busy", async () => {
    loadEditorMock.mockResolvedValueOnce(editorView()).mockResolvedValueOnce({
      ...editorView(),
      canSave: false,
      rowVersion: 9,
      saveBlockedReason: {
        code: "IMAGE_EDIT_ARTICLE_BUSY",
        message: "must be localized by code",
      },
    });
    const onDirtyChange = vi.fn();
    const user = userEvent.setup();
    const { rerenderWithRefreshKey } = await renderEditor({ onDirtyChange });
    await screen.findByRole("img", { name: "《SEO 指南》的 AI 生成配图" });
    await user.click(screen.getByRole("button", { name: "垂直翻转" }));

    rerenderWithRefreshKey("9:2:0:attempt-image-1");

    expect(
      await screen.findByText("当前文章正在排队或运行，暂时不能保存配图编辑。"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "保存新配图版本" }),
    ).toBeDisabled();
    expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    expect(
      screen.getByRole("img", { name: "《SEO 指南》的 AI 生成配图" }),
    ).toBeInTheDocument();
  });
});
