//! AI Provider 步骤分配、启停与安全删除的薄适配层。

use std::sync::Arc;

use bifang_content_production_assistant_core::{
    PromptTemplateKind, delete_provider_in_workspace, set_provider_enabled_in_workspace,
    set_provider_stage_assignment_in_workspace,
};
use tauri::State;

use super::{
    commands::provider_config::{provider_config_views, reconcile_pending_credential_deletions},
    models::{
        CommandError, ProviderConfigDeleteResult, ProviderConfigSaveResult, fixture_mode_enabled,
    },
    store::WorkspaceRepository,
    worker::BatchWorker,
};

#[cfg(test)]
use super::models::{ProviderConfig, ProviderKind, WorkspaceData};

/// 启用或停用一个已保存的 AI 服务；停用会原子解除全部默认位。
#[tauri::command]
pub async fn set_provider_enabled(
    provider_id: String,
    expected_revision: u32,
    enabled: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ProviderConfigSaveResult, CommandError> {
    reject_fixture_mutation()?;
    let _start_guard = worker.acquire_start().await;
    let saved = repository
        .update(|workspace| {
            set_provider_enabled_in_workspace(workspace, &provider_id, expected_revision, enabled)
        })
        .await
        .map_err(provider_lifecycle_error)?;
    let workspace = repository.snapshot().await;
    let providers = provider_config_views(&workspace);
    let saved = providers
        .iter()
        .find(|provider| provider.config.id == saved.id)
        .cloned()
        .ok_or_else(|| CommandError::new("STORE_ERROR", "更新后的 AI 服务无法复读"))?;
    Ok(ProviderConfigSaveResult { saved, providers })
}

/// 显式把一个已启用服务分配给单个生成步骤，或只解除该步骤的分配。
#[tauri::command]
pub async fn set_provider_stage_assignment(
    provider_id: String,
    expected_revision: u32,
    stage: PromptTemplateKind,
    assigned: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ProviderConfigSaveResult, CommandError> {
    reject_fixture_mutation()?;
    let _start_guard = worker.acquire_start().await;
    let saved = repository
        .update(|workspace| {
            set_provider_stage_assignment_in_workspace(
                workspace,
                &provider_id,
                expected_revision,
                stage,
                assigned,
            )
        })
        .await
        .map_err(provider_lifecycle_error)?;
    let workspace = repository.snapshot().await;
    let providers = provider_config_views(&workspace);
    let saved = providers
        .iter()
        .find(|provider| provider.config.id == saved.id)
        .cloned()
        .ok_or_else(|| CommandError::new("STORE_ERROR", "更新后的 AI 服务无法复读"))?;
    Ok(ProviderConfigSaveResult { saved, providers })
}

/// 删除一个未分配给当前步骤的 AI 服务，并保留历史执行快照。
#[tauri::command]
pub async fn delete_provider_config(
    provider_id: String,
    expected_revision: u32,
    confirmed: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ProviderConfigDeleteResult, CommandError> {
    reject_fixture_mutation()?;
    if !confirmed {
        return Err(CommandError::new(
            "PROVIDER_DELETE_CONFIRMATION_REQUIRED",
            "删除 AI 服务前必须明确确认",
        ));
    }
    let _start_guard = worker.acquire_start().await;
    let credential_state = repository
        .update(|workspace| {
            delete_provider_in_workspace(workspace, &provider_id, expected_revision)
        })
        .await
        .map_err(provider_lifecycle_error)?;
    reconcile_pending_credential_deletions(&repository).await;
    let workspace = repository.snapshot().await;
    let credential_cleanup_pending = credential_state
        .credential_refs
        .iter()
        .any(|reference| workspace.pending_credential_deletions.contains(reference));
    Ok(ProviderConfigDeleteResult {
        providers: provider_config_views(&workspace),
        credential_cleanup_pending,
        historical_bindings_detached: credential_state.historical_bindings_detached,
    })
}

fn reject_fixture_mutation() -> Result<(), CommandError> {
    if fixture_mode_enabled() {
        Err(CommandError::new(
            "FIXTURE_READ_ONLY",
            "测试夹具模式下 AI 服务配置为只读",
        ))
    } else {
        Ok(())
    }
}

fn provider_lifecycle_error(message: String) -> CommandError {
    let safe_message = match message.as_str() {
        "PROVIDER_NOT_FOUND" => "找不到指定 AI 服务",
        "PROVIDER_REVISION_CONFLICT" => "AI 服务已被其他操作更新，请刷新后重试",
        "PROVIDER_REVISION_EXHAUSTED" => "AI 服务修订号已达到上限",
        "PROVIDER_DISABLED_DEFAULT" => "已停用的 AI 服务不能分配给生成步骤",
        "PROVIDER_DEFAULT_MODEL_REQUIRED" => "请先填写当前步骤使用的模型",
        "PROVIDER_STAGE_UNSUPPORTED" => "这个 AI 服务不支持当前生成步骤",
        "PROVIDER_PARAMETER_INVALID" => "AI 服务生成参数无效",
        "PROVIDER_DELETE_DEFAULT_IN_USE" => "该服务仍被生成步骤使用，请先从对应步骤移除",
        "FIXTURE_DISABLED" => "正式界面不能修改测试 Provider",
        _ => return CommandError::new("STORE_ERROR", message),
    };
    CommandError::new(message, safe_message)
}

#[cfg(test)]
mod tests {
    use bifang_content_production_assistant_core::{
        ImageGenerationParameters, TextGenerationParameters,
    };

    use super::*;
    use crate::workbench::models::{
        ImageRequestContract, PackageExecutionBinding, ProviderExecutionBinding,
    };

    fn provider(id: &str) -> ProviderConfig {
        ProviderConfig {
            id: id.into(),
            name: id.into(),
            revision: 3,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://api.openai.com/v1".into(),
            text_model: "gpt-4o-mini".into(),
            article_model: Some("gpt-4o-mini".into()),
            summary_model: Some("gpt-4o-mini".into()),
            image_model: "gpt-image-2.5-flare".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: false,
            default_for_summary: false,
            default_for_image: false,
        }
    }

    #[test]
    fn toggle_checks_revision_and_clears_defaults() {
        let mut workspace = WorkspaceData::default();
        let mut config = provider("provider");
        config.default_for_article = true;
        workspace.provider_configs.push(config);

        assert_eq!(
            set_provider_enabled_in_workspace(&mut workspace, "provider", 2, false),
            Err("PROVIDER_REVISION_CONFLICT".into())
        );
        let saved =
            set_provider_enabled_in_workspace(&mut workspace, "provider", 3, false).unwrap();
        assert!(!saved.enabled);
        assert!(!saved.default_for_article);
        assert_eq!(saved.revision, 4);
        assert_eq!(workspace.provider_config_history.len(), 1);
    }

    #[test]
    fn stage_assignment_checks_revision_and_preserves_non_assignment_state() {
        let mut workspace = WorkspaceData::default();
        let mut first = provider("first");
        first.default_for_article = true;
        let second = provider("second");
        workspace.provider_configs = vec![first, second];
        workspace
            .provider_credential_refs
            .insert("second".into(), "credential-r3".into());
        workspace
            .pending_credential_deletions
            .insert("unrelated-pending".into());
        let refs_before = workspace.provider_credential_refs.clone();
        let pending_before = workspace.pending_credential_deletions.clone();
        let bindings_before = workspace.execution_bindings.clone();

        assert_eq!(
            set_provider_stage_assignment_in_workspace(
                &mut workspace,
                "second",
                2,
                PromptTemplateKind::Article,
                true,
            ),
            Err("PROVIDER_REVISION_CONFLICT".into())
        );
        let saved = set_provider_stage_assignment_in_workspace(
            &mut workspace,
            "second",
            3,
            PromptTemplateKind::Article,
            true,
        )
        .unwrap();
        assert!(saved.default_for_article);
        assert_eq!(saved.revision, 4);
        assert!(!workspace.provider_configs[0].default_for_article);
        assert_eq!(workspace.provider_credential_refs, refs_before);
        assert_eq!(workspace.pending_credential_deletions, pending_before);
        assert_eq!(workspace.execution_bindings, bindings_before);

        let removed = set_provider_stage_assignment_in_workspace(
            &mut workspace,
            "second",
            4,
            PromptTemplateKind::Article,
            false,
        )
        .unwrap();
        assert!(!removed.default_for_article);
        assert_eq!(removed.article_model(), "gpt-4o-mini");
        assert_eq!(workspace.provider_credential_refs, refs_before);
        assert_eq!(workspace.pending_credential_deletions, pending_before);
        assert_eq!(workspace.execution_bindings, bindings_before);
    }

    #[test]
    fn delete_preserves_historical_config_but_detaches_its_credential() {
        let mut workspace = WorkspaceData::default();
        let config = provider("provider");
        let mut historical_config = config.clone();
        historical_config.revision = 2;
        let other = provider("other");
        workspace.provider_configs = vec![config.clone(), other.clone()];
        workspace
            .provider_credential_refs
            .insert(config.id.clone(), "credential-r3".into());
        workspace
            .provider_credential_refs
            .insert(other.id.clone(), "credential-other".into());
        workspace.execution_bindings.push(PackageExecutionBinding {
            task_id: "task".into(),
            package_version: 1,
            prompt_renderer_version: 2,
            image_request_contract: Some(ImageRequestContract::CURRENT),
            article_template: workspace.prompt_templates[0].clone(),
            summary_template: workspace.prompt_templates[1].clone(),
            image_template: workspace.prompt_templates[2].clone(),
            article_provider: Some(ProviderExecutionBinding {
                config: config.clone(),
                credential_ref: Some("credential-r3".into()),
            }),
            summary_provider: Some(ProviderExecutionBinding {
                config: historical_config.clone(),
                credential_ref: Some("credential-r2".into()),
            }),
            image_provider: Some(ProviderExecutionBinding {
                config: other.clone(),
                credential_ref: Some("credential-other".into()),
            }),
        });
        let deleted = delete_provider_in_workspace(&mut workspace, "provider", 3).unwrap();
        assert_eq!(
            deleted.credential_refs,
            std::collections::BTreeSet::from(["credential-r2".into(), "credential-r3".into(),])
        );
        assert_eq!(deleted.historical_bindings_detached, 2);
        assert_eq!(workspace.provider_configs, vec![other.clone()]);
        assert!(!workspace.provider_credential_refs.contains_key("provider"));
        assert_eq!(
            workspace
                .provider_credential_refs
                .get("other")
                .map(String::as_str),
            Some("credential-other")
        );
        assert!(
            workspace
                .pending_credential_deletions
                .contains("credential-r2")
        );
        assert!(
            workspace
                .pending_credential_deletions
                .contains("credential-r3")
        );
        assert!(
            !workspace
                .pending_credential_deletions
                .contains("credential-other")
        );
        let binding = &workspace.execution_bindings[0];
        let historical_article = binding.article_provider.as_ref().unwrap();
        assert_eq!(historical_article.config, config);
        assert_eq!(historical_article.credential_ref, None);
        let historical_summary = binding.summary_provider.as_ref().unwrap();
        assert_eq!(historical_summary.config, historical_config);
        assert_eq!(historical_summary.credential_ref, None);
        assert_eq!(
            binding.image_provider,
            Some(ProviderExecutionBinding {
                config: other,
                credential_ref: Some("credential-other".into()),
            })
        );
        assert_eq!(workspace.provider_config_history.len(), 1);
    }

    #[test]
    fn delete_queues_an_unreferenced_credential() {
        let mut workspace = WorkspaceData::default();
        workspace.provider_configs.push(provider("provider"));
        workspace
            .provider_credential_refs
            .insert("provider".into(), "credential-r3".into());

        let deleted = delete_provider_in_workspace(&mut workspace, "provider", 3).unwrap();
        assert_eq!(
            deleted.credential_refs,
            std::collections::BTreeSet::from(["credential-r3".into()])
        );
        assert_eq!(deleted.historical_bindings_detached, 0);
        assert!(workspace.provider_configs.is_empty());
        assert!(
            workspace
                .pending_credential_deletions
                .contains("credential-r3")
        );
    }
}
