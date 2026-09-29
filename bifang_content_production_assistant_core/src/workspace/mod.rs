//! 文章工作区快照、冻结执行契约与兼容迁移。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    ArticleArtifacts, ArticleTask, CURRENT_INITIAL_CONTENT_REVISION,
    CURRENT_WORKSPACE_SCHEMA_VERSION, ConnectorConfig, DocumentSyncRecord,
    ImageGenerationParameters, InitialContentDecision, LEGACY_PROMPT_RENDERER_VERSION,
    OverallStatus, PromptTemplate, ProviderConfig, ProviderConfigMigration, ProviderKind,
    StageStatus, TextGenerationParameters, WorkflowStage, decide_initial_content,
    initial_article_tasks, initial_prompt_templates, migrate_connector_configs_to_cli,
    upgrade_legacy_default_prompts,
};

mod article;
mod batch;
mod binding;
mod connector;
mod contract;
mod execution_plan;
mod migration;
mod prompt;
mod provider_config;
mod provider_probe;
mod recovery;
mod run;
mod snapshot;
mod stage;
mod stage_execution;
mod stage_request;

pub use article::{
    AddedArticleTitles, ArticleTitleError, append_article_titles, manual_lineage,
    parse_article_titles, remove_article_and_collect_unused_credentials,
};
pub use batch::{
    queue_article_regeneration_in_workspace, queue_article_retry_in_workspace,
    queue_retry_selected_in_workspace, queue_selected_batch_in_workspace,
};
pub use binding::{
    BindingProviderError, ensure_binding_providers_enabled, execution_binding_uses_credential,
    find_article_mut, find_execution_binding, merge_stage_execution_binding,
    single_stage_execution_binding, upsert_execution_binding,
};
pub use connector::{
    ConnectorSelectionError, DocumentSyncSelection, DocumentUploadSizeError, MAX_FEISHU_FILE_BYTES,
    apply_connector_update_in_workspace,
    cancel_pending_connector_credential_deletion_if_referenced, connector_for_connection_test,
    delete_connector_in_workspace, prepare_connector_config_for_save,
    record_uploaded_document_sync, referenced_connector_credentials, reusable_sync_record,
    select_document_sync, set_connector_enabled_in_workspace,
    validate_connector_delete_confirmation, validate_upload_size,
};
pub use contract::{ImageRequestContract, PackageExecutionBinding, ProviderExecutionBinding};
pub use execution_plan::{
    ExecutionPlan, provider_kind_allowed, validate_execution_configuration_for_kind,
    validate_execution_providers, validate_prompt_renderer_version,
};
pub use prompt::{PromptWorkspaceError, append_prompt_template_revision, latest_prompt_templates};
pub use provider_config::{
    DeletedProviderCredentialState, apply_workspace_provider_config_update,
    delete_provider_in_workspace, endpoint_change_requires_key_rotation, prepare_provider_config,
    queue_credential_deletion_if_unused, set_provider_enabled_in_workspace,
    set_provider_stage_assignment_in_workspace,
};
pub use provider_probe::{
    ArkProbePlan, ImageProbePlan, ProviderProbeError, TextProbePlan, TextProbeSelection,
    ensure_ark_probe_credential_ready, require_ark_probe_paid_confirmation,
    require_image_probe_confirmation, select_ark_probe_provider, select_image_probe,
    select_text_probe,
};
pub use recovery::{recover_abandoned_runs, recover_interrupted_articles};
pub use run::{
    RunRecord, RunStatus, StageClaim, cancel_task_in_workspace, claim_next_stage_in_workspace,
    default_provider_for_stage, derive_terminal_run_status, mark_run_started_in_workspace,
    record_queued_run, record_queued_stage_run, recover_interrupted_run_articles,
    refresh_run_counts_in_workspace, run_uses_fixture_for_mode, terminal_run_status,
    terminal_stage_run_status, transition_controlled_run_status_in_workspace,
};
pub use snapshot::WorkspaceData;
pub use stage::{
    StageQueueError, inherited_stage_prompt_renderer_version, preflight_single_stage_request,
    queue_single_stage_in_workspace,
};
pub use stage_execution::{
    StageExecutionPlan, StageGenerationParameters, image_request_idempotency_material,
    render_stage_prompt, source_article_version, source_summary_version,
};
pub use stage_request::{StageRequestError, validate_single_stage_request};

use contract::legacy_image_request_contract;

/// v3 新建绑定使用的 Prompt renderer；首版 renderer 的 v3 执行历史存在歧义。
const V3_NEW_BINDING_PROMPT_RENDERER_VERSION: u32 = 2;

/// 首次支持火山方舟图片协议的工作区模式版本。
const VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION: u32 = 6;

/// v7 已完成火山方舟配置迁移，但尚未持久化 AI 原图引用。
const PRE_IMAGE_EDITOR_SCHEMA_VERSION: u32 = 7;

/// v8 已持久化 AI 原图，但尚未持久化创建时间与逐字段 token 用量。
const PRE_GENERATION_METRICS_SCHEMA_VERSION: u32 = 8;

/// v9 已持久化生成指标，但尚未包含云文档连接器。
const PRE_CONNECTOR_SCHEMA_VERSION: u32 = 9;

/// v10 使用 App ID/App Secret 直连飞书，尚未改用官方 CLI profile。
const PRE_LARK_CLI_CONNECTOR_SCHEMA_VERSION: u32 = 10;

/// 旧版把火山方舟图片端点误存成 OpenAI-compatible 根地址时使用的完整地址。
const LEGACY_VOLCENGINE_ARK_IMAGE_ENDPOINT: &str =
    "https://ark.cn-beijing.volces.com/api/v3/images/generations";

/// 火山方舟图片 Provider 当前持久化的 API 根地址。
const VOLCENGINE_ARK_API_ROOT: &str = "https://ark.cn-beijing.volces.com/api/v3";

/// 当前可持久化工作区快照模式版本。
///
/// v11 把飞书连接器迁移为官方 CLI 发布目标，并清理旧秘密引用。
pub const WORKSPACE_SCHEMA_VERSION: u32 = CURRENT_WORKSPACE_SCHEMA_VERSION;

#[cfg(test)]
mod tests;
