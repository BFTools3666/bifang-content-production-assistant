import { atom } from "jotai";

/** 页面查询生命周期，只区分分页回退所需的三种稳定状态。 */
export type PageQueryStatus = "loading" | "success" | "error";

/** 页面会话只保存查询控件与分页，不镜像异步结果或 core 数据。 */
export interface PageSessionState<TScope extends string, TQuery> {
  activeTab: TScope;
  query: TQuery;
  page: number;
  pageSize: number;
}

/** 改变查询或活动范围时需要原子提交的页面选择。 */
export interface PageSelection<TScope extends string, TQuery> {
  activeTab: TScope;
  query: TQuery;
}

/** 一次页面结果只暴露分页协调需要的状态和当前页条目数。 */
export interface PageResultSnapshot {
  status: PageQueryStatus;
  itemCount: number;
}

/** 拒绝会造成无效分页状态的零值、小数和负数。 */
function requirePositiveInteger(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`${field} must be a positive integer`);
  }
  return value;
}

/**
 * 创建由应用根 Jotai store 持有的页面会话状态。
 *
 * 调用方必须在页面模块顶层创建一次，不能在 route 组件渲染期间创建；该状态只在
 * 当前应用进程内跨路由保留，不得接入任何浏览器或 Tauri 持久化适配器。
 */
export function createPageSessionState<TScope extends string, TQuery>(
  initialState: PageSessionState<TScope, TQuery>,
) {
  const normalizedInitialState: PageSessionState<TScope, TQuery> = {
    ...initialState,
    page: requirePositiveInteger(initialState.page, "page"),
    pageSize: requirePositiveInteger(initialState.pageSize, "pageSize"),
  };
  const stateAtom = atom(normalizedInitialState);

  const setSelectionAtom = atom(
    null,
    (_get, set, selection: PageSelection<TScope, TQuery>) => {
      set(stateAtom, (current) => ({
        ...current,
        activeTab: selection.activeTab,
        query: selection.query,
        page: 1,
      }));
    },
  );

  const setPaginationAtom = atom(
    null,
    (
      _get,
      set,
      pagination: Pick<PageSessionState<TScope, TQuery>, "page" | "pageSize">,
    ) => {
      const page = requirePositiveInteger(pagination.page, "page");
      const pageSize = requirePositiveInteger(pagination.pageSize, "pageSize");
      set(stateAtom, (current) => ({
        ...current,
        page: pageSize === current.pageSize ? page : 1,
        pageSize,
      }));
    },
  );

  const reconcilePageAfterResultAtom = atom(
    null,
    (get, set, result: PageResultSnapshot): boolean => {
      if (!Number.isInteger(result.itemCount) || result.itemCount < 0) {
        throw new Error("itemCount must be a non-negative integer");
      }
      const current = get(stateAtom);
      if (
        result.status !== "success" ||
        result.itemCount > 0 ||
        current.page === 1
      ) {
        return false;
      }
      set(stateAtom, { ...current, page: 1 });
      return true;
    },
  );

  return Object.freeze({
    stateAtom,
    setSelectionAtom,
    setPaginationAtom,
    reconcilePageAfterResultAtom,
  });
}
