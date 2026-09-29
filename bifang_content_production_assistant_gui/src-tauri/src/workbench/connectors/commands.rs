//! 连接器配置生命周期、连接测试与最终文件同步命令。

use std::{collections::BTreeSet, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bifang_content_production_assistant_core::{
    ConnectorConfig, ConnectorSelectionError, DocumentFormat, DocumentSyncError,
    DocumentSyncRecord, DocumentSyncSelection,
    apply_connector_update_in_workspace as apply_core_connector_update_in_workspace,
    build_self_contained_document, cancel_pending_connector_credential_deletion_if_referenced,
    connector_for_connection_test,
    delete_connector_in_workspace as delete_core_connector_in_workspace, document_sync_file_name,
    prepare_connector_config_for_save, record_uploaded_document_sync,
    referenced_connector_credentials as core_referenced_connector_credentials,
    select_document_sync, set_connector_enabled_in_workspace,
    validate_connector_delete_confirmation, validate_upload_size as core_validate_upload_size,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::MutexGuard;

use super::{
    credentials::ConnectorCredentialVault,
    lark_cli::{LarkCliClient, LarkCliError},
};
use crate::workbench::{
    models::{CommandError, WorkspaceData, fixture_mode_enabled},
    store::{WorkspaceRepository, current_time_ms},
    worker::BatchWorker,
};

const CLI_SETUP_INSPECTION_TIMEOUT_MS: u64 = 30_000;

#[cfg(test)]
pub(super) use bifang_content_production_assistant_core::MAX_FEISHU_FILE_BYTES;
#[cfg(test)]
use bifang_content_production_assistant_core::reusable_sync_record as core_reusable_sync_record;

/// 连接器网络调用和配置/凭据切换的单一进程内所有者。
pub struct ConnectorRuntime {
    client: LarkCliClient,
    operation_lock: tokio::sync::Mutex<()>,
}

impl ConnectorRuntime {
    /// 建立固定调用官方 `lark-cli` 的连接器运行时。
    pub fn new() -> Self {
        Self {
            client: LarkCliClient::new(),
            operation_lock: tokio::sync::Mutex::new(()),
        }
    }
}

/// 不返回 CLI token 的完整发布目标配置视图。
pub type ConnectorConfigView = ConnectorConfig;

/// 一个可以安全展示的 CLI profile；不会包含 app ID、密钥或 token 正文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LarkCliProfileInspection {
    pub name: String,
    pub user_name: Option<String>,
    pub token_status: Option<String>,
}

/// 飞书 CLI 引导页的一次只读检查结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LarkCliSetupInspection {
    pub version: String,
    pub profiles: Vec<LarkCliProfileInspection>,
    pub selected_profile_configured: bool,
    pub selected_profile_authorized: bool,
    pub selected_user_name: Option<String>,
    pub authorization_issue_code: Option<String>,
}

/// 保存或启停连接器后的完整当前集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConfigSaveResult {
    /// 被创建或修订的连接器。
    pub saved: ConnectorConfigView,
    /// 原子更新后的全部当前连接器。
    pub connectors: Vec<ConnectorConfigView>,
}

/// 删除连接器后的当前集合和秘密清理状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConfigDeleteResult {
    /// 删除后的全部当前连接器。
    pub connectors: Vec<ConnectorConfigView>,
    /// 系统凭据清理是否仍在可重试队列中。
    pub credential_cleanup_pending: bool,
}

/// 显式只读连接测试的安全结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConnectionResult {
    /// 目标文件夹是否已通过只读列举验证。
    pub ok: bool,
    /// 不含凭据、token 或远端响应正文的说明。
    pub message: String,
}

/// 同步成功后返回的稳定文件身份；幂等命中时 `reused` 为真。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSyncResult {
    /// 文章稳定 ID。
    pub task_id: String,
    /// 已同步的文章包版本。
    pub package_version: u32,
    /// 连接器稳定 ID。
    pub connector_id: String,
    /// 使用的连接器修订。
    pub connector_revision: u32,
    /// 上传的最终格式。
    pub format: DocumentFormat,
    /// 飞书云空间文件 token。
    pub file_token: String,
    /// 上传使用的文件名。
    pub file_name: String,
    /// 首次同步成功时间，Unix 毫秒。
    pub synced_at_ms: i64,
    /// 是否复用了完全相同范围的既有成功记录。
    pub reused: bool,
}

/// 返回由 core 定义、尚无稳定 ID 或 CLI 凭据的飞书发布目标草稿。
#[tauri::command]
pub async fn new_feishu_connector_config_draft() -> ConnectorConfig {
    ConnectorConfig::feishu_drive_draft()
}

/// 返回全部非秘密发布目标配置。
#[tauri::command]
pub async fn list_connector_configs(
    repository: State<'_, Arc<WorkspaceRepository>>,
) -> Result<Vec<ConnectorConfigView>, CommandError> {
    Ok(connector_views(&repository.snapshot().await))
}

/// 检查本机 CLI、profile 列表和所选 profile 的用户授权，不修改共享 CLI 状态。
#[tauri::command]
pub async fn inspect_lark_cli_setup(
    profile: String,
    runtime: State<'_, ConnectorRuntime>,
) -> Result<LarkCliSetupInspection, CommandError> {
    let _operation_guard = runtime.operation_lock.lock().await;
    let snapshot = runtime
        .client
        .inspect_setup(&profile, CLI_SETUP_INSPECTION_TIMEOUT_MS)
        .await
        .map_err(lark_cli_error)?;
    Ok(LarkCliSetupInspection {
        version: snapshot.version,
        profiles: snapshot
            .profiles
            .into_iter()
            .map(|profile| LarkCliProfileInspection {
                name: profile.name,
                user_name: profile.user_name,
                token_status: profile.token_status,
            })
            .collect(),
        selected_profile_configured: snapshot.selected_profile_configured,
        selected_profile_authorized: snapshot.selected_profile_authorized,
        selected_user_name: snapshot.selected_user_name,
        authorization_issue_code: snapshot.authorization_issue_code.map(str::to_string),
    })
}

/// 校验并保存发布目标配置；保存本身不会调用 CLI 或访问飞书。
#[tauri::command]
pub async fn save_connector_config(
    config: ConnectorConfig,
    repository: State<'_, Arc<WorkspaceRepository>>,
    runtime: State<'_, ConnectorRuntime>,
) -> Result<ConnectorConfigSaveResult, CommandError> {
    let _operation_guard = runtime.operation_lock.lock().await;
    let config =
        prepare_connector_config_for_save(config, format!("connector-{}", uuid::Uuid::new_v4()))
            .map_err(|error| CommandError::new(error.code(), error.to_string()))?;

    let saved_config = repository
        .update(|workspace| apply_connector_update_in_workspace(workspace, config.clone()))
        .await
        .map_err(connector_update_error)?;
    let workspace = repository.snapshot().await;
    let connectors = connector_views(&workspace);
    let saved = connectors
        .iter()
        .find(|connector| connector.id == saved_config.id)
        .cloned()
        .ok_or_else(|| store_error("保存后的连接器无法复读".into()))?;
    Ok(ConnectorConfigSaveResult { saved, connectors })
}

/// 以乐观锁启用或停用一个连接器。
#[tauri::command]
pub async fn set_connector_enabled(
    connector_id: String,
    expected_revision: u32,
    enabled: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    runtime: State<'_, ConnectorRuntime>,
) -> Result<ConnectorConfigSaveResult, CommandError> {
    let _operation_guard = runtime.operation_lock.lock().await;
    let saved_config = repository
        .update(|workspace| {
            set_connector_enabled_in_workspace(workspace, &connector_id, expected_revision, enabled)
                .map_err(|error| error.code().to_string())
        })
        .await
        .map_err(connector_update_error)?;
    let workspace = repository.snapshot().await;
    let connectors = connector_views(&workspace);
    let saved = connectors
        .iter()
        .find(|connector| connector.id == saved_config.id)
        .cloned()
        .ok_or_else(|| store_error("更新后的连接器无法复读".into()))?;
    Ok(ConnectorConfigSaveResult { saved, connectors })
}

/// 显式确认后删除发布目标；共享 CLI profile 始终不受影响。
#[tauri::command]
pub async fn delete_connector_config(
    connector_id: String,
    expected_revision: u32,
    confirmed: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    runtime: State<'_, ConnectorRuntime>,
) -> Result<ConnectorConfigDeleteResult, CommandError> {
    validate_connector_delete_confirmation(confirmed)
        .map_err(|message| CommandError::new("CONNECTOR_DELETE_CONFIRMATION_REQUIRED", message))?;
    let _operation_guard = runtime.operation_lock.lock().await;
    let removed_refs = repository
        .update(|workspace| {
            delete_connector_in_workspace(workspace, &connector_id, expected_revision)
        })
        .await
        .map_err(connector_update_error)?;
    let vault = ConnectorCredentialVault;
    reconcile_pending_connector_credential_deletions_inner(&repository, |reference| {
        vault.delete_app_secret(reference)
    })
    .await;
    let workspace = repository.snapshot().await;
    let credential_cleanup_pending = removed_refs.iter().any(|reference| {
        workspace
            .pending_connector_credential_deletions
            .contains(reference)
    });
    Ok(ConnectorConfigDeleteResult {
        connectors: connector_views(&workspace),
        credential_cleanup_pending,
    })
}

/// 使用显式 CLI profile 的用户身份，并只读列出目标文件夹。
#[tauri::command]
pub async fn verify_connector_connection(
    connector_id: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    runtime: State<'_, ConnectorRuntime>,
) -> Result<ConnectorConnectionResult, CommandError> {
    let _operation_guard = runtime.operation_lock.lock().await;
    let workspace = repository.snapshot().await;
    let connector =
        connector_for_connection_test(&workspace, &connector_id).map_err(selection_error)?;
    let version = runtime
        .client
        .verify_folder(
            &connector.cli_profile,
            &connector.folder_token,
            connector.timeout_ms,
        )
        .await
        .map_err(lark_cli_error)?;
    Ok(ConnectorConnectionResult {
        ok: true,
        message: format!("lark-cli v{version} 可用，已只读验证目标飞书文件夹"),
    })
}

/// 将当前格式阶段成功的自包含 Markdown/HTML 发布到飞书云空间。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn sync_article_to_connector(
    task_id: String,
    connector_id: String,
    format: DocumentFormat,
    expected_row_version: u64,
    expected_package_version: u32,
    expected_connector_revision: u32,
    confirmed: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
    runtime: State<'_, ConnectorRuntime>,
) -> Result<ConnectorSyncResult, CommandError> {
    let _operation_guard = runtime.operation_lock.lock().await;
    let workspace = repository.snapshot().await;
    let (article, connector) = match select_document_sync(
        &workspace,
        &task_id,
        &connector_id,
        format,
        expected_row_version,
        expected_package_version,
        expected_connector_revision,
        confirmed,
    )
    .map_err(selection_error)?
    {
        DocumentSyncSelection::Reused(record) => return Ok(sync_result(&record, true)),
        DocumentSyncSelection::Ready { article, connector } => (article, connector),
    };

    let source = match format {
        DocumentFormat::Markdown => article.artifacts.markdown.as_deref(),
        DocumentFormat::Html => article.artifacts.html.as_deref(),
    }
    .ok_or_else(|| document_sync_error(DocumentSyncError::ArtifactMissing))?;
    let image_reference = article
        .artifacts
        .image_ref
        .as_deref()
        .ok_or_else(|| document_sync_error(DocumentSyncError::ImageMissing))?;
    let image_data_url = if image_reference.starts_with("data:image/") && fixture_mode_enabled() {
        image_reference.to_string()
    } else {
        let (mime, bytes) = worker
            .read_managed_article_image(image_reference)
            .await
            .map_err(|_| {
                CommandError::new("ARTICLE_IMAGE_UNAVAILABLE", "当前配图无法从受管目录读取")
            })?;
        format!("data:{mime};base64,{}", BASE64.encode(bytes))
    };
    let document = build_self_contained_document(format, source, &image_data_url, &article.title)
        .map_err(document_sync_error)?;
    let content = document.into_bytes();
    validate_upload_size(content.len())?;
    let file_name =
        document_sync_file_name(&article.title, &article.id, article.package_version, format);
    let uploaded = match format {
        DocumentFormat::Markdown => {
            runtime
                .client
                .create_markdown_with_check(
                    &connector.cli_profile,
                    &connector.folder_token,
                    &file_name,
                    content,
                    connector.timeout_ms,
                    || {
                        recheck_document_sync_before_upload(
                            &repository,
                            &task_id,
                            &connector_id,
                            format,
                            expected_row_version,
                            expected_package_version,
                            expected_connector_revision,
                        )
                    },
                )
                .await
        }
        DocumentFormat::Html => {
            runtime
                .client
                .upload_html_with_check(
                    &connector.cli_profile,
                    &connector.folder_token,
                    &file_name,
                    content,
                    connector.timeout_ms,
                    || {
                        recheck_document_sync_before_upload(
                            &repository,
                            &task_id,
                            &connector_id,
                            format,
                            expected_row_version,
                            expected_package_version,
                            expected_connector_revision,
                        )
                    },
                )
                .await
        }
    }
    .map_err(lark_cli_error)?;
    let record = DocumentSyncRecord {
        task_id: article.id,
        package_version: article.package_version,
        connector_id: connector.id,
        connector_revision: connector.revision,
        format,
        file_token: uploaded.file_token,
        file_name,
        synced_at_ms: current_time_ms(),
    };
    let saved = persist_uploaded_sync_record(&repository, record).await?;
    Ok(sync_result(&saved, false))
}

/// CLI 已完成远端只读预检后，再用 Core 核对即将出站的工作区版本。
pub(super) async fn recheck_document_sync_before_upload<'a>(
    repository: &'a WorkspaceRepository,
    task_id: &str,
    connector_id: &str,
    format: DocumentFormat,
    expected_row_version: u64,
    expected_package_version: u32,
    expected_connector_revision: u32,
) -> Result<MutexGuard<'a, WorkspaceData>, LarkCliError> {
    let current = repository.lock_for_external_write().await;
    match select_document_sync(
        &current,
        task_id,
        connector_id,
        format,
        expected_row_version,
        expected_package_version,
        expected_connector_revision,
        true,
    ) {
        Ok(DocumentSyncSelection::Ready { .. }) => Ok(current),
        Ok(DocumentSyncSelection::Reused(_)) => Err(LarkCliError::new(
            "CONNECTOR_SYNC_ALREADY_RECORDED",
            "该文章包已存在成功同步记录，请刷新后复用",
        )),
        Err(error) => Err(LarkCliError::new(
            error.code(),
            "文章或连接器状态已变化，请刷新后重试",
        )),
    }
}

/// 应用启动和连接器保存/删除后重试独立秘密清理队列。
pub async fn reconcile_pending_connector_credential_deletions(repository: &WorkspaceRepository) {
    let vault = ConnectorCredentialVault;
    reconcile_pending_connector_credential_deletions_inner(repository, |reference| {
        vault.delete_app_secret(reference)
    })
    .await;
}

pub(super) async fn reconcile_pending_connector_credential_deletions_inner(
    repository: &WorkspaceRepository,
    mut delete: impl FnMut(&str) -> Result<(), String>,
) {
    let pending = repository
        .snapshot()
        .await
        .pending_connector_credential_deletions
        .into_iter()
        .collect::<Vec<_>>();
    for credential_ref in pending {
        let snapshot = repository.snapshot().await;
        if !snapshot
            .pending_connector_credential_deletions
            .contains(&credential_ref)
        {
            continue;
        }
        if core_referenced_connector_credentials(&snapshot).contains(&credential_ref) {
            let _ = repository
                .update(|workspace| {
                    cancel_pending_connector_credential_deletion_if_referenced(
                        workspace,
                        &credential_ref,
                    );
                    Ok(())
                })
                .await;
            continue;
        }
        if delete(&credential_ref).is_err() {
            continue;
        }
        let _ = repository
            .update(|workspace| {
                workspace
                    .pending_connector_credential_deletions
                    .remove(&credential_ref);
                Ok(())
            })
            .await;
    }
}

fn connector_views(workspace: &WorkspaceData) -> Vec<ConnectorConfigView> {
    workspace.connector_configs.clone()
}

pub(super) fn apply_connector_update_in_workspace(
    workspace: &mut WorkspaceData,
    requested: ConnectorConfig,
) -> Result<ConnectorConfig, String> {
    apply_core_connector_update_in_workspace(workspace, requested)
        .map_err(|error| error.code().to_string())
}

pub(super) fn delete_connector_in_workspace(
    workspace: &mut WorkspaceData,
    connector_id: &str,
    expected_revision: u32,
) -> Result<BTreeSet<String>, String> {
    delete_core_connector_in_workspace(workspace, connector_id, expected_revision)
        .map_err(|error| error.code().to_string())
}

#[cfg(test)]
pub(super) fn reusable_sync_record<'a>(
    workspace: &'a WorkspaceData,
    task_id: &str,
    package_version: u32,
    connector_id: &str,
    connector_revision: u32,
    format: DocumentFormat,
) -> Option<&'a DocumentSyncRecord> {
    core_reusable_sync_record(
        workspace,
        task_id,
        package_version,
        connector_id,
        connector_revision,
        format,
    )
}

/// 远端已经返回 file_token 后，本地失败必须标为结果未知，不能伪装成可直接重试的仓储错误。
pub(super) async fn persist_uploaded_sync_record(
    repository: &WorkspaceRepository,
    record: DocumentSyncRecord,
) -> Result<DocumentSyncRecord, CommandError> {
    repository
        .update(|workspace| Ok(record_uploaded_document_sync(workspace, record.clone())))
        .await
        .map_err(|_| {
            CommandError::new(
                "CONNECTOR_SYNC_OUTCOME_UNKNOWN",
                "飞书已返回文件 token，但本地未能保存同步记录；请先在飞书检查文件，避免重复上传",
            )
        })
}

fn sync_result(record: &DocumentSyncRecord, reused: bool) -> ConnectorSyncResult {
    ConnectorSyncResult {
        task_id: record.task_id.clone(),
        package_version: record.package_version,
        connector_id: record.connector_id.clone(),
        connector_revision: record.connector_revision,
        format: record.format,
        file_token: record.file_token.clone(),
        file_name: record.file_name.clone(),
        synced_at_ms: record.synced_at_ms,
        reused,
    }
}

pub(super) fn validate_upload_size(size: usize) -> Result<(), CommandError> {
    core_validate_upload_size(size)
        .map_err(|error| CommandError::new(error.code(), error.message()))
}

fn selection_error(error: ConnectorSelectionError) -> CommandError {
    CommandError::new(error.code(), error.message())
}

fn connector_update_error(message: String) -> CommandError {
    let safe_message = match message.as_str() {
        "CONNECTOR_ID_REQUIRED" => "连接器稳定 ID 不能为空",
        "CONNECTOR_NAME_REQUIRED" => "连接器名称不能为空",
        "CONNECTOR_CLI_PROFILE_REQUIRED" => "飞书 CLI profile 不能为空",
        "CONNECTOR_CLI_PROFILE_INVALID" => "飞书 CLI profile 格式无效",
        "CONNECTOR_FOLDER_TOKEN_REQUIRED" => "飞书目标文件夹 token 不能为空",
        "CONNECTOR_TIMEOUT_INVALID" => "连接器请求超时必须在 1 到 300 秒之间",
        "CONNECTOR_DISABLED_DEFAULT" => "已停用的连接器不能设为默认连接器",
        "CONNECTOR_REVISION_CONFLICT" => "连接器已被其他操作更新，请刷新后重试",
        "CONNECTOR_REVISION_EXHAUSTED" => "连接器修订号已达到上限",
        "CONNECTOR_NOT_FOUND" => "找不到指定连接器",
        _ => return store_error(message),
    };
    CommandError::new(message, safe_message)
}

fn document_sync_error(error: DocumentSyncError) -> CommandError {
    CommandError::new(error.code(), error.to_string())
}

fn lark_cli_error(error: LarkCliError) -> CommandError {
    CommandError::new(error.code, error.message)
}

fn store_error(message: String) -> CommandError {
    CommandError::new("STORE_ERROR", message)
}
