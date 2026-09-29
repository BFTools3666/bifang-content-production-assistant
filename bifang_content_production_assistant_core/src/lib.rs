//! 批量 AI 内容工作台的接口无关领域核心。

#![deny(missing_docs)]

mod article_query;
mod connector;
mod image_edit;
mod initial_content;
mod model;
mod prompt;
mod provider;
pub mod public_services;
pub mod research;
mod sop;
mod workflow;
mod workspace;

pub use article_query::{
    ArticleDetail, ArticleHistoryCounts, ArticleList, ArticleListItem, ArticleReadArtifacts,
    ArticleReadError, ArticleReadFuture, ArticleReadRepository, ArticleWorkspaceSnapshot,
    CURRENT_WORKSPACE_SCHEMA_VERSION, get_article, list_articles, validate_article_id,
};
pub use connector::{
    ConnectorConfig, ConnectorConfigError, ConnectorConfigUpdate, ConnectorConfigUpdateError,
    ConnectorKind, DocumentFormat, DocumentSyncError, DocumentSyncRecord,
    apply_connector_config_update, build_self_contained_document, delete_connector_config,
    document_sync_file_name, migrate_connector_configs_to_cli, set_connector_config_enabled,
    validate_document_sync,
};
pub use image_edit::{
    IMAGE_EDIT_COORDINATE_SCALE, ImageCrop, ImageEditBlockedReason, ImageEditContext,
    ImageEditError, ImageEditLineage, ImageEditLock, ImageEditSpec,
};
pub use initial_content::{
    CURRENT_INITIAL_CONTENT_REVISION, InitialArticleInput, InitialContentDecision,
    UnsupportedInitialContentRevision, decide_initial_content, initial_article_inputs,
    initial_article_tasks, initial_prompt_templates, upgrade_legacy_default_prompts,
};
pub use model::{
    ApprovedPackage, ArticleArtifacts, ArticlePackageSnapshot, ArticleTask, ArtifactLineage,
    Failure, OverallStatus, ReviewStatus, StageAttempt, StageAttemptStatus, StageProgress,
    StageStates, StageStatus, TokenUsage, WorkflowStage,
};
pub use prompt::{
    CURRENT_PROMPT_RENDERER_VERSION, LEGACY_PROMPT_RENDERER_VERSION, PromptTemplate,
    PromptTemplateKind, TemplateError, extract_template_variables, render_prompt,
    render_prompt_with_version,
};
pub use provider::{
    CURRENT_PROVIDER_CONFIG_SCHEMA_VERSION, ImageGenerationFuture, ImageGenerationParameters,
    ImageGenerationRequest, ImageGenerationResult, ImageGenerator, ProviderCapability,
    ProviderConfig, ProviderConfigDeleteError, ProviderConfigError, ProviderConfigMigration,
    ProviderConfigMigrationError, ProviderConfigUpdate, ProviderConfigUpdateError, ProviderError,
    ProviderErrorKind, ProviderKind, ProviderParameterError, TextGenerationFuture,
    TextGenerationParameters, TextGenerationRequest, TextGenerationResult, TextGenerator,
    apply_provider_config_update, delete_provider_config, set_provider_config_enabled,
    set_provider_stage_assignment,
};
pub use sop::{
    WorkflowDefinition, WorkflowOutputDefinition, WorkflowOutputKind, WorkflowReviewGate,
    WorkflowStageDefinition, describe_fixed_content_sop,
};
pub use workflow::{
    ArtifactOutput, BatchPreparation, SkippedTask, WorkflowError, prepare_retry_batch,
    prepare_selected_batch, preview_selected_batch,
};
pub use workspace::{
    AddedArticleTitles, ArkProbePlan, ArticleTitleError, BindingProviderError,
    ConnectorSelectionError, DeletedProviderCredentialState, DocumentSyncSelection,
    DocumentUploadSizeError, ExecutionConfigurationError, ExecutionPlan, ImageProbePlan, ImageRequestContract,
    MAX_FEISHU_FILE_BYTES, PackageExecutionBinding, PromptWorkspaceError, ProviderExecutionBinding,
    ProviderProbeError, RunRecord, RunStatus, StageClaim, StageExecutionPlan,
    StageGenerationParameters, StageQueueError, StageRequestError, TextProbePlan,
    TextProbeSelection, WORKSPACE_SCHEMA_VERSION, WorkspaceData, append_article_titles,
    append_prompt_template_revision, apply_connector_update_in_workspace,
    apply_workspace_provider_config_update,
    cancel_pending_connector_credential_deletion_if_referenced, cancel_task_in_workspace,
    claim_next_stage_in_workspace, connector_for_connection_test, default_provider_for_stage,
    delete_connector_in_workspace, delete_provider_in_workspace, derive_terminal_run_status,
    endpoint_change_requires_key_rotation, ensure_ark_probe_credential_ready,
    ensure_binding_providers_enabled, execution_binding_uses_credential, find_article_mut,
    find_execution_binding, image_request_idempotency_material,
    inherited_stage_prompt_renderer_version, latest_prompt_templates, manual_lineage,
    mark_run_started_in_workspace, merge_stage_execution_binding, parse_article_titles,
    preflight_single_stage_request, prepare_connector_config_for_save, prepare_provider_config,
    provider_kind_allowed, queue_article_regeneration_in_workspace,
    queue_article_retry_in_workspace, queue_credential_deletion_if_unused,
    queue_retry_selected_in_workspace, queue_selected_batch_in_workspace,
    queue_single_stage_in_workspace, record_queued_run, record_queued_stage_run,
    record_uploaded_document_sync, recover_abandoned_runs, recover_interrupted_articles,
    recover_interrupted_run_articles, referenced_connector_credentials,
    refresh_run_counts_in_workspace, remove_article_and_collect_unused_credentials,
    render_stage_prompt, require_ark_probe_paid_confirmation, require_image_probe_confirmation,
    reusable_sync_record, run_uses_fixture_for_mode, select_ark_probe_provider,
    select_document_sync, select_image_probe, select_text_probe,
    set_connector_enabled_in_workspace, set_provider_enabled_in_workspace,
    set_provider_stage_assignment_in_workspace, single_stage_execution_binding,
    source_article_version, source_summary_version, terminal_run_status, terminal_stage_run_status,
    transition_controlled_run_status_in_workspace, upsert_execution_binding,
    validate_connector_delete_confirmation, validate_execution_configuration_for_kind,
    validate_execution_providers, validate_prompt_renderer_version, validate_single_stage_request,
    validate_upload_size,
};
