//! 工作台 GUI 动作到仓储、core 与 worker 的薄 Tauri command 映射。

use std::{collections::BTreeMap, sync::Arc};

use bifang_content_production_assistant_core::{
    ArtifactLineage, BindingProviderError, ImageEditLock, ImageEditSpec, PromptTemplate,
    ProviderConfigError, ProviderErrorKind, ProviderProbeError, StageQueueError, TemplateError,
    TextProbeSelection, WorkflowError, WorkflowStage, append_prompt_template_revision,
    apply_workspace_provider_config_update as apply_provider_config_update,
    endpoint_change_requires_key_rotation as should_rotate_provider_key,
    ensure_binding_providers_enabled, find_article_mut, find_execution_binding,
    inherited_stage_prompt_renderer_version, latest_prompt_templates,
    preflight_single_stage_request, prepare_provider_config, prepare_retry_batch,
    preview_selected_batch as preview_batch, queue_article_regeneration_in_workspace,
    queue_article_retry_in_workspace, queue_retry_selected_in_workspace,
    queue_selected_batch_in_workspace, queue_single_stage_in_workspace,
    remove_article_and_collect_unused_credentials, render_prompt, require_image_probe_confirmation,
    select_image_probe, select_text_probe,
};
use tauri::State;

use super::{
    image_editor::{
        ArticleImageEditorView, load_article_image_editor as load_image_editor,
        save_article_image_edit as save_image_edit,
    },
    models::{
        AddTitlesResult, ArticleView, BatchStartResult, CommandError, ImageRequestContract,
        PromptPreview, PromptTemplateView, ProviderConfig, ProviderConfigSaveResult,
        ProviderConfigView, ProviderConnectionResult, ProviderImageTestResult, ProviderKind,
        RunRecord, WorkbenchView, WorkspaceData, fixture_mode_enabled,
    },
    provider::{
        CredentialVault, OpenAiCompatibleClient, provider_endpoint,
        validate_ark_generation_endpoint,
    },
    store::{WorkspaceRepository, current_time_ms},
    worker::{BatchWorker, validated_image_generation_preview},
};

pub(crate) mod article;
pub(crate) mod batch;
mod errors;
pub(crate) mod prompts;
pub(crate) mod provider_config;
pub(crate) mod provider_probe;
pub(crate) mod stage;

#[cfg(test)]
mod tests;

#[cfg(test)]
use article::*;
use batch::*;
#[cfg(test)]
use bifang_content_production_assistant_core::ReviewStatus;
#[cfg(test)]
use bifang_content_production_assistant_core::single_stage_execution_binding;
use errors::*;
use provider_config::*;
#[cfg(test)]
use provider_probe::*;
#[cfg(test)]
use stage::*;
