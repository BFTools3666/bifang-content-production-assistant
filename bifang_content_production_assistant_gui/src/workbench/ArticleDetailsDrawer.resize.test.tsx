import { MantineProvider, rem } from "@mantine/core";
import "@testing-library/jest-dom/vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
  type RenderResult,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createInstance } from "i18next";
import type { ComponentProps } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import { ArticleDetailsDrawer } from "./ArticleDetailsDrawer";
import {
  DETAIL_DRAWER_DEFAULT_WIDTH_PX,
  DETAIL_DRAWER_MIN_WIDTH_PX,
  DETAIL_DRAWER_WIDTH_STORAGE_KEY,
} from "./detailDrawerWidth";
import type { ArticleView, StageProgress } from "./workbenchTypes";
import classes from "./workbench.module.css";

const { listConnectorsMock, loadEditorMock, loadImageMock } = vi.hoisted(
  () => ({
    listConnectorsMock: vi.fn(),
    loadEditorMock: vi.fn(),
    loadImageMock: vi.fn(),
  }),
);

vi.mock("./workbenchCommands", () => ({
  listConnectorConfigs: listConnectorsMock,
  loadArticleImage: loadImageMock,
  loadArticleImageEditor: loadEditorMock,
}));

const RESIZE_HANDLE_NAME = "调整文章详情抽屉宽度";
const VIEWPORT_WIDTH_PX = 1_200;

/** 为 Mantine 测试提供不执行真实测量的观察器。 */
class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}

  /** jsdom 不执行真实布局测量。 */
  observe(): void {}

  /** jsdom 允许停止观察但没有持久状态。 */
  unobserve(): void {}
}

/** 创建未开始的阶段状态。 */
function emptyStage(): StageProgress {
  return {
    activeAttemptId: null,
    artifactVersion: null,
    attempts: 0,
    failure: null,
    status: "notStarted",
  };
}

/** 创建足以展示完整详情控件的草稿文章。 */
function draftArticle(): ArticleView {
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
      id: "article-resize",
      attemptHistory: [],
      keywords: ["企业知识库"],
      packageHistory: [],
      packageVersion: 1,
      requirements: "结构清晰",
      reviewStatus: "draft",
      rowVersion: 1,
      stages: {
        article: emptyStage(),
        format: emptyStage(),
        image: emptyStage(),
        summary: emptyStage(),
      },
      title: "企业知识库搭建指南",
      updatedAtMs: 1_789_123_456_000,
    },
    hasImage: false,
    overallStatus: "draft",
  };
}

/** 复用详情组件公开属性，避免测试夹具复制回调签名。 */
type DrawerProps = ComponentProps<typeof ArticleDetailsDrawer>;

/** 控制专项渲染所需的页签和 Mantine Portal 环境。 */
interface RenderDrawerOptions {
  activeTab?: DrawerProps["activeTab"];
  article?: ArticleView;
  mantineEnvironment?: "default" | "test";
  onClose?: () => void;
}

/** 使用真实 Mantine 与 i18n provider 渲染已打开的详情抽屉。 */
async function renderDrawer({
  activeTab = "inputs",
  article = draftArticle(),
  mantineEnvironment = "test",
  onClose = () => undefined,
}: RenderDrawerOptions = {}): Promise<RenderResult> {
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

  return render(
    <I18nextProvider i18n={instance}>
      <QueryClientProvider client={queryClient}>
        <MantineProvider env={mantineEnvironment}>
          <ArticleDetailsDrawer
            activeTab={activeTab}
            actionsLocked={false}
            imageEditorActionsLocked={false}
            article={article}
            onActiveTabChange={() => undefined}
            onApprove={() => Promise.resolve()}
            onClose={onClose}
            onGenerateStage={() => undefined}
            onRegenerate={() => Promise.resolve()}
            onRetry={() => Promise.resolve()}
            onReturnToDraft={() => Promise.resolve()}
            onSave={() => Promise.resolve()}
            onSaveInputs={() => Promise.resolve()}
            onSaveImage={() => Promise.resolve()}
            opened
            runHistory={[]}
            stageActionError={null}
            stageActionTarget={null}
            stageProviderState={{
              article: "ready",
              format: "ready",
              image: "ready",
              summary: "ready",
            }}
          />
        </MantineProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

/** 把 jsdom 视口固定为可重复验证的宽度。 */
function setViewportWidth(width: number): void {
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    value: width,
    writable: true,
  });
}

/** 返回具有完整范围语义的详情抽屉拖拽柄。 */
function getResizeHandle(): HTMLElement {
  return screen.getByRole("separator", { name: RESIZE_HANDLE_NAME });
}

/** 同时验证语义值和 Mantine 实际消费的 Drawer 尺寸变量。 */
function expectDrawerWidth(width: number): void {
  expect(getResizeHandle()).toHaveAttribute("aria-valuenow", String(width));
  expect(
    screen
      .getByTestId("article-details-drawer")
      .style.getPropertyValue("--drawer-size"),
  ).toBe(rem(width));
}

/** 取得 CSS Module 中必须存在的类名，避免测试静默接受缺失样式。 */
function requiredCssClass(name: keyof typeof classes): string {
  const value = classes[name];
  if (typeof value !== "string") {
    throw new Error(`missing CSS module class: ${name}`);
  }
  return value;
}

describe("resizable article details drawer", () => {
  beforeEach(() => {
    listConnectorsMock.mockReset();
    listConnectorsMock.mockResolvedValue([]);
    loadEditorMock.mockReset();
    loadEditorMock.mockResolvedValue({
      canSave: true,
      currentSpec: {
        crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
        flipHorizontal: false,
        flipVertical: false,
        rotationQuarterTurns: 0,
      },
      editRevision: 0,
      imageAttemptId: "attempt-image-1",
      imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
      packageVersion: 1,
      rowVersion: 1,
      saveBlockedReason: null,
      sourceHeight: 675,
      sourceWidth: 1200,
    });
    loadImageMock.mockReset();
    loadImageMock.mockResolvedValue("data:image/png;base64,iVBORw0KGgo=");
    window.localStorage.clear();
    setViewportWidth(VIEWPORT_WIDTH_PX);
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

  afterEach(() => {
    cleanup();
    document
      .querySelectorAll('[data-portal="true"]')
      .forEach((node) => node.remove());
  });

  /** 生产 Mantine 环境必须把 Drawer 挂到 body，并保留防裁切布局契约。 */
  it("portals_the_drawer_and_keeps_all_controls_inside_bounded_layouts", async () => {
    const rendered = await renderDrawer({ mantineEnvironment: "default" });
    const dialog = await screen.findByRole("dialog", {
      name: "文章详情与审核",
    });

    expect(dialog.closest('[data-portal="true"]')).not.toBeNull();
    expect(rendered.container).not.toContainElement(dialog);
    expect(dialog).toHaveClass(requiredCssClass("detailDrawerContent"));
    expect(dialog.querySelector(`.${classes.detailDrawerBody}`)).not.toBeNull();
    expect(
      dialog.querySelector(`.${classes.detailDrawerLayout}`),
    ).not.toBeNull();

    const tabList = within(dialog).getByRole("tablist");
    expect(tabList).toHaveClass(requiredCssClass("detailDrawerTabsList"));
    expect(tabList.parentElement).toHaveClass(
      requiredCssClass("detailDrawerTabs"),
    );
    expect(
      within(dialog)
        .getAllByRole("tab")
        .map((tab) => tab.textContent),
    ).toEqual(["输入", "正文", "简介", "配图", "格式", "历史"]);
    expect(
      within(dialog).getByRole("textbox", { name: "关键词" }),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("textbox", { name: "内容要求" }),
    ).toBeInTheDocument();
  });

  /** 抽屉只承载预览，独立高级编辑弹窗自行处理脏草稿且不能误关抽屉。 */
  it("keeps_advanced_image_editing_in_a_separate_modal", async () => {
    const article = draftArticle();
    article.hasImage = true;
    article.article.artifacts.imageLineage = {
      attemptId: "attempt-image-1",
      model: "image-model",
      packageVersion: 1,
      providerId: "provider-image",
      providerRevision: 1,
      sourceArticleVersion: 1,
      sourceSummaryVersion: 1,
      templateId: "prompt-image",
      templateVersion: 1,
    };
    const onClose = vi.fn();
    const user = userEvent.setup();
    await renderDrawer({ activeTab: "image", article, onClose });
    const details = screen.getByRole("dialog", { name: "文章详情与审核" });
    expect(
      within(details).queryByRole("button", { name: "右转 90°" }),
    ).not.toBeInTheDocument();
    expect(
      await within(details).findByRole("button", { name: "高级编辑配图" }),
    ).toBeInTheDocument();

    await user.click(
      within(details).getByRole("button", { name: "高级编辑配图" }),
    );
    const editor = await screen.findByRole("dialog", {
      name: "《企业知识库搭建指南》配图高级编辑",
    });
    await user.click(
      await within(editor).findByRole("button", { name: "全屏查看" }),
    );
    const fullscreen = screen.getByRole("dialog", {
      name: "《企业知识库搭建指南》配图全屏查看",
    });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(fullscreen).not.toBeInTheDocument());
    expect(editor).toBeInTheDocument();
    expect(details).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();

    await user.click(within(editor).getByRole("button", { name: "右转 90°" }));

    await user.keyboard("{Escape}");
    expect(onClose).not.toHaveBeenCalled();
    const confirmation = screen.getByRole("dialog", {
      name: "放弃未保存的配图编辑？",
    });
    await user.click(
      within(confirmation).getByRole("button", { name: "放弃并关闭" }),
    );
    await waitFor(() => expect(editor).not.toBeInTheDocument());
    expect(details).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();

    await user.click(within(details).getByRole("button", { name: "关闭" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  /** 指针拖拽按右侧 Drawer 方向计算，并在最小值与视口最大值处收口。 */
  it("clamps_pointer_drag_to_minimum_and_viewport_maximum_then_persists", async () => {
    await renderDrawer();
    const handle = getResizeHandle();

    expect(handle).toHaveAttribute("aria-orientation", "vertical");
    expect(handle).toHaveAttribute(
      "aria-valuemin",
      String(DETAIL_DRAWER_MIN_WIDTH_PX),
    );
    expect(handle).toHaveAttribute("aria-valuemax", String(VIEWPORT_WIDTH_PX));
    expect(handle).toHaveProperty("tabIndex", 0);
    expectDrawerWidth(DETAIL_DRAWER_DEFAULT_WIDTH_PX);

    fireEvent.pointerDown(handle, {
      button: 0,
      clientX: 800,
      isPrimary: true,
      pointerId: 1,
      pointerType: "mouse",
    });
    expect(handle).toHaveAttribute("data-resizing", "true");
    fireEvent.pointerMove(window, {
      buttons: 1,
      clientX: 2_000,
      isPrimary: true,
      pointerId: 1,
      pointerType: "mouse",
    });
    expectDrawerWidth(DETAIL_DRAWER_MIN_WIDTH_PX);
    fireEvent.pointerUp(window, {
      button: 0,
      clientX: 2_000,
      isPrimary: true,
      pointerId: 1,
      pointerType: "mouse",
    });
    expect(handle).not.toHaveAttribute("data-resizing");
    expect(window.localStorage.getItem(DETAIL_DRAWER_WIDTH_STORAGE_KEY)).toBe(
      String(DETAIL_DRAWER_MIN_WIDTH_PX),
    );

    fireEvent.pointerDown(handle, {
      button: 0,
      clientX: 800,
      isPrimary: true,
      pointerId: 2,
      pointerType: "mouse",
    });
    fireEvent.pointerMove(window, {
      buttons: 1,
      clientX: -2_000,
      isPrimary: true,
      pointerId: 2,
      pointerType: "mouse",
    });
    expectDrawerWidth(VIEWPORT_WIDTH_PX);
    fireEvent.pointerUp(window, {
      button: 0,
      clientX: -2_000,
      isPrimary: true,
      pointerId: 2,
      pointerType: "mouse",
    });
    expect(window.localStorage.getItem(DETAIL_DRAWER_WIDTH_STORAGE_KEY)).toBe(
      String(VIEWPORT_WIDTH_PX),
    );

    fireEvent.pointerMove(window, {
      buttons: 1,
      clientX: 1_100,
      isPrimary: true,
      pointerId: 2,
      pointerType: "mouse",
    });
    expectDrawerWidth(VIEWPORT_WIDTH_PX);
  });

  /** 已完成拖拽的设备偏好在组件卸载后由新实例恢复。 */
  it("restores_the_persisted_width_after_unmount_and_reopen", async () => {
    const firstRender = await renderDrawer();
    const handle = getResizeHandle();
    const restoredWidth = 760;

    fireEvent.pointerDown(handle, {
      button: 0,
      clientX: 800,
      isPrimary: true,
      pointerId: 3,
      pointerType: "mouse",
    });
    fireEvent.pointerMove(window, {
      buttons: 1,
      clientX: 660,
      isPrimary: true,
      pointerId: 3,
      pointerType: "mouse",
    });
    fireEvent.pointerUp(window, {
      button: 0,
      clientX: 660,
      isPrimary: true,
      pointerId: 3,
      pointerType: "mouse",
    });
    expect(window.localStorage.getItem(DETAIL_DRAWER_WIDTH_STORAGE_KEY)).toBe(
      String(restoredWidth),
    );

    firstRender.unmount();
    await renderDrawer();

    await waitFor(() => expectDrawerWidth(restoredWidth));
  });

  /** 键盘方向键按固定步长调整，Home 与 End 精确到达两端并立即保存。 */
  it("supports_keyboard_steps_and_boundary_shortcuts", async () => {
    await renderDrawer();
    const handle = getResizeHandle();
    handle.focus();
    expect(handle).toHaveFocus();

    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expectDrawerWidth(DETAIL_DRAWER_DEFAULT_WIDTH_PX + 24);
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    expectDrawerWidth(DETAIL_DRAWER_DEFAULT_WIDTH_PX);
    fireEvent.keyDown(handle, { key: "Home" });
    expectDrawerWidth(DETAIL_DRAWER_MIN_WIDTH_PX);
    fireEvent.keyDown(handle, { key: "End" });
    expectDrawerWidth(VIEWPORT_WIDTH_PX);
    expect(window.localStorage.getItem(DETAIL_DRAWER_WIDTH_STORAGE_KEY)).toBe(
      String(VIEWPORT_WIDTH_PX),
    );
  });

  /** 非数值或低于产品下限的保存值不能污染下一次 Drawer 初始宽度。 */
  it("falls_back_from_invalid_or_below_minimum_storage_values", async () => {
    window.localStorage.setItem(
      DETAIL_DRAWER_WIDTH_STORAGE_KEY,
      "not-a-number",
    );
    const firstRender = await renderDrawer();
    expectDrawerWidth(DETAIL_DRAWER_DEFAULT_WIDTH_PX);

    firstRender.unmount();
    window.localStorage.setItem(
      DETAIL_DRAWER_WIDTH_STORAGE_KEY,
      String(DETAIL_DRAWER_MIN_WIDTH_PX - 1),
    );
    await renderDrawer();
    expectDrawerWidth(DETAIL_DRAWER_DEFAULT_WIDTH_PX);
  });

  /** 视口临时变窄只约束显示宽度，恢复后仍采用用户保存的首选宽度。 */
  it("temporarily_clamps_to_a_narrow_viewport_then_restores_preference", async () => {
    const preferredWidth = 900;
    window.localStorage.setItem(
      DETAIL_DRAWER_WIDTH_STORAGE_KEY,
      String(preferredWidth),
    );
    await renderDrawer();
    expectDrawerWidth(preferredWidth);

    setViewportWidth(700);
    fireEvent.resize(window);
    expectDrawerWidth(700);
    expect(getResizeHandle()).toHaveAttribute("aria-valuemax", "700");
    expect(window.localStorage.getItem(DETAIL_DRAWER_WIDTH_STORAGE_KEY)).toBe(
      String(preferredWidth),
    );

    setViewportWidth(VIEWPORT_WIDTH_PX);
    fireEvent.resize(window);
    expectDrawerWidth(preferredWidth);
  });
});
