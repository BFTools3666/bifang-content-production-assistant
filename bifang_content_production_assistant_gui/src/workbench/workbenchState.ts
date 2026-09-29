import { atom } from "jotai";

import { createPageSessionState } from "../brand-support/pageSessionState";
import type { OverallStatus } from "./workbenchTypes";
import type { PromptTemplateKind } from "./workbenchTypes";

/** 工作台允许用户选择的固定分页数量。 */
export const WORKBENCH_PAGE_SIZE_OPTIONS = [20, 50, 100] as const;

/** Grid 支持的排序方式。 */
export type WorkbenchSort = "created" | "updated" | "title" | "status";

/** 工作台查询控件值，不包含 TanStack Query 结果。 */
export interface WorkbenchPageQuery {
  search: string;
  sort: WorkbenchSort;
}

/** Grid 状态范围。 */
export type WorkbenchStatusScope = OverallStatus | "all";

/** 工作台页面会话在应用根 store 中按进程保存。 */
export const workbenchPageSession = createPageSessionState<
  WorkbenchStatusScope,
  WorkbenchPageQuery
>({
  activeTab: "all",
  page: 1,
  pageSize: 20,
  query: {
    search: "",
    sort: "created",
  },
});

/** 工作台标题搜索在本次应用进程中跨路由保留。 */
export const workbenchSearchAtom = atom(
  (get) => get(workbenchPageSession.stateAtom).query.search,
  (get, set, search: string) => {
    const current = get(workbenchPageSession.stateAtom);
    set(workbenchPageSession.setSelectionAtom, {
      activeTab: current.activeTab,
      query: { ...current.query, search },
    });
  },
);

/** Grid 状态筛选在本次应用进程中跨路由保留。 */
export const workbenchStatusFilterAtom = atom(
  (get) => get(workbenchPageSession.stateAtom).activeTab,
  (get, set, activeTab: WorkbenchStatusScope) => {
    const current = get(workbenchPageSession.stateAtom);
    set(workbenchPageSession.setSelectionAtom, {
      activeTab,
      query: current.query,
    });
  },
);

/** Grid 排序方式在本次应用进程中跨路由保留。 */
export const workbenchSortAtom = atom(
  (get) => get(workbenchPageSession.stateAtom).query.sort,
  (get, set, sort: WorkbenchSort) => {
    const current = get(workbenchPageSession.stateAtom);
    set(workbenchPageSession.setSelectionAtom, {
      activeTab: current.activeTab,
      query: { ...current.query, sort },
    });
  },
);

/** 当前会话明确选中的文章 ID；后端权威任务数据不在这里镜像。 */
export const selectedArticleIdsAtom = atom<Set<string>>(new Set<string>());

/** 当前详情抽屉中的文章 ID。 */
export const inspectedArticleIdAtom = atom<string | null>(null);

/** 详情抽屉活动标签在本次应用进程中跨路由保留。 */
export const articleDetailsTabAtom = atom<
  "inputs" | "article" | "summary" | "image" | "formats" | "history"
>("inputs");

/** 格式页当前产物选项卡在本次应用进程中跨路由保留。 */
export type ArticleFormatTab = "markdown" | "html";
export const articleFormatTabAtom = atom<ArticleFormatTab>("markdown");

/** 格式页当前显示模式在本次应用进程中跨路由保留。 */
export type ArticleFormatView = "source" | "rendered";
export const articleFormatViewAtom = atom<ArticleFormatView>("rendered");

/** Prompt 管理页活动模板在本次应用进程中跨路由保留。 */
export const activePromptKindAtom = atom<PromptTemplateKind>("article");

/** AI 服务页当前配置步骤在本次应用进程中跨路由保留。 */
export const activeProviderStageAtom = atom<PromptTemplateKind>("article");
