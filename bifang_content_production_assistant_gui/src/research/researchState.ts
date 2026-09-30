import { atom } from "jotai";
import { useQuery } from "@tanstack/react-query";
import { getResearchLibrary } from "./researchCommands";
import type { ContentPlatform, GenerationInput } from "./researchTypes";

export const IS_XHS_EDITION = import.meta.env.VITE_BIFANG_EDITION === "xhs";
export const DEFAULT_PLATFORM: ContentPlatform = IS_XHS_EDITION ? "xiaohongshu" : "csdn";
export const RESEARCH_QUERY_KEY = ["research-library"] as const;

/** 当前进程的查询、筛选和勾选状态；业务资产由后端拥有。 */
export interface BenchmarkSession {
  platform: ContentPlatform;
  keyword: string;
  query: string;
  authorUrl: string;
  minimumViews: number;
  minimumFavorites: number;
  minimumLikes: number;
  sort: keyof import("./researchTypes").WorkMetrics | "recent";
  selected: string[];
}

export const benchmarkSessionAtom = atom<BenchmarkSession>({ platform: DEFAULT_PLATFORM, keyword: "", query: "", authorUrl: "", minimumViews: 0, minimumFavorites: 0, minimumLikes: 0, sort: IS_XHS_EDITION ? "likes" : "views", selected: [] });
/** 仅保存当前草稿与服务的选择ID；完整业务对象由Query拥有。 */
export const writerDraftIdAtom = atom<string | null>(null);
export const writerProviderIdAtom = atom<string | null>(null);
export const historyDraftIdAtom = atom<string | null>(null);
/** 尚未提交的标题和正文是表单输入；路由切换保留，保存成功后删除。 */
export const draftEditsAtom = atom<Record<string, { title: string; body: string }>>({});
export const composerSessionAtom = atom<GenerationInput>({ platform: DEFAULT_PLATFORM, keyword: "", sourceIds: [], styleId: null, userMaterial: "", promptOverride: "" });

/** 从统一Query缓存读取当前业务资产，路由切换不重复建立数据库镜像。 */
export function useResearchLibrary() {
  return useQuery({ queryKey: RESEARCH_QUERY_KEY, queryFn: getResearchLibrary, staleTime: 5000, retry: false });
}

/** 返回当前发行版允许的生成和采集平台。 */
export function platformOptions() {
  return IS_XHS_EDITION ? [{ value: "xiaohongshu", label: "小红书" }] : [{ value: "csdn", label: "CSDN" }, { value: "baijiahao", label: "百家号" }];
}

/** 用户界面显示平台名称，不显示内部协议标识。 */
export function platformLabel(platform: ContentPlatform): string {
  return { csdn: "CSDN", baijiahao: "百家号", xiaohongshu: "小红书" }[platform];
}

/** 将缺失指标和真实零值清楚地区分。 */
export function metricText(value: number | null): string {
  return value === null ? "未提供" : value.toLocaleString();
}

/** 四步写作的视图进度由进程内会话保存，业务资产仍归原有 core。 */
export const writerStepAtom = atom<0 | 1 | 2 | 3>(0);
/** 本次写作方式只决定展示；输入和风格由 core 最终校验。 */
export const writerModeAtom = atom<"topic" | "reference" | "style">("topic");
