import { invoke } from "@tauri-apps/api/core";

import type {
  AddTitlesResult,
  ArticleImageEditorView,
  ArticleView,
  BatchStartResult,
  ConnectorConfig,
  ConnectorConfigDeleteResult,
  ConnectorConfigSaveResult,
  ConnectorConfigView,
  ConnectorConnectionResult,
  ConnectorDocumentFormat,
  ConnectorSyncResult,
  LarkCliSetupInspection,
  PromptTemplate,
  PromptTemplateKind,
  PromptTemplateView,
  ProviderConfig,
  ProviderConfigDeleteResult,
  ProviderConfigSaveResult,
  ProviderConfigView,
  ProviderConnectionResult,
  ProviderImageTestResult,
  RunRecord,
  ImageEditSpec,
  WorkbenchView,
  WorkflowStage,
} from "./workbenchTypes";
import {
  decodeAddTitlesResult,
  decodeArticleImage,
  decodeArticleImageEditorView,
  decodeArticleThumbnail,
  decodeArticleView,
  decodeBatchId,
  decodeBatchStartResult,
  decodePromptPreview,
  decodePromptTemplate,
  decodePromptTemplates,
  decodeRunRecords,
  decodeUnit,
  decodeWorkbenchView,
  decodeImageEditSpec,
} from "./workbenchValidation";
import {
  decodeConnectorConfig,
  decodeConnectorConfigDeleteResult,
  decodeConnectorConfigSaveResult,
  decodeConnectorConfigViews,
  decodeConnectorConnectionResult,
  decodeConnectorSyncResult,
  decodeLarkCliSetupInspection,
  decodeProviderConfig,
  decodeProviderConfigDeleteResult,
  decodeProviderConfigSaveResult,
  decodeProviderConfigViews,
  decodeProviderConnectionResult,
  decodeProviderImageTestResult,
} from "./workbenchConnectionValidation";

/** 读取 Grid、活动批次和最近运行记录。 */
export async function getWorkbench(): Promise<WorkbenchView> {
  return decodeWorkbenchView(await invoke<unknown>("get_workbench"));
}

/** 读取后端按文章 ID 授权的图片内联数据。 */
export async function loadArticleImage(taskId: string): Promise<string | null> {
  return decodeArticleImage(
    await invoke<unknown>("load_article_image", { taskId }),
  );
}

/** 读取原生层按固定尺寸生成的 Grid 配图缩略图。 */
export async function loadArticleThumbnail(
  taskId: string,
): Promise<string | null> {
  return decodeArticleThumbnail(
    await invoke<unknown>("load_article_thumbnail", { taskId }),
  );
}

/** 按文章 ID 读取 Rust 授权的 AI 原图、像素、当前编辑规范与乐观锁。 */
export async function loadArticleImageEditor(
  taskId: string,
): Promise<ArticleImageEditorView> {
  return decodeArticleImageEditorView(
    await invoke<unknown>("load_article_image_editor", { taskId }),
  );
}

/** 保存从 AI 原图重算的白名单编辑，并携带读取时的全部乐观锁。 */
export async function saveArticleImageEdit(
  taskId: string,
  expectedRowVersion: number,
  expectedPackageVersion: number,
  expectedImageAttemptId: string,
  expectedEditRevision: number,
  spec: ImageEditSpec,
): Promise<ArticleView> {
  const safeSpec = decodeImageEditSpec(spec);
  return decodeArticleView(
    await invoke<unknown>("save_article_image_edit", {
      taskId,
      expectedRowVersion,
      expectedPackageVersion,
      expectedImageAttemptId,
      expectedEditRevision,
      spec: safeSpec,
    }),
  );
}

/** 把多行标题批量创建为独立任务。 */
export async function addArticleTitles(
  titles: string,
): Promise<AddTitlesResult> {
  return decodeAddTitlesResult(
    await invoke<unknown>("add_article_titles", { titles }),
  );
}

/** 删除一条当前没有运行的文章任务。 */
export async function deleteArticle(taskId: string): Promise<void> {
  decodeUnit(await invoke<unknown>("delete_article", { taskId }));
}

/** 只启动明确选中的文章。 */
export async function startSelectedBatch(
  taskIds: string[],
): Promise<BatchStartResult> {
  return decodeBatchStartResult(
    await invoke<unknown>("start_selected_batch", { taskIds }),
  );
}

/** 只读预检选中行，不排队也不调用外部服务。 */
export async function previewSelectedBatch(
  taskIds: string[],
): Promise<BatchStartResult> {
  return decodeBatchStartResult(
    await invoke<unknown>("preview_selected_batch", { taskIds }),
  );
}

/** 只重试指定文章最早的失败或中断阶段。 */
export async function retryArticle(taskId: string): Promise<BatchStartResult> {
  return decodeBatchStartResult(
    await invoke<unknown>("retry_article", { taskId }),
  );
}

/** 把选中失败行组成一次精确重试批次。 */
export async function retrySelectedArticles(
  taskIds: string[],
): Promise<BatchStartResult> {
  return decodeBatchStartResult(
    await invoke<unknown>("retry_selected_articles", { taskIds }),
  );
}

/** 为一条文章创建新包版本并重新运行完整 SOP。 */
export async function regenerateArticle(
  taskId: string,
): Promise<BatchStartResult> {
  return decodeBatchStartResult(
    await invoke<unknown>("regenerate_article", { taskId }),
  );
}

/** 只运行用户在 Grid 或详情中点选的一个阶段，不自动推进后续 SOP。 */
export async function generateArticleStage(
  taskId: string,
  stage: WorkflowStage,
  expectedRowVersion: number,
  expectedPackageVersion: number,
  confirmed: boolean,
  expectedProviderId: string | null,
  expectedProviderRevision: number | null,
): Promise<BatchStartResult> {
  return decodeBatchStartResult(
    await invoke<unknown>("generate_article_stage", {
      confirmed,
      expectedPackageVersion,
      expectedProviderId,
      expectedProviderRevision,
      expectedRowVersion,
      stage,
      taskId,
    }),
  );
}

/** 保存文章关键词与内容要求。 */
export async function updateArticleInputs(
  taskId: string,
  keywords: string[],
  requirements: string,
): Promise<ArticleView> {
  return decodeArticleView(
    await invoke<unknown>("update_article_inputs", {
      taskId,
      keywords,
      requirements,
    }),
  );
}

/** 保存人工编辑的正文或简介。 */
export async function updateArticleContent(
  taskId: string,
  markdown: string | null,
  summary: string | null,
): Promise<ArticleView> {
  return decodeArticleView(
    await invoke<unknown>("update_article_content", {
      taskId,
      markdown,
      summary,
    }),
  );
}

/** 批准当前完整文章包。 */
export async function approveArticle(taskId: string): Promise<ArticleView> {
  return decodeArticleView(
    await invoke<unknown>("approve_article", { taskId }),
  );
}

/** 把待审核文章退回草稿供人工修改。 */
export async function returnArticleToDraft(
  taskId: string,
): Promise<ArticleView> {
  return decodeArticleView(
    await invoke<unknown>("return_article_to_draft", { taskId }),
  );
}

/** 暂停活动批次领取后续步骤。 */
export async function pauseActiveBatch(): Promise<string> {
  return decodeBatchId(await invoke<unknown>("pause_active_batch"));
}

/** 恢复已暂停批次。 */
export async function resumeActiveBatch(): Promise<string> {
  return decodeBatchId(await invoke<unknown>("resume_active_batch"));
}

/** 取消活动批次中尚未执行的工作。 */
export async function cancelActiveBatch(): Promise<string> {
  return decodeBatchId(await invoke<unknown>("cancel_active_batch"));
}

/** 读取三类 Prompt 的最新版本。 */
export async function listPromptTemplates(): Promise<PromptTemplateView[]> {
  return decodePromptTemplates(await invoke<unknown>("list_prompt_templates"));
}

/** 保存 Prompt 并由后端创建新版本。 */
export async function savePromptTemplate(
  template: PromptTemplate,
): Promise<PromptTemplate> {
  return decodePromptTemplate(
    await invoke<unknown>("save_prompt_template", { template }),
  );
}

/** 在本地渲染 Prompt 示例，绝不调用外部 AI。 */
export async function previewPromptTemplate(
  template: PromptTemplate,
  values: Record<string, string>,
): Promise<string> {
  return decodePromptPreview(
    await invoke<unknown>("preview_prompt_template", { template, values }),
  );
}

/** 读取不含密钥内容的 AI 服务配置。 */
export async function listProviderConfigs(): Promise<ProviderConfigView[]> {
  return decodeProviderConfigViews(
    await invoke<unknown>("list_provider_configs"),
  );
}

/** 读取 core 定义的新 AI 服务表单默认值。 */
export async function newProviderConfigDraft(): Promise<ProviderConfig> {
  return decodeProviderConfig(
    await invoke<unknown>("new_provider_config_draft"),
  );
}

/** 读取共享 core 的 DeepSeek 快速接入默认值，不进行付费测试。 */
export async function newDeepseekTextProviderDraft(): Promise<ProviderConfig> {
  return decodeProviderConfig(await invoke<unknown>("new_deepseek_text_provider_draft"));
}

/** 读取 core 定义的火山方舟图片服务默认值。 */
export async function newVolcengineArkImageProviderConfigDraft(): Promise<ProviderConfig> {
  return decodeProviderConfig(
    await invoke<unknown>("new_volcengine_ark_image_provider_config_draft"),
  );
}

/** 保存 AI 服务配置，并仅在非空时更新系统凭据库中的密钥。 */
export async function saveProviderConfig(
  config: ProviderConfig,
  apiKey: string | null,
): Promise<ProviderConfigSaveResult> {
  return decodeProviderConfigSaveResult(
    await invoke<unknown>("save_provider_config", { config, apiKey }),
  );
}

/** 启用或停用 AI 服务，并以当前修订防止陈旧界面覆盖新状态。 */
export async function setProviderEnabled(
  providerId: string,
  expectedRevision: number,
  enabled: boolean,
): Promise<ProviderConfigSaveResult> {
  return decodeProviderConfigSaveResult(
    await invoke<unknown>("set_provider_enabled", {
      providerId,
      expectedRevision,
      enabled,
    }),
  );
}

/** 把已保存服务显式分配给一个生成步骤，或只解除该步骤的分配。 */
export async function setProviderStageAssignment(
  providerId: string,
  expectedRevision: number,
  stage: PromptTemplateKind,
  assigned: boolean,
): Promise<ProviderConfigSaveResult> {
  return decodeProviderConfigSaveResult(
    await invoke<unknown>("set_provider_stage_assignment", {
      assigned,
      expectedRevision,
      providerId,
      stage,
    }),
  );
}

/** 删除已由界面确认、且未分配给当前生成步骤的 AI 服务。 */
export async function deleteProviderConfig(
  providerId: string,
  expectedRevision: number,
): Promise<ProviderConfigDeleteResult> {
  return decodeProviderConfigDeleteResult(
    await invoke<unknown>("delete_provider_config", {
      providerId,
      expectedRevision,
      confirmed: true,
    }),
  );
}

/** 发起一次用户明确要求的真实连接测试。 */
export async function verifyProviderConnection(
  providerId: string,
  stage: "article" | "summary",
): Promise<ProviderConnectionResult> {
  return decodeProviderConnectionResult(
    await invoke<unknown>("verify_provider_connection", { providerId, stage }),
  );
}

/** 在用户确认可能计费后，以当前已保存配置生成并校验一张测试图片。 */
export async function testProviderImage(
  providerId: string,
  expectedRevision: number,
): Promise<ProviderImageTestResult> {
  return decodeProviderImageTestResult(
    await invoke<unknown>("test_provider_image", {
      confirmed: true,
      expectedRevision,
      providerId,
    }),
    providerId,
    expectedRevision,
  );
}

/** 读取不含 CLI token 的云文档发布目标配置。 */
export async function listConnectorConfigs(): Promise<ConnectorConfigView[]> {
  return decodeConnectorConfigViews(
    await invoke<unknown>("list_connector_configs"),
  );
}

/** 读取 core 定义的新飞书 CLI 发布目标默认值。 */
export async function newFeishuConnectorConfigDraft(): Promise<ConnectorConfig> {
  return decodeConnectorConfig(
    await invoke<unknown>("new_feishu_connector_config_draft"),
  );
}

/** 保存发布目标；CLI 登录和 token 始终由外部官方 CLI 管理。 */
export async function saveConnectorConfig(
  config: ConnectorConfig,
): Promise<ConnectorConfigSaveResult> {
  return decodeConnectorConfigSaveResult(
    await invoke<unknown>("save_connector_config", { config }),
  );
}

/** 启停连接器，并用当前修订阻止陈旧页面覆盖新状态。 */
export async function setConnectorEnabled(
  connectorId: string,
  expectedRevision: number,
  enabled: boolean,
): Promise<ConnectorConfigSaveResult> {
  return decodeConnectorConfigSaveResult(
    await invoke<unknown>("set_connector_enabled", {
      connectorId,
      enabled,
      expectedRevision,
    }),
  );
}

/** 删除已由界面确认的连接器，并返回独立凭据清理状态。 */
export async function deleteConnectorConfig(
  connectorId: string,
  expectedRevision: number,
): Promise<ConnectorConfigDeleteResult> {
  return decodeConnectorConfigDeleteResult(
    await invoke<unknown>("delete_connector_config", {
      confirmed: true,
      connectorId,
      expectedRevision,
    }),
  );
}

/** 对目标飞书文件夹执行一次无写入的显式连接测试。 */
export async function verifyConnectorConnection(
  connectorId: string,
): Promise<ConnectorConnectionResult> {
  return decodeConnectorConnectionResult(
    await invoke<unknown>("verify_connector_connection", { connectorId }),
  );
}

/** 只读检查 CLI 版本、已有 profile 和所选 profile 的用户登录状态。 */
export async function inspectLarkCliSetup(
  profile: string,
): Promise<LarkCliSetupInspection> {
  return decodeLarkCliSetupInspection(
    await invoke<unknown>("inspect_lark_cli_setup", { profile }),
  );
}

/** 用户显式确认后，将当前文章包的最终文件同步到指定云文档连接器。 */
export async function syncArticleToConnector(
  taskId: string,
  connectorId: string,
  format: ConnectorDocumentFormat,
  expectedRowVersion: number,
  expectedPackageVersion: number,
  expectedConnectorRevision: number,
): Promise<ConnectorSyncResult> {
  return decodeConnectorSyncResult(
    await invoke<unknown>("sync_article_to_connector", {
      confirmed: true,
      connectorId,
      expectedConnectorRevision,
      expectedPackageVersion,
      expectedRowVersion,
      format,
      taskId,
    }),
    {
      connectorId,
      connectorRevision: expectedConnectorRevision,
      packageVersion: expectedPackageVersion,
      taskId,
    },
  );
}

/** 读取最近一百次批量运行记录。 */
export async function listRunHistory(): Promise<RunRecord[]> {
  return decodeRunRecords(await invoke<unknown>("list_run_history"));
}
