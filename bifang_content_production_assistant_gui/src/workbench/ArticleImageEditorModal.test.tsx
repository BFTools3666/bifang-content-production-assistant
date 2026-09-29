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
import userEvent from "@testing-library/user-event";
import { createInstance } from "i18next";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import type { ArticleImageEditorView } from "./workbenchTypes";

const { loadEditorMock, loadImageMock } = vi.hoisted(() => ({
  loadEditorMock: vi.fn(),
  loadImageMock: vi.fn(),
}));

vi.mock("./workbenchCommands", () => ({
  loadArticleImage: loadImageMock,
  loadArticleImageEditor: loadEditorMock,
}));

import {
  ArticleImageEditorModal,
  type ArticleImageEditorModalProps,
} from "./ArticleImageEditorModal";

/** 创建携带完整保存锁的图片编辑器视图。 */
function editorView(): ArticleImageEditorView {
  return {
    canSave: true,
    currentSpec: {
      crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
      flipHorizontal: false,
      flipVertical: false,
      rotationQuarterTurns: 0,
    },
    editRevision: 0,
    imageAttemptId: "attempt-image-1",
    imageDataUrl: "data:image/png;base64,b3JpZ2luYWw=",
    packageVersion: 2,
    rowVersion: 8,
    saveBlockedReason: null,
    sourceHeight: 675,
    sourceWidth: 1200,
  };
}

/** 在真实 Mantine 与双语资源中渲染抽屉图片入口。 */
async function renderImageModal(
  onSave: ArticleImageEditorModalProps["onSave"] = vi.fn(() =>
    Promise.resolve(),
  ),
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
  render(
    <I18nextProvider i18n={instance}>
      <MantineProvider env="test">
        <ArticleImageEditorModal
          actionLocked={false}
          active
          hasImage
          manualLineage={null}
          onSave={onSave}
          refreshKey="8:2:0:attempt-image-1"
          taskId="article-1"
          title="SEO 指南"
        />
      </MantineProvider>
    </I18nextProvider>,
  );
}

/** 获取高级图片编辑器遮罩，缺失时给出可诊断的测试错误。 */
function imageEditorOverlay(): HTMLElement {
  const overlay = document.getElementById("image-editor-modal-overlay");
  if (overlay === null) {
    throw new Error("image editor modal overlay was not rendered");
  }
  return overlay;
}

describe("article image editor modal", () => {
  beforeEach(() => {
    loadImageMock.mockReset();
    loadImageMock.mockResolvedValue("data:image/png;base64,Y3VycmVudA==");
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

  /** 抽屉只展示可点击预览，高级控件必须在显式打开的独立弹窗内出现。 */
  it("keeps_advanced_controls_out_of_the_drawer_until_the_modal_opens", async () => {
    const user = userEvent.setup();
    await renderImageModal();

    const trigger = await screen.findByRole("button", {
      name: "打开《SEO 指南》配图高级编辑",
    });
    expect(loadImageMock).toHaveBeenCalledWith("article-1");
    expect(loadEditorMock).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("button", { name: "右转 90°" }),
    ).not.toBeInTheDocument();

    trigger.focus();
    await user.click(trigger);
    const dialog = await screen.findByRole("dialog", {
      name: "《SEO 指南》配图高级编辑",
    });
    expect(loadEditorMock).toHaveBeenCalledOnce();
    expect(
      await within(dialog).findByRole("button", { name: "右转 90°" }),
    ).toBeInTheDocument();
    expect(dialog.contains(document.activeElement)).toBe(true);

    const closeButton = within(dialog).getByRole("button", {
      name: "关闭配图高级编辑",
    });
    await user.click(closeButton);
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
    expect(
      screen.queryByRole("dialog", { name: "放弃未保存的配图编辑？" }),
    ).not.toBeInTheDocument();
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  /** 脏草稿的关闭按钮、遮罩和 Escape 都只打开确认，明确放弃才关闭编辑弹窗。 */
  it("confirms_dirty_drafts_for_every_modal_close_path", async () => {
    const user = userEvent.setup();
    await renderImageModal();
    const trigger = await screen.findByRole("button", {
      name: "打开《SEO 指南》配图高级编辑",
    });
    await user.click(trigger);
    const dialog = await screen.findByRole("dialog", {
      name: "《SEO 指南》配图高级编辑",
    });
    await user.click(
      await within(dialog).findByRole("button", { name: "水平翻转" }),
    );

    const closeButton = within(dialog).getByRole("button", {
      name: "关闭配图高级编辑",
    });
    await user.click(closeButton);
    let confirmation = screen.getByRole("dialog", {
      name: "放弃未保存的配图编辑？",
    });
    expect(dialog).toBeInTheDocument();
    await user.click(
      within(confirmation).getByRole("button", { name: "继续编辑" }),
    );
    await waitFor(() => expect(closeButton).toHaveFocus());

    fireEvent.click(imageEditorOverlay());
    confirmation = screen.getByRole("dialog", {
      name: "放弃未保存的配图编辑？",
    });
    expect(dialog).toBeInTheDocument();
    await user.click(
      within(confirmation).getByRole("button", { name: "继续编辑" }),
    );

    await user.keyboard("{Escape}");
    confirmation = screen.getByRole("dialog", {
      name: "放弃未保存的配图编辑？",
    });
    expect(dialog).toBeInTheDocument();
    await user.click(
      within(confirmation).getByRole("button", { name: "放弃并关闭" }),
    );

    await waitFor(() => expect(dialog).not.toBeInTheDocument());
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(loadEditorMock).toHaveBeenCalledOnce();
  });

  /** 原生保存无法取消，保存完成前必须锁住所有关闭入口。 */
  it("keeps_the_editor_open_while_a_save_is_pending", async () => {
    const user = userEvent.setup();
    let finishSave: (() => void) | undefined;
    const onSave = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishSave = resolve;
        }),
    );
    await renderImageModal(onSave);
    await user.click(
      await screen.findByRole("button", {
        name: "打开《SEO 指南》配图高级编辑",
      }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "《SEO 指南》配图高级编辑",
    });
    await user.click(
      await within(dialog).findByRole("button", { name: "水平翻转" }),
    );
    await user.click(
      within(dialog).getByRole("button", { name: "保存新配图版本" }),
    );
    await waitFor(() => expect(onSave).toHaveBeenCalledOnce());

    const closeButton = within(dialog).getByRole("button", {
      name: "关闭配图高级编辑",
    });
    expect(closeButton).toBeDisabled();
    await user.keyboard("{Escape}");
    fireEvent.click(imageEditorOverlay());
    expect(dialog).toBeInTheDocument();
    expect(
      screen.queryByRole("dialog", { name: "放弃未保存的配图编辑？" }),
    ).not.toBeInTheDocument();
    expect(onSave).toHaveBeenCalledOnce();

    await act(async () => finishSave?.());
    await waitFor(() => expect(closeButton).toBeEnabled());
    expect(
      within(dialog).getByText("配图编辑已保存为新的内容包版本。"),
    ).toBeInTheDocument();
    await user.click(closeButton);
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
  });
});
