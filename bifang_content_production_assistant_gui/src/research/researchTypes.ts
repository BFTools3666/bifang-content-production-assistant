/** 对标来源与生成目标的平台契约。 */
export type ContentPlatform = "csdn" | "baijiahao" | "xiaohongshu";

/** 平台未公开的互动指标保留null，不能与0混淆。 */
export interface WorkMetrics {
  views: number | null;
  likes: number | null;
  favorites: number | null;
  comments: number | null;
  shares: number | null;
}

/** 后端校验并保存的公开作品。 */
export interface BenchmarkWork {
  id: string;
  platform: ContentPlatform;
  title: string;
  url: string;
  author: string;
  authorUrl: string;
  publishedAt: string;
  excerpt: string;
  body: string;
  metrics: WorkMetrics;
  collectedAtMs: number;
}

/** 风格库资产包含冻结样本与可修改的应用指令。 */
export interface StyleProfile {
  id: string;
  name: string;
  platform: ContentPlatform;
  author: string;
  homepageUrl: string;
  samples: BenchmarkWork[];
  analysis: string;
  instructions: string;
  revision: number;
  createdAtMs: number;
  updatedAtMs: number;
}

/** 用户可保存和单次覆盖的内置提示词。 */
export interface ResearchPrompts {
  seoRewrite: string;
  xhsRewrite: string;
  styleAnalysis: string;
}

/** 真实生成草稿及非秘密来源、模型和用量记录。 */
export interface ResearchDraft {
  id: string;
  platform: ContentPlatform;
  keyword: string;
  sourceIds: string[];
  styleId: string | null;
  titles: string[];
  title: string;
  body: string;
  tags: string[];
  prompt: string;
  providerId: string;
  model: string;
  tokenUsage: { inputTokens: number | null; outputTokens: number | null; totalTokens: number | null } | null;
  createdAtMs: number;
}

/** 与原文章工作台一起由SQLite原子保存的对标资产。 */
export interface ResearchData {
  works: BenchmarkWork[];
  styles: StyleProfile[];
  drafts: ResearchDraft[];
  prompts: ResearchPrompts;
}

/** 用户提交给core验证的单次原创写作请求。 */
export interface GenerationInput {
  platform: ContentPlatform;
  keyword: string;
  sourceIds: string[];
  styleId: string | null;
  userMaterial: string;
  promptOverride: string;
}

/** 可见采集窗口的最近一次真实结果。 */
export interface CollectionStatus {
  pageUrl: string;
  message: string;
  collectedCount: number;
  updatedAtMs: number;
}
