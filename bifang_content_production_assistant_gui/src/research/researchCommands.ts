import { invoke } from "@tauri-apps/api/core";
import type { CollectionStatus, ContentPlatform, GenerationInput, ResearchData, ResearchDraft, ResearchPrompts, StyleProfile } from "./researchTypes";

/** 读取只包含公开业务资产的持久工作区。 */
export async function getResearchLibrary(): Promise<ResearchData> {
  return invoke<ResearchData>("get_research_library");
}

/** 读取采集窗口的轻量状态，不解密任何凭据。 */
export async function getCollectionStatus(): Promise<CollectionStatus> {
  return invoke<CollectionStatus>("get_collection_status");
}

/** 在可见窗口搜索关键词或打开当前平台的公开主页、作品。 */
export async function startCollection(platform: ContentPlatform, keyword: string, pageUrl: string | null = null, pageUrls: string[] | null = null): Promise<void> {
  return invoke<void>("start_benchmark_collection", { platform, keyword, pageUrl, pageUrls });
}

/** 使用原有已保存AI服务生成标题与正文，完成后保存真实草稿。 */
export async function generateContent(input: GenerationInput, providerId: string): Promise<ResearchDraft> {
  return invoke<ResearchDraft>("generate_research_content", { input, providerId });
}

/** 将同一主页至少三件完整公开作品分析并保存为风格。 */
export async function analyzeStyle(platform: ContentPlatform, homepageUrl: string, sourceIds: string[], name: string, providerId: string): Promise<StyleProfile> {
  return invoke<StyleProfile>("analyze_research_style", { platform, homepageUrl, sourceIds, name, providerId });
}

/** 基于当前修订保存人工编辑的风格应用指令。 */
export async function updateStyle(style: StyleProfile): Promise<StyleProfile> {
  return invoke<StyleProfile>("update_research_style", { style });
}

/** 保存用户编辑的内置二创和风格分析提示词。 */
export async function updatePrompts(prompts: ResearchPrompts): Promise<void> {
  return invoke<void>("update_research_prompts", { prompts });
}

/** 保存已生成草稿的标题和正文，不改写来源与模型血缘。 */
export async function updateDraft(id: string, title: string, body: string): Promise<void> {
  return invoke<void>("update_research_draft", { id, title, body });
}

/** 使用系统保存对话框导出已保存的原创内容。 */
export async function exportDraft(id: string, format: "markdown" | "text" | "html"): Promise<boolean> {
  return invoke<boolean>("export_research_draft", { id, format });
}

/** 将安全后端错误映射为可读提示，不显示任意对象或内部堆栈。 */
export function researchError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return "操作未完成，请检查当前配置后重试";
}

/** 原子复制已保存正文到既有文章流程，重复操作不会重复创建任务。 */
export async function prepareDraftArticle(id: string): Promise<string> {
  return invoke<string>("prepare_research_article_task", { id });
}
