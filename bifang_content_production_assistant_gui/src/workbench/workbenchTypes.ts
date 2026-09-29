/** 固定 SOP 中可由后端独立记录和恢复的阶段状态。 */
export type StageStatus =
  | "notStarted"
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "stale"
  | "cancelled"
  | "interrupted";

/** Grid 行的总体状态由 Rust core 从阶段与审核状态推导。 */
export type OverallStatus =
  | "draft"
  | "queued"
  | "running"
  | "pendingReview"
  | "approved"
  | "partialFailure"
  | "cancelled"
  | "interrupted";

/** 当前文章包的人工审核状态。 */
export type ReviewStatus = "draft" | "pending" | "approved";

/** 后端允许管理的三类 Prompt 模板。 */
export type PromptTemplateKind = "article" | "summary" | "image";

/** 固定 SOP 阶段名称。 */
export type WorkflowStage = "article" | "summary" | "image" | "format";

/** AI 服务适配器类型；火山方舟配置只能承担图片阶段。 */
export type ProviderKind =
  "openAiCompatible" | "volcengineArkImage" | "fixture";

/** 一次批量运行的持久状态。 */
export type RunStatus =
  | "queued"
  | "running"
  | "paused"
  | "succeeded"
  | "partialFailure"
  | "cancelled";

/** 一次阶段尝试的持久状态。 */
export type StageAttemptStatus =
  "running" | "succeeded" | "failed" | "interrupted" | "cancelled";

/** 可安全展示的阶段失败，不含外部响应或凭据。 */
export interface Failure {
  code: string;
  message: string;
  retryable: boolean;
  retryAfterMs: number | null;
}

/** Provider 对一次生成实际返回的 token 字段；缺失字段绝不由客户端推算。 */
export interface TokenUsage {
  inputTokens: number | null;
  outputTokens: number | null;
  totalTokens: number | null;
}

/** 单个阶段的当前状态与尝试次数。 */
export interface StageProgress {
  status: StageStatus;
  attempts: number;
  artifactVersion: number | null;
  failure: Failure | null;
  activeAttemptId: string | null;
}

/** 固定四阶段 SOP 的状态集合。 */
export interface StageStates {
  article: StageProgress;
  summary: StageProgress;
  image: StageProgress;
  format: StageProgress;
}

/** 单项产物的生成血缘。 */
export interface ArtifactLineage {
  packageVersion: number;
  sourceArticleVersion: number | null;
  sourceSummaryVersion: number | null;
  templateId: string | null;
  templateVersion: number | null;
  providerId: string | null;
  providerRevision: number | null;
  model: string | null;
  attemptId: string;
}

/** 相对于 AI 原图、以百万分之一为单位的整数裁剪矩形。 */
export interface ImageCrop {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 后端按固定次序执行的完整图片编辑白名单。 */
export interface ImageEditSpec {
  crop: ImageCrop;
  rotationQuarterTurns: 0 | 1 | 2 | 3;
  flipHorizontal: boolean;
  flipVertical: boolean;
}

/** 与 AI 阶段尝试分离的一次人工配图编辑血缘。 */
export interface ImageEditLineage {
  sourceAttemptId: string;
  editId: string;
  editedAtMs: number;
  spec: ImageEditSpec;
}

/** 当前文章包的四类产物及其生成血缘。 */
export interface ArticleArtifacts {
  markdown: string | null;
  summary: string | null;
  html: string | null;
  articleLineage: ArtifactLineage | null;
  summaryLineage: ArtifactLineage | null;
  imageLineage: ArtifactLineage | null;
  imageEditLineage: ImageEditLineage | null;
  imageEditRevision: number;
  formatLineage: ArtifactLineage | null;
}

/** 编辑器保存被禁止时的后端稳定原因。 */
export interface ImageEditBlockedReason {
  code: string;
  message: string;
}

/** 只读图片编辑器 IPC；不包含任意本地路径或文件引用。 */
export interface ArticleImageEditorView {
  imageDataUrl: string;
  sourceWidth: number;
  sourceHeight: number;
  currentSpec: ImageEditSpec;
  rowVersion: number;
  packageVersion: number;
  imageAttemptId: string | null;
  editRevision: number;
  canSave: boolean;
  saveBlockedReason: ImageEditBlockedReason | null;
}

/** 历史已批准的不可变文章包。 */
export interface ApprovedPackage {
  version: number;
  title: string;
  keywords: string[];
  requirements: string;
  artifacts: ArticleArtifacts;
  approvedAtMs: number;
}

/** 创建新版本前冻结的完整文章包。 */
export interface ArticlePackageSnapshot {
  version: number;
  title: string;
  keywords: string[];
  requirements: string;
  reviewStatus: ReviewStatus;
  artifacts: ArticleArtifacts;
  capturedAtMs: number;
}

/** 可审计的一次真实阶段执行。 */
export interface StageAttempt {
  id: string;
  packageVersion: number;
  stage: WorkflowStage;
  status: StageAttemptStatus;
  startedAtMs: number;
  finishedAtMs: number | null;
  failure: Failure | null;
  tokenUsage: TokenUsage | null;
  lineage: ArtifactLineage | null;
}

/** Rust core 持有的一行完整文章任务。 */
export interface ArticleTask {
  id: string;
  title: string;
  keywords: string[];
  requirements: string;
  packageVersion: number;
  rowVersion: number;
  stages: StageStates;
  reviewStatus: ReviewStatus;
  artifacts: ArticleArtifacts;
  approvedHistory: ApprovedPackage[];
  packageHistory: ArticlePackageSnapshot[];
  attemptHistory: StageAttempt[];
  createdAtMs: number;
  updatedAtMs: number;
}

/** GUI 查询中的文章行及其推导状态。 */
export interface ArticleView {
  article: ArticleTask;
  overallStatus: OverallStatus;
  hasImage: boolean;
}

/** 批次运行记录。 */
export interface RunRecord {
  id: string;
  taskIds: string[];
  /** null 表示完整 SOP 批次；其余值表示只执行该阶段的一次运行。 */
  targetStage: WorkflowStage | null;
  status: RunStatus;
  startedAtMs: number;
  finishedAtMs: number | null;
  completedCount: number;
  failedCount: number;
  isTestFixture: boolean;
}

/** 工作台首屏一次查询需要的完整后端视图。 */
export interface WorkbenchView {
  articles: ArticleView[];
  runHistory: RunRecord[];
  activeRun: RunRecord | null;
}

/** 批量粘贴标题的后端结果。 */
export interface AddTitlesResult {
  createdCount: number;
  ignoredBlankCount: number;
  duplicateCount: number;
  taskIds: string[];
}

/** 启动批次的后端资格判定结果。 */
export interface BatchStartResult {
  batchId: string | null;
  queuedCount: number;
  queuedTaskIds: string[];
  skippedCount: number;
  skipped: { taskId: string; reason: string }[];
}

/** 可版本化的 Prompt 模板。 */
export interface PromptTemplate {
  id: string;
  name: string;
  kind: PromptTemplateKind;
  content: string;
  version: number;
}

/** 后端从 core 变量契约派生的 Prompt 模板展示视图。 */
export interface PromptTemplateView extends PromptTemplate {
  allowedVariables: string[];
  requiredVariables: string[];
}

/** 正文或简介阶段允许发送的有界参数。 */
export interface TextGenerationParameters {
  temperatureMilli: number | null;
  maxOutputTokens: number | null;
  timeoutMs: number;
}

/** 配图阶段允许发送的有界参数。 */
export interface ImageGenerationParameters {
  size: "1024x1024" | "1536x1024" | "1024x1536" | "2K";
  timeoutMs: number;
  watermark: boolean;
}

/** 不含 API 密钥、并为三个阶段分别冻结模型与参数的 AI 服务配置。 */
export interface ProviderConfig {
  id: string;
  name: string;
  revision: number;
  kind: ProviderKind;
  enabled: boolean;
  baseUrl: string;
  /** 仅用于读取旧快照的共享文本模型镜像。 */
  textModel: string;
  articleModel: string;
  summaryModel: string;
  imageModel: string;
  articleParameters: TextGenerationParameters;
  summaryParameters: TextGenerationParameters;
  imageParameters: ImageGenerationParameters;
  defaultForArticle: boolean;
  defaultForSummary: boolean;
  defaultForImage: boolean;
}

/** GUI 可见的 AI 服务状态，不返回密钥内容。 */
export interface ProviderConfigView extends ProviderConfig {
  hasApiKey: boolean;
  isTestFixture: boolean;
}

/** Provider 保存后同一原子默认位更新的完整非秘密结果。 */
export interface ProviderConfigSaveResult {
  saved: ProviderConfigView;
  providers: ProviderConfigView[];
}

/** Provider 安全删除后的当前集合与凭据清理状态。 */
export interface ProviderConfigDeleteResult {
  providers: ProviderConfigView[];
  credentialCleanupPending: boolean;
  historicalBindingsDetached: number;
}

/** 用户显式连接测试的安全结果。 */
export interface ProviderConnectionResult {
  ok: boolean;
  message: string;
}

/** 一次真实图片测试返回的内存预览，不含远端 URL 或本地路径。 */
export interface ProviderImageTestResult {
  providerId: string;
  providerRevision: number;
  mimeType: "image/png" | "image/jpeg" | "image/webp";
  byteLength: number;
  imageDataUrl: string;
}

/** 首版云文档连接器支持的远端类型。 */
export type ConnectorKind = "feishuDrive";

/** 最终文章可以按两种自包含文件格式同步。 */
export type ConnectorDocumentFormat = "markdown" | "html";

/** 不含 CLI token 的飞书发布目标配置。 */
export interface ConnectorConfig {
  id: string;
  name: string;
  revision: number;
  kind: ConnectorKind;
  enabled: boolean;
  cliProfile: string;
  folderToken: string;
  defaultFormat: ConnectorDocumentFormat;
  isDefault: boolean;
  timeoutMs: number;
}

/** GUI 可见的发布目标不包含任何 CLI 凭据或原始命令输出。 */
export type ConnectorConfigView = ConnectorConfig;

/** 保存配置后返回默认位原子更新后的完整列表。 */
export interface ConnectorConfigSaveResult {
  saved: ConnectorConfigView;
  connectors: ConnectorConfigView[];
}

/** 删除连接器后返回剩余列表和系统凭据清理状态。 */
export interface ConnectorConfigDeleteResult {
  connectors: ConnectorConfigView[];
  credentialCleanupPending: boolean;
}

/** 用户显式执行只读连接测试的安全结果。 */
export interface ConnectorConnectionResult {
  ok: boolean;
  message: string;
}

/** 官方 CLI 中可供用户选择的非秘密 profile 摘要。 */
export interface LarkCliProfileSummary {
  name: string;
  userName: string | null;
  tokenStatus: string | null;
}

/** 首次连接向导的一次只读环境检测结果。 */
export interface LarkCliSetupInspection {
  version: string;
  profiles: LarkCliProfileSummary[];
  selectedProfileConfigured: boolean;
  selectedProfileAuthorized: boolean;
  selectedUserName: string | null;
  authorizationIssueCode: string | null;
}

/** 一次文章文件同步的可展示、可幂等复用结果。 */
export interface ConnectorSyncResult {
  taskId: string;
  packageVersion: number;
  connectorId: string;
  connectorRevision: number;
  format: ConnectorDocumentFormat;
  fileToken: string;
  fileName: string;
  syncedAtMs: number;
  reused: boolean;
}
