import type {
  AddTitlesResult,
  ArticleImageEditorView,
  ArticleView,
  BatchStartResult,
  PromptTemplate,
  PromptTemplateView,
  RunRecord,
  WorkbenchView,
  ImageEditSpec,
} from "./workbenchTypes";

import {
  arrayValue,
  booleanValue,
  enumValue,
  exactRecord,
  invalid,
  nullable,
  stringArray,
  stringValue,
  uintValue,
} from "./workbenchValidationPrimitives";
import type { CommandError } from "./workbenchValidationPrimitives";

/** Rust core 允许序列化的阶段状态。 */
const STAGE_STATUSES = new Set([
  "notStarted",
  "queued",
  "running",
  "succeeded",
  "failed",
  "stale",
  "cancelled",
  "interrupted",
]);
/** Rust core 允许序列化的文章总体状态。 */
const OVERALL_STATUSES = new Set([
  "draft",
  "queued",
  "running",
  "pendingReview",
  "approved",
  "partialFailure",
  "cancelled",
  "interrupted",
]);
/** Rust core 允许序列化的审核状态。 */
const REVIEW_STATUSES = new Set(["draft", "pending", "approved"]);
/** 固定 SOP 阶段标识。 */
const WORKFLOW_STAGES = new Set(["article", "summary", "image", "format"]);
/** 持久化阶段尝试允许的状态。 */
const ATTEMPT_STATUSES = new Set([
  "running",
  "succeeded",
  "failed",
  "interrupted",
  "cancelled",
]);
/** 批量运行允许的状态。 */
const RUN_STATUSES = new Set([
  "queued",
  "running",
  "paused",
  "succeeded",
  "partialFailure",
  "cancelled",
]);
/** 当前支持的 Prompt 类型。 */
const PROMPT_KINDS = new Set(["article", "summary", "image"]);
/** 与 Rust core 一致的相对裁剪整数精度。 */
const IMAGE_EDIT_COORDINATE_SCALE = 1_000_000;
/** 只读编辑器允许返回的稳定保存禁用原因。 */
const IMAGE_EDIT_BLOCKED_CODES = new Set([
  "IMAGE_EDIT_STALE",
  "IMAGE_EDIT_ARTICLE_BUSY",
  "IMAGE_EDIT_NOT_READY",
  "IMAGE_EDIT_LINEAGE_MISSING",
  "IMAGE_EDIT_UNSUPPORTED_SOURCE",
]);

/** 验证可安全展示的阶段失败。 */
function decodeFailure(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["code", "message", "retryable", "retryAfterMs"],
    path,
  );
  stringValue(record.code, `${path}.code`, true);
  stringValue(record.message, `${path}.message`, true);
  booleanValue(record.retryable, `${path}.retryable`);
  nullable(record.retryAfterMs, uintValue, `${path}.retryAfterMs`);
}

/** 验证单项产物的完整生成血缘。 */
function decodeLineage(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    [
      "packageVersion",
      "sourceArticleVersion",
      "sourceSummaryVersion",
      "templateId",
      "templateVersion",
      "providerId",
      "providerRevision",
      "model",
      "attemptId",
    ],
    path,
  );
  uintValue(record.packageVersion, `${path}.packageVersion`, true);
  nullable(
    record.sourceArticleVersion,
    uintValue,
    `${path}.sourceArticleVersion`,
  );
  nullable(
    record.sourceSummaryVersion,
    uintValue,
    `${path}.sourceSummaryVersion`,
  );
  nullable(record.templateId, stringValue, `${path}.templateId`);
  nullable(record.templateVersion, uintValue, `${path}.templateVersion`);
  nullable(record.providerId, stringValue, `${path}.providerId`);
  nullable(record.providerRevision, uintValue, `${path}.providerRevision`);
  nullable(record.model, stringValue, `${path}.model`);
  stringValue(record.attemptId, `${path}.attemptId`, true);
}

/** 严格验证相对裁剪矩形，拒绝零面积、溢出和越界。 */
function decodeImageCrop(value: unknown, path: string): void {
  const record = exactRecord(value, ["x", "y", "width", "height"], path);
  const x = uintValue(record.x, `${path}.x`);
  const y = uintValue(record.y, `${path}.y`);
  const width = uintValue(record.width, `${path}.width`, true);
  const height = uintValue(record.height, `${path}.height`, true);
  if (
    x >= IMAGE_EDIT_COORDINATE_SCALE ||
    y >= IMAGE_EDIT_COORDINATE_SCALE ||
    x + width > IMAGE_EDIT_COORDINATE_SCALE ||
    y + height > IMAGE_EDIT_COORDINATE_SCALE
  ) {
    throw invalid(path);
  }
}

/** 严格验证前后端共享的裁剪、旋转和翻转白名单。 */
export function decodeImageEditSpec(value: unknown): ImageEditSpec {
  const path = "imageEditSpec";
  const record = exactRecord(
    value,
    ["crop", "rotationQuarterTurns", "flipHorizontal", "flipVertical"],
    path,
  );
  decodeImageCrop(record.crop, `${path}.crop`);
  const rotation = uintValue(
    record.rotationQuarterTurns,
    `${path}.rotationQuarterTurns`,
  );
  if (rotation > 3) throw invalid(`${path}.rotationQuarterTurns`);
  booleanValue(record.flipHorizontal, `${path}.flipHorizontal`);
  booleanValue(record.flipVertical, `${path}.flipVertical`);
  return value as ImageEditSpec;
}

/** 验证与 AI 尝试分离的人工编辑血缘。 */
function decodeImageEditLineage(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["sourceAttemptId", "editId", "editedAtMs", "spec"],
    path,
  );
  stringValue(record.sourceAttemptId, `${path}.sourceAttemptId`, true);
  stringValue(record.editId, `${path}.editId`, true);
  uintValue(record.editedAtMs, `${path}.editedAtMs`);
  decodeImageEditSpecAtPath(record.spec, `${path}.spec`);
}

/** 在嵌套协议位置复用图片编辑规范验证，并保留准确错误路径。 */
function decodeImageEditSpecAtPath(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["crop", "rotationQuarterTurns", "flipHorizontal", "flipVertical"],
    path,
  );
  decodeImageCrop(record.crop, `${path}.crop`);
  const rotation = uintValue(
    record.rotationQuarterTurns,
    `${path}.rotationQuarterTurns`,
  );
  if (rotation > 3) throw invalid(`${path}.rotationQuarterTurns`);
  booleanValue(record.flipHorizontal, `${path}.flipHorizontal`);
  booleanValue(record.flipVertical, `${path}.flipVertical`);
}

/** 验证当前文章包的产物及血缘字段。 */
function decodeArtifacts(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    [
      "markdown",
      "summary",
      "imageRef",
      "originalImageRef",
      "html",
      "articleLineage",
      "summaryLineage",
      "imageLineage",
      "imageEditLineage",
      "imageEditRevision",
      "formatLineage",
    ],
    path,
  );
  nullable(record.markdown, stringValue, `${path}.markdown`);
  nullable(record.summary, stringValue, `${path}.summary`);
  if (record.imageRef !== null) throw invalid(`${path}.imageRef`);
  if (record.originalImageRef !== null) {
    throw invalid(`${path}.originalImageRef`);
  }
  nullable(record.html, stringValue, `${path}.html`);
  for (const key of [
    "articleLineage",
    "summaryLineage",
    "imageLineage",
    "formatLineage",
  ]) {
    nullable(record[key], decodeLineage, `${path}.${key}`);
  }
  nullable(
    record.imageEditLineage,
    decodeImageEditLineage,
    `${path}.imageEditLineage`,
  );
  uintValue(record.imageEditRevision, `${path}.imageEditRevision`);
}

/** 验证一个 SOP 阶段的状态、失败与活动尝试。 */
function decodeStageProgress(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["status", "attempts", "artifactVersion", "failure", "activeAttemptId"],
    path,
  );
  enumValue(record.status, STAGE_STATUSES, `${path}.status`);
  uintValue(record.attempts, `${path}.attempts`);
  nullable(record.artifactVersion, uintValue, `${path}.artifactVersion`);
  nullable(record.failure, decodeFailure, `${path}.failure`);
  nullable(record.activeAttemptId, stringValue, `${path}.activeAttemptId`);
}

/** 验证四阶段状态集合。 */
function decodeStageStates(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["article", "summary", "image", "format"],
    path,
  );
  for (const key of ["article", "summary", "image", "format"]) {
    decodeStageProgress(record[key], `${path}.${key}`);
  }
}

/** 验证已批准文章包的不可变快照。 */
function decodeApprovedPackage(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    [
      "version",
      "title",
      "keywords",
      "requirements",
      "artifacts",
      "approvedAtMs",
    ],
    path,
  );
  uintValue(record.version, `${path}.version`, true);
  stringValue(record.title, `${path}.title`);
  stringArray(record.keywords, `${path}.keywords`);
  stringValue(record.requirements, `${path}.requirements`);
  decodeArtifacts(record.artifacts, `${path}.artifacts`);
  uintValue(record.approvedAtMs, `${path}.approvedAtMs`);
}

/** 验证被新版本替换的完整文章包快照。 */
function decodePackageSnapshot(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    [
      "version",
      "title",
      "keywords",
      "requirements",
      "reviewStatus",
      "artifacts",
      "capturedAtMs",
    ],
    path,
  );
  uintValue(record.version, `${path}.version`, true);
  stringValue(record.title, `${path}.title`);
  stringArray(record.keywords, `${path}.keywords`);
  stringValue(record.requirements, `${path}.requirements`);
  enumValue(record.reviewStatus, REVIEW_STATUSES, `${path}.reviewStatus`);
  decodeArtifacts(record.artifacts, `${path}.artifacts`);
  uintValue(record.capturedAtMs, `${path}.capturedAtMs`);
}

/** 验证一次阶段执行尝试。 */
function decodeStageAttempt(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    [
      "id",
      "packageVersion",
      "stage",
      "status",
      "startedAtMs",
      "finishedAtMs",
      "failure",
      "tokenUsage",
      "lineage",
    ],
    path,
  );
  stringValue(record.id, `${path}.id`, true);
  uintValue(record.packageVersion, `${path}.packageVersion`, true);
  enumValue(record.stage, WORKFLOW_STAGES, `${path}.stage`);
  enumValue(record.status, ATTEMPT_STATUSES, `${path}.status`);
  uintValue(record.startedAtMs, `${path}.startedAtMs`);
  nullable(record.finishedAtMs, uintValue, `${path}.finishedAtMs`);
  nullable(record.failure, decodeFailure, `${path}.failure`);
  nullable(record.tokenUsage, decodeTokenUsage, `${path}.tokenUsage`);
  nullable(record.lineage, decodeLineage, `${path}.lineage`);
}

/** 验证逐字段可缺失且不经推算的 Provider token 用量。 */
function decodeTokenUsage(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["inputTokens", "outputTokens", "totalTokens"],
    path,
  );
  nullable(record.inputTokens, uintValue, `${path}.inputTokens`);
  nullable(record.outputTokens, uintValue, `${path}.outputTokens`);
  nullable(record.totalTokens, uintValue, `${path}.totalTokens`);
}

/** 从未知 IPC 值严格解码一行文章视图。 */
export function decodeArticleView(value: unknown): ArticleView {
  const path = "articleView";
  const view = exactRecord(
    value,
    ["article", "overallStatus", "hasImage"],
    path,
  );
  const article = exactRecord(
    view.article,
    [
      "id",
      "title",
      "keywords",
      "requirements",
      "packageVersion",
      "rowVersion",
      "stages",
      "reviewStatus",
      "artifacts",
      "approvedHistory",
      "packageHistory",
      "attemptHistory",
      "createdAtMs",
      "updatedAtMs",
    ],
    `${path}.article`,
  );
  stringValue(article.id, `${path}.article.id`, true);
  stringValue(article.title, `${path}.article.title`, true);
  stringArray(article.keywords, `${path}.article.keywords`);
  stringValue(article.requirements, `${path}.article.requirements`);
  uintValue(article.packageVersion, `${path}.article.packageVersion`, true);
  uintValue(article.rowVersion, `${path}.article.rowVersion`, true);
  decodeStageStates(article.stages, `${path}.article.stages`);
  enumValue(
    article.reviewStatus,
    REVIEW_STATUSES,
    `${path}.article.reviewStatus`,
  );
  decodeArtifacts(article.artifacts, `${path}.article.artifacts`);
  arrayValue(
    article.approvedHistory,
    decodeApprovedPackage,
    `${path}.article.approvedHistory`,
  );
  arrayValue(
    article.packageHistory,
    decodePackageSnapshot,
    `${path}.article.packageHistory`,
  );
  arrayValue(
    article.attemptHistory,
    decodeStageAttempt,
    `${path}.article.attemptHistory`,
  );
  uintValue(article.createdAtMs, `${path}.article.createdAtMs`);
  uintValue(article.updatedAtMs, `${path}.article.updatedAtMs`);
  enumValue(view.overallStatus, OVERALL_STATUSES, `${path}.overallStatus`);
  booleanValue(view.hasImage, `${path}.hasImage`);
  return value as ArticleView;
}

/** 从未知 IPC 值严格解码一次批量运行记录。 */
export function decodeRunRecord(value: unknown): RunRecord {
  const path = "runRecord";
  const record = exactRecord(
    value,
    [
      "id",
      "taskIds",
      "targetStage",
      "status",
      "startedAtMs",
      "finishedAtMs",
      "completedCount",
      "failedCount",
      "isTestFixture",
    ],
    path,
  );
  stringValue(record.id, `${path}.id`, true);
  stringArray(record.taskIds, `${path}.taskIds`);
  nullable(
    record.targetStage,
    (stage, stagePath) => enumValue(stage, WORKFLOW_STAGES, stagePath),
    `${path}.targetStage`,
  );
  enumValue(record.status, RUN_STATUSES, `${path}.status`);
  uintValue(record.startedAtMs, `${path}.startedAtMs`);
  nullable(record.finishedAtMs, uintValue, `${path}.finishedAtMs`);
  uintValue(record.completedCount, `${path}.completedCount`);
  uintValue(record.failedCount, `${path}.failedCount`);
  booleanValue(record.isTestFixture, `${path}.isTestFixture`);
  return value as RunRecord;
}

/** 从未知 IPC 值严格解码工作台聚合视图。 */
export function decodeWorkbenchView(value: unknown): WorkbenchView {
  const record = exactRecord(
    value,
    ["articles", "runHistory", "activeRun"],
    "workbench",
  );
  arrayValue(
    record.articles,
    (item) => decodeArticleView(item),
    "workbench.articles",
  );
  arrayValue(
    record.runHistory,
    (item) => decodeRunRecord(item),
    "workbench.runHistory",
  );
  nullable(
    record.activeRun,
    (item) => decodeRunRecord(item),
    "workbench.activeRun",
  );
  return value as WorkbenchView;
}

/** 从未知 IPC 值严格解码批量添加标题结果。 */
export function decodeAddTitlesResult(value: unknown): AddTitlesResult {
  const record = exactRecord(
    value,
    ["createdCount", "ignoredBlankCount", "duplicateCount", "taskIds"],
    "addTitlesResult",
  );
  const taskIds = stringArray(record.taskIds, "addTitlesResult.taskIds");
  const createdCount = uintValue(
    record.createdCount,
    "addTitlesResult.createdCount",
  );
  uintValue(record.ignoredBlankCount, "addTitlesResult.ignoredBlankCount");
  uintValue(record.duplicateCount, "addTitlesResult.duplicateCount");
  if (createdCount !== taskIds.length)
    throw invalid("addTitlesResult.createdCount");
  return value as AddTitlesResult;
}

/** 从未知 IPC 值严格解码批次启动或预检结果。 */
export function decodeBatchStartResult(value: unknown): BatchStartResult {
  const record = exactRecord(
    value,
    ["batchId", "queuedCount", "queuedTaskIds", "skippedCount", "skipped"],
    "batchStartResult",
  );
  nullable(record.batchId, stringValue, "batchStartResult.batchId");
  const queuedIds = stringArray(
    record.queuedTaskIds,
    "batchStartResult.queuedTaskIds",
  );
  const skipped = arrayValue(
    record.skipped,
    (item, path) => {
      const entry = exactRecord(item, ["taskId", "reason"], path);
      stringValue(entry.taskId, `${path}.taskId`, true);
      stringValue(entry.reason, `${path}.reason`, true);
      return entry;
    },
    "batchStartResult.skipped",
  );
  if (
    uintValue(record.queuedCount, "batchStartResult.queuedCount") !==
    queuedIds.length
  ) {
    throw invalid("batchStartResult.queuedCount");
  }
  if (
    uintValue(record.skippedCount, "batchStartResult.skippedCount") !==
    skipped.length
  ) {
    throw invalid("batchStartResult.skippedCount");
  }
  return value as BatchStartResult;
}

/** 从未知 IPC 值严格解码一份 Prompt 模板。 */
export function decodePromptTemplate(value: unknown): PromptTemplate {
  const record = exactRecord(
    value,
    ["id", "name", "kind", "content", "version"],
    "promptTemplate",
  );
  stringValue(record.id, "promptTemplate.id", true);
  stringValue(record.name, "promptTemplate.name", true);
  enumValue(record.kind, PROMPT_KINDS, "promptTemplate.kind");
  stringValue(record.content, "promptTemplate.content", true);
  uintValue(record.version, "promptTemplate.version", true);
  return value as PromptTemplate;
}

/** 从未知 IPC 值严格解码 Prompt 模板列表。 */
export function decodePromptTemplates(value: unknown): PromptTemplateView[] {
  const templates = arrayValue(
    value,
    (item, path) => {
      const record = exactRecord(
        item,
        [
          "id",
          "name",
          "kind",
          "content",
          "version",
          "allowedVariables",
          "requiredVariables",
        ],
        path,
      );
      stringValue(record.id, `${path}.id`, true);
      stringValue(record.name, `${path}.name`, true);
      enumValue(record.kind, PROMPT_KINDS, `${path}.kind`);
      stringValue(record.content, `${path}.content`, true);
      uintValue(record.version, `${path}.version`, true);
      const allowed = stringArray(
        record.allowedVariables,
        `${path}.allowedVariables`,
      );
      const required = stringArray(
        record.requiredVariables,
        `${path}.requiredVariables`,
      );
      if (
        allowed.length === 0 ||
        required.length === 0 ||
        new Set(allowed).size !== allowed.length ||
        new Set(required).size !== required.length ||
        required.some((variable) => !allowed.includes(variable))
      ) {
        throw invalid(`${path}.variables`);
      }
      return item as PromptTemplateView;
    },
    "promptTemplates",
  );
  const ids = new Set(templates.map((template) => template.id));
  const kinds = new Set(templates.map((template) => template.kind));
  if (
    templates.length !== PROMPT_KINDS.size ||
    ids.size !== templates.length ||
    kinds.size !== PROMPT_KINDS.size
  ) {
    throw invalid("promptTemplates");
  }
  return templates;
}

/** 从未知 IPC 值严格解码批量运行记录列表。 */
export function decodeRunRecords(value: unknown): RunRecord[] {
  return arrayValue(value, (item) => decodeRunRecord(item), "runRecords");
}

/** 从未知拒绝值严格解码后端稳定安全错误。 */
export function decodeCommandError(value: unknown): CommandError {
  const record = exactRecord(value, ["code", "message"], "commandError");
  return {
    code: stringValue(record.code, "commandError.code", true),
    message: stringValue(record.message, "commandError.message", true),
  };
}

/** 从未知 IPC 值解码 Prompt 本地预览正文。 */
export function decodePromptPreview(value: unknown): string {
  const record = exactRecord(value, ["rendered"], "promptPreview");
  return stringValue(record.rendered, "promptPreview.rendered");
}

/** 从未知 IPC 值解码可空的文章图片 data URL。 */
export function decodeArticleImage(value: unknown): string | null {
  if (value === null) return null;
  const image = stringValue(value, "articleImage", true);
  if (
    !/^data:image\/(?:png|jpeg|webp|svg\+xml);base64,[A-Za-z0-9+/]+={0,2}$/.test(
      image,
    )
  ) {
    throw invalid("articleImage");
  }
  return image;
}

/** 严格解码图片编辑器只读视图，并拒绝路径、额外参数与不一致可编辑状态。 */
export function decodeArticleImageEditorView(
  value: unknown,
): ArticleImageEditorView {
  const path = "articleImageEditor";
  const record = exactRecord(
    value,
    [
      "imageDataUrl",
      "sourceWidth",
      "sourceHeight",
      "currentSpec",
      "rowVersion",
      "packageVersion",
      "imageAttemptId",
      "editRevision",
      "canSave",
      "saveBlockedReason",
    ],
    path,
  );
  const rawImageDataUrl = stringValue(
    record.imageDataUrl,
    `${path}.imageDataUrl`,
    true,
  );
  if (rawImageDataUrl.length > 35_000_000) {
    throw invalid(`${path}.imageDataUrl`);
  }
  const imageDataUrl = decodeArticleImage(rawImageDataUrl);
  if (imageDataUrl === null) throw invalid(`${path}.imageDataUrl`);
  const sourceWidth = uintValue(
    record.sourceWidth,
    `${path}.sourceWidth`,
    true,
  );
  const sourceHeight = uintValue(
    record.sourceHeight,
    `${path}.sourceHeight`,
    true,
  );
  if (sourceWidth > 4096 || sourceHeight > 4096) {
    throw invalid(path);
  }
  decodeImageEditSpecAtPath(record.currentSpec, `${path}.currentSpec`);
  uintValue(record.rowVersion, `${path}.rowVersion`, true);
  uintValue(record.packageVersion, `${path}.packageVersion`, true);
  const attemptId = nullable(
    record.imageAttemptId,
    (input, fieldPath) => stringValue(input, fieldPath, true),
    `${path}.imageAttemptId`,
  );
  uintValue(record.editRevision, `${path}.editRevision`);
  const canSave = booleanValue(record.canSave, `${path}.canSave`);
  const blocked = nullable(
    record.saveBlockedReason,
    (input, fieldPath) => {
      const reason = exactRecord(input, ["code", "message"], fieldPath);
      const code = stringValue(reason.code, `${fieldPath}.code`, true);
      if (!IMAGE_EDIT_BLOCKED_CODES.has(code)) {
        throw invalid(`${fieldPath}.code`);
      }
      stringValue(reason.message, `${fieldPath}.message`, true);
      return true;
    },
    `${path}.saveBlockedReason`,
  );
  if (
    (canSave && (attemptId === null || blocked !== null)) ||
    (!canSave && blocked === null)
  ) {
    throw invalid(path);
  }
  return value as ArticleImageEditorView;
}

/** Grid 缩略图只接受原生层的有界 PNG，Fixture 模式兼容受控 SVG。 */
export function decodeArticleThumbnail(value: unknown): string | null {
  const thumbnail = decodeArticleImage(value);
  if (thumbnail !== null && thumbnail.length > 360_000) {
    throw invalid("articleThumbnail");
  }
  return thumbnail;
}

/** 从未知 IPC 值解码后端返回的批次 ID。 */
export function decodeBatchId(value: unknown): string {
  return stringValue(value, "batchId", true);
}

/** 验证无返回值命令只返回 Tauri unit 对应的 null。 */
export function decodeUnit(value: unknown): void {
  if (value !== null) throw invalid("unit");
}
