//! Provider 配置草稿和工作区凭据引用的原子规则。

use std::collections::BTreeSet;

use super::{ProviderConfig, ProviderKind, WorkspaceData};
use crate::{
    PromptTemplateKind, ProviderConfigError,
    apply_provider_config_update as update_provider_collection,
    delete_provider_config as delete_provider_collection,
    set_provider_config_enabled as set_provider_enabled,
    set_provider_stage_assignment as set_stage_assignment,
};

/// 删除 Provider 后，需要清理的凭据和已解除秘密引用的历史绑定数量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedProviderCredentialState {
    /// 需要复核并清理的不透明系统凭据引用。
    pub credential_refs: BTreeSet<String>,
    /// 已解除秘密引用的历史绑定数量。
    pub historical_bindings_detached: u32,
}

/// 规范化输入并生成缺失的稳定 ID；ID 由调用方注入以便接口无关。
pub fn prepare_provider_config(
    mut config: ProviderConfig,
    generated_id: &str,
) -> Result<ProviderConfig, ProviderConfigError> {
    config.name = config.name.trim().to_string();
    config.base_url = config.base_url.trim().to_string();
    config.text_model = config.text_model.trim().to_string();
    config.article_model = config.article_model.map(|model| model.trim().to_string());
    config.summary_model = config.summary_model.map(|model| model.trim().to_string());
    config.image_model = config.image_model.trim().to_string();
    if config.name.is_empty() {
        return Err(ProviderConfigError::NameRequired);
    }
    if config.id.trim().is_empty() {
        config.id = generated_id.to_string();
    }
    config.materialize_stage_configuration();
    config.text_model = if !config.article_model().trim().is_empty() {
        config.article_model().to_string()
    } else {
        config.summary_model().to_string()
    };
    config.validate_for_persistence()?;
    Ok(config)
}

/// 地址或协议变化且旧配置确实持有密钥时，必须同步轮换。
pub const fn endpoint_change_requires_key_rotation(
    old_key_is_present: bool,
    transport_changed: bool,
) -> bool {
    old_key_is_present && transport_changed
}

/// 原子替换 Provider，并更新当前密钥引用与延迟清理队列。
pub fn apply_workspace_provider_config_update(
    workspace: &mut WorkspaceData,
    requested: ProviderConfig,
    new_credential_ref: Option<&str>,
) -> Result<(ProviderConfig, String, Option<String>), String> {
    if workspace
        .provider_configs
        .iter()
        .find(|provider| provider.id == requested.id)
        .is_some_and(|provider| provider.revision != requested.revision)
    {
        return Err("PROVIDER_REVISION_CONFLICT".into());
    }
    let previous_credential_ref = if new_credential_ref.is_some() {
        Some(
            workspace
                .provider_credential_refs
                .get(&requested.id)
                .cloned()
                .unwrap_or_else(|| requested.id.clone()),
        )
    } else {
        None
    };
    let update = update_provider_collection(
        &mut workspace.provider_configs,
        &mut workspace.provider_config_history,
        requested,
    )
    .map_err(|error| error.code().to_string())?;
    let superseded_credential_ref = update
        .replaced_existing
        .then_some(previous_credential_ref)
        .flatten();
    if let Some(credential_ref) = new_credential_ref {
        workspace
            .provider_credential_refs
            .insert(update.saved.id.clone(), credential_ref.to_string());
        workspace
            .pending_credential_deletions
            .remove(credential_ref);
    }
    let active_credential_ref = workspace
        .provider_credential_refs
        .get(&update.saved.id)
        .cloned()
        .unwrap_or_else(|| update.saved.id.clone());
    if let Some(superseded) = superseded_credential_ref
        .as_deref()
        .filter(|superseded| *superseded != active_credential_ref)
    {
        queue_credential_deletion_if_unused(workspace, superseded);
    }
    Ok((
        update.saved,
        active_credential_ref,
        superseded_credential_ref,
    ))
}

/// 只有当前配置与所有冻结文章包都不再使用该引用时才登记删除。
pub fn queue_credential_deletion_if_unused(workspace: &mut WorkspaceData, credential_ref: &str) {
    if !workspace
        .referenced_credential_refs()
        .contains(credential_ref)
    {
        workspace
            .pending_credential_deletions
            .insert(credential_ref.to_string());
    }
}

fn current_provider(
    workspace: &WorkspaceData,
    provider_id: &str,
    expected_revision: u32,
) -> Result<(), String> {
    let current = workspace
        .provider_configs
        .iter()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| "PROVIDER_NOT_FOUND".to_string())?;
    if current.kind == ProviderKind::Fixture {
        return Err("FIXTURE_DISABLED".into());
    }
    if current.revision != expected_revision {
        return Err("PROVIDER_REVISION_CONFLICT".into());
    }
    Ok(())
}

/// 按修订锁启停已保存 Provider，并同步历史修订。
pub fn set_provider_enabled_in_workspace(
    workspace: &mut WorkspaceData,
    provider_id: &str,
    expected_revision: u32,
    enabled: bool,
) -> Result<ProviderConfig, String> {
    current_provider(workspace, provider_id, expected_revision)?;
    set_provider_enabled(
        &mut workspace.provider_configs,
        &mut workspace.provider_config_history,
        provider_id,
        enabled,
    )
    .map(|update| update.saved)
    .map_err(|error| error.code().to_string())
}

/// 按修订锁分配或解除单个 Provider 步骤，并同步历史修订。
pub fn set_provider_stage_assignment_in_workspace(
    workspace: &mut WorkspaceData,
    provider_id: &str,
    expected_revision: u32,
    stage: PromptTemplateKind,
    assigned: bool,
) -> Result<ProviderConfig, String> {
    current_provider(workspace, provider_id, expected_revision)?;
    set_stage_assignment(
        &mut workspace.provider_configs,
        &mut workspace.provider_config_history,
        provider_id,
        stage,
        assigned,
    )
    .map(|update| update.saved)
    .map_err(|error| error.code().to_string())
}

/// 删除未分配的 Provider，保留历史配置但解除历史绑定中的秘密引用。
pub fn delete_provider_in_workspace(
    workspace: &mut WorkspaceData,
    provider_id: &str,
    expected_revision: u32,
) -> Result<DeletedProviderCredentialState, String> {
    current_provider(workspace, provider_id, expected_revision)?;
    delete_provider_collection(
        &mut workspace.provider_configs,
        &mut workspace.provider_config_history,
        provider_id,
    )
    .map_err(|error| error.code().to_string())?;
    let mut credential_refs = workspace
        .provider_credential_refs
        .remove(provider_id)
        .or_else(|| Some(provider_id.to_string()))
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut historical_bindings_detached = 0_u32;
    for binding in &mut workspace.execution_bindings {
        for bound in [
            &mut binding.article_provider,
            &mut binding.summary_provider,
            &mut binding.image_provider,
        ] {
            let Some(bound) = bound.as_mut() else {
                continue;
            };
            if bound.config.id != provider_id {
                continue;
            }
            if let Some(reference) = bound.credential_ref.take() {
                credential_refs.insert(reference);
            }
            historical_bindings_detached = historical_bindings_detached.saturating_add(1);
        }
    }
    let retained_refs = workspace.referenced_credential_refs();
    for reference in &credential_refs {
        if !retained_refs.contains(reference) {
            workspace
                .pending_credential_deletions
                .insert(reference.clone());
        }
    }
    Ok(DeletedProviderCredentialState {
        credential_refs,
        historical_bindings_detached,
    })
}
