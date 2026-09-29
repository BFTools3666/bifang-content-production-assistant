//! 云文档连接器的工作区级配置、凭据引用与同步记录规则。

use std::collections::BTreeSet;

use super::WorkspaceData;
use crate::{
    ArticleTask, ConnectorConfig, ConnectorConfigError, ConnectorConfigUpdateError, DocumentFormat,
    DocumentSyncError, DocumentSyncRecord, apply_connector_config_update, delete_connector_config,
    set_connector_config_enabled, validate_document_sync,
};

/// 首版飞书文件上传上限，计算对象为最终自包含文件的字节数。
pub const MAX_FEISHU_FILE_BYTES: usize = 20 * 1024 * 1024;

/// 最终上传文件大小拒绝原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentUploadSizeError {
    /// 最终文件内容为空。
    Empty,
    /// 自包含文件超过 20 MB。
    TooLarge,
}

impl DocumentUploadSizeError {
    /// 稳定 IPC 错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "FORMAT_ARTIFACT_MISSING",
            Self::TooLarge => "CONNECTOR_FILE_TOO_LARGE",
        }
    }

    /// 不暴露上传文件内容的提示。
    pub const fn message(self) -> &'static str {
        match self {
            Self::Empty => "最终同步文件不能为空",
            Self::TooLarge => "包含配图的最终文件超过 20MB 上限",
        }
    }
}

/// 同步预检对工作区身份或领域状态的拒绝原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorSelectionError {
    /// 目标文章不存在。
    ArticleNotFound,
    /// 目标连接器不存在。
    ConnectorNotFound,
    /// 只读连接测试不允许停用的连接器。
    ConnectorDisabled,
    /// 外部同步未满足格式、确认或修订约束。
    DocumentSync(DocumentSyncError),
}

impl ConnectorSelectionError {
    /// 稳定 IPC 错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::ArticleNotFound => "ARTICLE_NOT_FOUND",
            Self::ConnectorNotFound => "CONNECTOR_NOT_FOUND",
            Self::ConnectorDisabled => "CONNECTOR_DISABLED",
            Self::DocumentSync(error) => error.code(),
        }
    }

    /// 不含远端响应或凭据的安全提示。
    pub fn message(self) -> String {
        match self {
            Self::ArticleNotFound => "找不到指定文章".into(),
            Self::ConnectorNotFound => "找不到指定连接器".into(),
            Self::ConnectorDisabled => "该连接器已停用，请先启用后再测试".into(),
            Self::DocumentSync(error) => error.to_string(),
        }
    }
}

/// 远端写入之前的纯领域决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentSyncSelection {
    /// 完全相同范围的已成功上传记录，可零出站复用。
    Reused(DocumentSyncRecord),
    /// 必须经宿主读取图片并调用远端的冻结输入。
    Ready {
        /// 当前文章及其最终产物快照。
        article: ArticleTask,
        /// 目标连接器修订快照。
        connector: ConnectorConfig,
    },
}

/// 表单归一化与校验由 Core 完成，稳定 ID 只由宿主提供。
pub fn prepare_connector_config_for_save(
    mut config: ConnectorConfig,
    new_id: String,
) -> Result<ConnectorConfig, ConnectorConfigError> {
    config.name = config.name.trim().to_string();
    config.cli_profile = config.cli_profile.trim().to_string();
    config.folder_token = config.folder_token.trim().to_string();
    if config.id.trim().is_empty() {
        config.id = new_id;
    }
    config.validate_for_persistence()?;
    Ok(config)
}

/// 在一个工作区更新中建立连接器下一修订与历史。
pub fn apply_connector_update_in_workspace(
    workspace: &mut WorkspaceData,
    requested: ConnectorConfig,
) -> Result<ConnectorConfig, ConnectorConfigUpdateError> {
    apply_connector_config_update(
        &mut workspace.connector_configs,
        &mut workspace.connector_config_history,
        requested,
    )
    .map(|update| update.saved)
}

/// 在一个工作区更新中启停并保留连接器历史。
pub fn set_connector_enabled_in_workspace(
    workspace: &mut WorkspaceData,
    connector_id: &str,
    expected_revision: u32,
    enabled: bool,
) -> Result<ConnectorConfig, ConnectorConfigUpdateError> {
    set_connector_config_enabled(
        &mut workspace.connector_configs,
        &mut workspace.connector_config_history,
        connector_id,
        expected_revision,
        enabled,
    )
    .map(|update| update.saved)
}

/// 删除外部发布目标前必须获得本次显式确认。
pub fn validate_connector_delete_confirmation(confirmed: bool) -> Result<(), &'static str> {
    if confirmed {
        Ok(())
    } else {
        Err("删除连接器前必须明确确认")
    }
}

/// 删除连接器时只把不再被其他配置引用的遗留凭据送入可重试清理队列。
pub fn delete_connector_in_workspace(
    workspace: &mut WorkspaceData,
    connector_id: &str,
    expected_revision: u32,
) -> Result<BTreeSet<String>, ConnectorConfigUpdateError> {
    delete_connector_config(
        &mut workspace.connector_configs,
        &mut workspace.connector_config_history,
        connector_id,
        expected_revision,
    )?;
    let removed_refs = workspace
        .connector_credential_refs
        .remove(connector_id)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let retained = referenced_connector_credentials(workspace);
    workspace.pending_connector_credential_deletions.extend(
        removed_refs
            .iter()
            .filter(|reference| !retained.contains(*reference))
            .cloned(),
    );
    Ok(removed_refs)
}

/// 当前仍有活跃连接器引用的全部遗留凭据条目。
pub fn referenced_connector_credentials(workspace: &WorkspaceData) -> BTreeSet<String> {
    workspace
        .connector_credential_refs
        .values()
        .cloned()
        .collect()
}

/// 清理队列中的引用重新变成活跃引用时撤销待删除状态。
pub fn cancel_pending_connector_credential_deletion_if_referenced(
    workspace: &mut WorkspaceData,
    credential_ref: &str,
) -> bool {
    if referenced_connector_credentials(workspace).contains(credential_ref) {
        workspace
            .pending_connector_credential_deletions
            .remove(credential_ref)
    } else {
        false
    }
}

/// 连接测试只选择存在且启用的当前连接器，不涉及网络。
pub fn connector_for_connection_test(
    workspace: &WorkspaceData,
    connector_id: &str,
) -> Result<ConnectorConfig, ConnectorSelectionError> {
    let connector = workspace
        .connector_configs
        .iter()
        .find(|connector| connector.id == connector_id)
        .ok_or(ConnectorSelectionError::ConnectorNotFound)?;
    if !connector.enabled {
        return Err(ConnectorSelectionError::ConnectorDisabled);
    }
    Ok(connector.clone())
}

/// 同步前在 Core 中完成身份、乐观锁、确认与成功记录复用选择。
#[allow(clippy::too_many_arguments)]
pub fn select_document_sync(
    workspace: &WorkspaceData,
    task_id: &str,
    connector_id: &str,
    format: DocumentFormat,
    expected_row_version: u64,
    expected_package_version: u32,
    expected_connector_revision: u32,
    confirmed: bool,
) -> Result<DocumentSyncSelection, ConnectorSelectionError> {
    let article = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .ok_or(ConnectorSelectionError::ArticleNotFound)?;
    let connector = workspace
        .connector_configs
        .iter()
        .find(|connector| connector.id == connector_id)
        .ok_or(ConnectorSelectionError::ConnectorNotFound)?;
    validate_document_sync(
        article,
        connector,
        format,
        expected_row_version,
        expected_package_version,
        expected_connector_revision,
        confirmed,
    )
    .map_err(ConnectorSelectionError::DocumentSync)?;
    if let Some(record) = reusable_sync_record(
        workspace,
        task_id,
        expected_package_version,
        connector_id,
        expected_connector_revision,
        format,
    ) {
        return Ok(DocumentSyncSelection::Reused(record.clone()));
    }
    Ok(DocumentSyncSelection::Ready {
        article: article.clone(),
        connector: connector.clone(),
    })
}

/// 查找完全相同文章包、连接器修订与格式的成功同步记录。
pub fn reusable_sync_record<'a>(
    workspace: &'a WorkspaceData,
    task_id: &str,
    package_version: u32,
    connector_id: &str,
    connector_revision: u32,
    format: DocumentFormat,
) -> Option<&'a DocumentSyncRecord> {
    workspace.document_sync_records.iter().find(|record| {
        record.matches_request(
            task_id,
            package_version,
            connector_id,
            connector_revision,
            format,
        )
    })
}

/// 远端已返回 token 后幂等写入成功记录；宿主仍须区分本地持久化失败。
pub fn record_uploaded_document_sync(
    workspace: &mut WorkspaceData,
    record: DocumentSyncRecord,
) -> DocumentSyncRecord {
    if let Some(existing) = reusable_sync_record(
        workspace,
        &record.task_id,
        record.package_version,
        &record.connector_id,
        record.connector_revision,
        record.format,
    ) {
        return existing.clone();
    }
    workspace.document_sync_records.insert(0, record.clone());
    record
}

/// 检查自包含最终文件字节数。
pub fn validate_upload_size(size: usize) -> Result<(), DocumentUploadSizeError> {
    if size == 0 {
        return Err(DocumentUploadSizeError::Empty);
    }
    if size > MAX_FEISHU_FILE_BYTES {
        return Err(DocumentUploadSizeError::TooLarge);
    }
    Ok(())
}
