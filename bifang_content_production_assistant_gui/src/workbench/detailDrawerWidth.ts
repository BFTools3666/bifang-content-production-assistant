import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEventHandler,
  type PointerEventHandler,
} from "react";

export const DETAIL_DRAWER_WIDTH_STORAGE_KEY =
  "bifang-content-production-assistant:article-details-drawer-width";
export const DETAIL_DRAWER_DEFAULT_WIDTH_PX = 620;
export const DETAIL_DRAWER_MIN_WIDTH_PX = 480;
const DETAIL_DRAWER_KEYBOARD_STEP_PX = 24;

/** 保存一次指针拖拽的起点，保证移动量始终相对按下位置计算。 */
interface DrawerResizeStart {
  pointerId: number;
  pointerX: number;
  target: HTMLDivElement;
  width: number;
}

/** 向详情抽屉暴露当前宽度和语义化拖拽手柄事件。 */
export interface DetailDrawerWidthController {
  maximumWidth: number;
  minimumWidth: number;
  onResizeKeyDown: KeyboardEventHandler<HTMLDivElement>;
  onResizePointerDown: PointerEventHandler<HTMLDivElement>;
  resizing: boolean;
  width: number;
}

/** 返回当前 WebView 可用宽度；非浏览器环境使用既有抽屉默认值。 */
function currentViewportWidth(): number {
  if (typeof window === "undefined") return DETAIL_DRAWER_DEFAULT_WIDTH_PX;
  return Math.max(1, Math.floor(window.innerWidth));
}

/** 把抽屉宽度约束到产品最小值和当前 WebView 之间。 */
export function constrainDetailDrawerWidth(
  width: number,
  viewportWidth: number,
): number {
  const maximum =
    Number.isFinite(viewportWidth) && viewportWidth > 0
      ? Math.max(1, Math.floor(viewportWidth))
      : DETAIL_DRAWER_DEFAULT_WIDTH_PX;
  const minimum = Math.min(DETAIL_DRAWER_MIN_WIDTH_PX, maximum);
  const candidate = Number.isFinite(width)
    ? Math.round(width)
    : DETAIL_DRAWER_DEFAULT_WIDTH_PX;
  return Math.min(maximum, Math.max(minimum, candidate));
}

/** 读取用户上次确认的设备级抽屉宽度，非法或不可读时回到默认值。 */
export function readDetailDrawerWidthPreference(): number {
  if (typeof window === "undefined") return DETAIL_DRAWER_DEFAULT_WIDTH_PX;
  try {
    const stored = window.localStorage.getItem(DETAIL_DRAWER_WIDTH_STORAGE_KEY);
    if (stored === null) return DETAIL_DRAWER_DEFAULT_WIDTH_PX;
    const parsed = Number(stored);
    return Number.isFinite(parsed) && parsed >= DETAIL_DRAWER_MIN_WIDTH_PX
      ? Math.round(parsed)
      : DETAIL_DRAWER_DEFAULT_WIDTH_PX;
  } catch {
    return DETAIL_DRAWER_DEFAULT_WIDTH_PX;
  }
}

/** 持久保存用户确认的抽屉宽度；设备存储不可用时保留当前会话状态。 */
export function persistDetailDrawerWidthPreference(width: number): void {
  if (
    typeof window === "undefined" ||
    !Number.isFinite(width) ||
    width < DETAIL_DRAWER_MIN_WIDTH_PX
  ) {
    return;
  }
  try {
    window.localStorage.setItem(
      DETAIL_DRAWER_WIDTH_STORAGE_KEY,
      String(Math.round(width)),
    );
  } catch {
    // 设备级存储不可用时，React 状态仍保留本次运行的宽度。
  }
}

/** 管理右侧详情抽屉的拖拽、键盘缩放、视口约束和设备级记忆。 */
export function useDetailDrawerWidth(): DetailDrawerWidthController {
  const preferredWidthRef = useRef<number | null>(null);
  if (preferredWidthRef.current === null) {
    preferredWidthRef.current = readDetailDrawerWidthPreference();
  }
  const [maximumWidth, setMaximumWidth] = useState(currentViewportWidth);
  const [width, setWidth] = useState(() =>
    constrainDetailDrawerWidth(
      preferredWidthRef.current ?? DETAIL_DRAWER_DEFAULT_WIDTH_PX,
      currentViewportWidth(),
    ),
  );
  const widthRef = useRef(width);
  widthRef.current = width;
  const resizeStartRef = useRef<DrawerResizeStart | null>(null);
  const [resizing, setResizing] = useState(false);

  const applyPreferredWidth = useCallback(
    (candidate: number, persist: boolean): void => {
      const viewportWidth = currentViewportWidth();
      const nextWidth = constrainDetailDrawerWidth(candidate, viewportWidth);
      preferredWidthRef.current = nextWidth;
      widthRef.current = nextWidth;
      setMaximumWidth(viewportWidth);
      setWidth(nextWidth);
      if (persist) persistDetailDrawerWidthPreference(nextWidth);
    },
    [],
  );

  useEffect(() => {
    const handleViewportResize = (): void => {
      const viewportWidth = currentViewportWidth();
      const nextWidth = constrainDetailDrawerWidth(
        preferredWidthRef.current ?? DETAIL_DRAWER_DEFAULT_WIDTH_PX,
        viewportWidth,
      );
      widthRef.current = nextWidth;
      setMaximumWidth(viewportWidth);
      setWidth(nextWidth);
    };
    window.addEventListener("resize", handleViewportResize);
    return () => window.removeEventListener("resize", handleViewportResize);
  }, []);

  const finishPointerResize = useCallback((event: PointerEvent): void => {
    const start = resizeStartRef.current;
    if (!start || event.pointerId !== start.pointerId) return;
    resizeStartRef.current = null;
    try {
      start.target.releasePointerCapture?.(start.pointerId);
    } catch {
      // WebView 已经自动释放捕获时不影响宽度提交。
    }
    setResizing(false);
    persistDetailDrawerWidthPreference(widthRef.current);
  }, []);

  useEffect(() => {
    if (!resizing) return;
    const handlePointerMove = (event: PointerEvent): void => {
      const start = resizeStartRef.current;
      if (!start || event.pointerId !== start.pointerId) return;
      applyPreferredWidth(start.width + start.pointerX - event.clientX, false);
    };
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", finishPointerResize);
    window.addEventListener("pointercancel", finishPointerResize);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", finishPointerResize);
      window.removeEventListener("pointercancel", finishPointerResize);
    };
  }, [applyPreferredWidth, finishPointerResize, resizing]);

  const onResizePointerDown = useCallback<PointerEventHandler<HTMLDivElement>>(
    (event) => {
      if (event.button !== 0 || event.isPrimary === false) return;
      event.preventDefault();
      event.currentTarget.focus();
      resizeStartRef.current = {
        pointerId: event.pointerId,
        pointerX: event.clientX,
        target: event.currentTarget,
        width: widthRef.current,
      };
      try {
        event.currentTarget.setPointerCapture?.(event.pointerId);
      } catch {
        // 不支持指针捕获的 WebView 仍由 window 级监听完成拖拽。
      }
      setResizing(true);
    },
    [],
  );

  const onResizeKeyDown = useCallback<KeyboardEventHandler<HTMLDivElement>>(
    (event) => {
      let nextWidth: number | null = null;
      if (event.key === "ArrowLeft") {
        nextWidth = width + DETAIL_DRAWER_KEYBOARD_STEP_PX;
      } else if (event.key === "ArrowRight") {
        nextWidth = width - DETAIL_DRAWER_KEYBOARD_STEP_PX;
      } else if (event.key === "Home") {
        nextWidth = DETAIL_DRAWER_MIN_WIDTH_PX;
      } else if (event.key === "End") {
        nextWidth = maximumWidth;
      }
      if (nextWidth === null) return;
      event.preventDefault();
      applyPreferredWidth(nextWidth, true);
    },
    [applyPreferredWidth, maximumWidth, width],
  );

  return {
    maximumWidth,
    minimumWidth: Math.min(DETAIL_DRAWER_MIN_WIDTH_PX, maximumWidth),
    onResizeKeyDown,
    onResizePointerDown,
    resizing,
    width,
  };
}
