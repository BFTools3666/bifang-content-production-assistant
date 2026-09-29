//! 云文档连接器的配置、同步资格与成功记录领域规则。

use serde::{Deserialize, Serialize};

use crate::{ArticleTask, StageStatus, WorkflowStage};

/// 首版允许同步到连接器的最终文件格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentFormat {
    /// 自包含 Markdown 文件。
    Markdown,
    /// 自包含 HTML 文件。
    Html,
}

impl DocumentFormat {
    /// 返回上传文件使用的稳定扩展名。
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
        }
    }
}

/// 当前支持的云文档连接器类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectorKind {
    /// 飞书云空间文件连接器。
    FeishuDrive,
}

fn default_connector_revision() -> u32 {
    1
}

/// 与官方 CLI 适配器一致的 profile 名称安全子集。
fn is_safe_cli_profile_name(profile: &str) -> bool {
    let invalid_character = |character: char| {
        character.is_control()
            || character.is_whitespace()
            || matches!(
                character,
                '/' | '\\'
                    | '"'
                    | '\''
                    | '`'
                    | '$'
                    | '#'
                    | '!'
                    | '&'
                    | '|'
                    | ';'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '?'
                    | '*'
                    | '~'
            )
    };
    !profile.is_empty() && profile.chars().count() <= 64 && !profile.chars().any(invalid_character)
}

/// 不包含 CLI token 的云文档发布目标配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConfig {
    /// 本地稳定配置 ID。
    pub id: String,
    /// 用户可识别的名称。
    pub name: String,
    /// 同一稳定 ID 下不可变配置的修订号。
    #[serde(default = "default_connector_revision")]
    pub revision: u32,
    /// 连接器类型。
    pub kind: ConnectorKind,
    /// 是否允许连接测试和新的同步操作。
    #[serde(default)]
    pub enabled: bool,
    /// 由用户在官方 `lark-cli` 中独立维护的 profile 名称。
    #[serde(default)]
    pub cli_profile: String,
    /// 最终文件所在飞书云空间文件夹 token。
    pub folder_token: String,
    /// 新同步动作默认选择的文件格式。
    pub default_format: DocumentFormat,
    /// 是否为界面默认选中的唯一连接器。
    #[serde(default)]
    pub is_default: bool,
    /// 单次 CLI 调用的等待上限，单位毫秒。
    pub timeout_ms: u64,
}

impl ConnectorConfig {
    /// 建立不含稳定 ID 和 CLI 凭据的新飞书发布目标表单默认值。
    pub fn feishu_drive_draft() -> Self {
        Self {
            id: String::new(),
            name: "飞书 CLI 发布目标".into(),
            revision: 1,
            kind: ConnectorKind::FeishuDrive,
            enabled: true,
            cli_profile: String::new(),
            folder_token: String::new(),
            default_format: DocumentFormat::Markdown,
            is_default: false,
            timeout_ms: 30_000,
        }
    }

    /// 校验连接器进入持久当前集合所需的领域约束。
    pub fn validate_for_persistence(&self) -> Result<(), ConnectorConfigError> {
        if self.id.trim().is_empty() {
            return Err(ConnectorConfigError::IdRequired);
        }
        if self.name.trim().is_empty() {
            return Err(ConnectorConfigError::NameRequired);
        }
        if self.cli_profile.trim().is_empty() {
            return Err(ConnectorConfigError::CliProfileRequired);
        }
        if !is_safe_cli_profile_name(&self.cli_profile) {
            return Err(ConnectorConfigError::CliProfileInvalid);
        }
        if self.folder_token.trim().is_empty() {
            return Err(ConnectorConfigError::FolderTokenRequired);
        }
        if !(1_000..=300_000).contains(&self.timeout_ms) {
            return Err(ConnectorConfigError::TimeoutOutOfRange);
        }
        if !self.enabled && self.is_default {
            return Err(ConnectorConfigError::DisabledDefault);
        }
        Ok(())
    }
}

/// 连接器配置违反稳定领域约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorConfigError {
    /// 持久配置必须拥有稳定 ID。
    IdRequired,
    /// 配置必须拥有用户可识别名称。
    NameRequired,
    /// 必须显式选择一个 CLI profile。
    CliProfileRequired,
    /// CLI profile 必须为 1..64 个字符，且不能包含空白或命令特殊字符。
    CliProfileInvalid,
    /// 目标文件夹 token 不能为空。
    FolderTokenRequired,
    /// 请求超时必须在 1 秒到 5 分钟之间。
    TimeoutOutOfRange,
    /// 停用配置不能继续作为默认连接器。
    DisabledDefault,
}

impl ConnectorConfigError {
    /// 返回供适配器和前端使用的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdRequired => "CONNECTOR_ID_REQUIRED",
            Self::NameRequired => "CONNECTOR_NAME_REQUIRED",
            Self::CliProfileRequired => "CONNECTOR_CLI_PROFILE_REQUIRED",
            Self::CliProfileInvalid => "CONNECTOR_CLI_PROFILE_INVALID",
            Self::FolderTokenRequired => "CONNECTOR_FOLDER_TOKEN_REQUIRED",
            Self::TimeoutOutOfRange => "CONNECTOR_TIMEOUT_INVALID",
            Self::DisabledDefault => "CONNECTOR_DISABLED_DEFAULT",
        }
    }
}

impl std::fmt::Display for ConnectorConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::IdRequired => "连接器稳定 ID 不能为空",
            Self::NameRequired => "连接器名称不能为空",
            Self::CliProfileRequired => "飞书 CLI profile 不能为空",
            Self::CliProfileInvalid => "飞书 CLI profile 格式无效",
            Self::FolderTokenRequired => "飞书目标文件夹 token 不能为空",
            Self::TimeoutOutOfRange => "连接器请求超时必须在 1 到 300 秒之间",
            Self::DisabledDefault => "已停用的连接器不能设为默认连接器",
        })
    }
}

impl std::error::Error for ConnectorConfigError {}

/// 把旧直连配置迁移为需要用户显式选择 profile 后重新启用的 CLI 发布目标。
///
/// 迁移保留稳定 ID、文件夹、格式和历史关联；不推测 profile。若需停用旧目标，会把旧修订写入
/// 历史并建立一个新修订，避免同一修订出现两种可观察状态。调用方必须用工作区 schema 版本
/// 保证该迁移只执行一次。
pub fn migrate_connector_configs_to_cli(
    current: &mut [ConnectorConfig],
    history: &mut Vec<ConnectorConfig>,
) -> Result<bool, ConnectorConfigUpdateError> {
    let mut changed = false;
    for connector in current {
        if connector.cli_profile.trim().is_empty() {
            let previous = connector.clone();
            connector.revision = connector
                .revision
                .checked_add(1)
                .ok_or(ConnectorConfigUpdateError::RevisionExhausted)?;
            connector.enabled = false;
            connector.is_default = false;
            history.push(previous);
            changed = true;
        }
    }
    Ok(changed)
}

/// 一次原子连接器配置更新的领域结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorConfigUpdate {
    /// 写入当前配置集合的新修订。
    pub saved: ConnectorConfig,
    /// 此稳定 ID 是否替换了已有修订。
    pub replaced_existing: bool,
}

/// 连接器配置无法安全建立下一修订。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorConfigUpdateError {
    /// 请求配置违反持久化约束。
    InvalidConfig(ConnectorConfigError),
    /// 前端提交的修订已经过期。
    RevisionConflict,
    /// 当前或被挤下默认位的配置已达到修订号上限。
    RevisionExhausted,
    /// 指定稳定 ID 不存在。
    NotFound,
}

impl ConnectorConfigUpdateError {
    /// 返回供适配器和前端使用的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfig(error) => error.code(),
            Self::RevisionConflict => "CONNECTOR_REVISION_CONFLICT",
            Self::RevisionExhausted => "CONNECTOR_REVISION_EXHAUSTED",
            Self::NotFound => "CONNECTOR_NOT_FOUND",
        }
    }
}

impl std::fmt::Display for ConnectorConfigUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(error) => error.fmt(formatter),
            Self::RevisionConflict => formatter.write_str("连接器已被其他操作更新"),
            Self::RevisionExhausted => formatter.write_str("连接器修订号已达到上限"),
            Self::NotFound => formatter.write_str("指定连接器不存在"),
        }
    }
}

impl std::error::Error for ConnectorConfigUpdateError {}

impl From<ConnectorConfigError> for ConnectorConfigUpdateError {
    fn from(error: ConnectorConfigError) -> Self {
        Self::InvalidConfig(error)
    }
}

/// 原子新增或修订连接器，并为被挤下默认位的配置建立新修订。
pub fn apply_connector_config_update(
    current: &mut Vec<ConnectorConfig>,
    history: &mut Vec<ConnectorConfig>,
    mut requested: ConnectorConfig,
) -> Result<ConnectorConfigUpdate, ConnectorConfigUpdateError> {
    requested.validate_for_persistence()?;
    let target_position = current
        .iter()
        .position(|connector| connector.id == requested.id);
    let previous_target = target_position.map(|position| current[position].clone());
    if previous_target
        .as_ref()
        .is_some_and(|previous| previous.revision != requested.revision)
    {
        return Err(ConnectorConfigUpdateError::RevisionConflict);
    }
    requested.revision = previous_target
        .as_ref()
        .map(|connector| {
            connector
                .revision
                .checked_add(1)
                .ok_or(ConnectorConfigUpdateError::RevisionExhausted)
        })
        .transpose()?
        .unwrap_or(1);

    let mut displaced = Vec::new();
    if requested.is_default {
        for (position, connector) in current.iter().enumerate() {
            if Some(position) == target_position || !connector.is_default {
                continue;
            }
            let revision = connector
                .revision
                .checked_add(1)
                .ok_or(ConnectorConfigUpdateError::RevisionExhausted)?;
            displaced.push((position, revision));
        }
    }
    for (position, revision) in displaced {
        let previous = current[position].clone();
        let mut revised = previous.clone();
        revised.revision = revision;
        revised.is_default = false;
        history.push(previous);
        current[position] = revised;
    }

    if let Some(position) = target_position {
        history.push(previous_target.expect("target position must have a previous value"));
        current[position] = requested.clone();
    } else {
        current.push(requested.clone());
    }
    Ok(ConnectorConfigUpdate {
        saved: requested,
        replaced_existing: target_position.is_some(),
    })
}

/// 以乐观锁启用或停用一个连接器；停用同时解除默认位。
pub fn set_connector_config_enabled(
    current: &mut Vec<ConnectorConfig>,
    history: &mut Vec<ConnectorConfig>,
    connector_id: &str,
    expected_revision: u32,
    enabled: bool,
) -> Result<ConnectorConfigUpdate, ConnectorConfigUpdateError> {
    let existing = current
        .iter()
        .find(|connector| connector.id == connector_id)
        .cloned()
        .ok_or(ConnectorConfigUpdateError::NotFound)?;
    if existing.revision != expected_revision {
        return Err(ConnectorConfigUpdateError::RevisionConflict);
    }
    if existing.enabled == enabled {
        return Ok(ConnectorConfigUpdate {
            saved: existing,
            replaced_existing: true,
        });
    }
    let mut requested = existing;
    requested.enabled = enabled;
    if !enabled {
        requested.is_default = false;
    }
    apply_connector_config_update(current, history, requested)
}

/// 以乐观锁删除一个当前连接器，并把最后修订保留在历史中。
pub fn delete_connector_config(
    current: &mut Vec<ConnectorConfig>,
    history: &mut Vec<ConnectorConfig>,
    connector_id: &str,
    expected_revision: u32,
) -> Result<ConnectorConfig, ConnectorConfigUpdateError> {
    let position = current
        .iter()
        .position(|connector| connector.id == connector_id)
        .ok_or(ConnectorConfigUpdateError::NotFound)?;
    if current[position].revision != expected_revision {
        return Err(ConnectorConfigUpdateError::RevisionConflict);
    }
    let removed = current.remove(position);
    history.push(removed.clone());
    Ok(removed)
}

/// 一次已经成功上传到云文档连接器的不可变记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSyncRecord {
    /// 本地文章稳定 ID。
    pub task_id: String,
    /// 上传时冻结的文章包版本。
    pub package_version: u32,
    /// 使用的连接器稳定 ID。
    pub connector_id: String,
    /// 使用的连接器不可变修订。
    pub connector_revision: u32,
    /// 上传的最终文件格式。
    pub format: DocumentFormat,
    /// 飞书返回的云空间文件 token。
    pub file_token: String,
    /// 上传时使用的文件名。
    pub file_name: String,
    /// 成功完成时间，使用 Unix 毫秒。
    pub synced_at_ms: i64,
}

impl DocumentSyncRecord {
    /// 判断成功记录能否幂等满足同一同步请求。
    pub fn matches_request(
        &self,
        task_id: &str,
        package_version: u32,
        connector_id: &str,
        connector_revision: u32,
        format: DocumentFormat,
    ) -> bool {
        self.task_id == task_id
            && self.package_version == package_version
            && self.connector_id == connector_id
            && self.connector_revision == connector_revision
            && self.format == format
    }
}

/// 同步动作在任何网络调用前违反的稳定领域原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSyncError {
    /// 外部写入缺少明确确认。
    ConfirmationRequired,
    /// 文章行或包版本已经变化。
    ArticleRevisionConflict,
    /// 当前文章的格式阶段尚未成功。
    FormatNotReady,
    /// 当前文章缺少请求格式的持久产物。
    ArtifactMissing,
    /// 当前文章缺少可嵌入的配图。
    ImageMissing,
    /// 连接器修订已经变化。
    ConnectorRevisionConflict,
    /// 连接器已停用。
    ConnectorDisabled,
}

impl DocumentSyncError {
    /// 返回供适配器和前端使用的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::ConfirmationRequired => "CONNECTOR_SYNC_CONFIRMATION_REQUIRED",
            Self::ArticleRevisionConflict => "ARTICLE_REVISION_CONFLICT",
            Self::FormatNotReady => "FORMAT_STAGE_NOT_READY",
            Self::ArtifactMissing => "FORMAT_ARTIFACT_MISSING",
            Self::ImageMissing => "ARTICLE_IMAGE_MISSING",
            Self::ConnectorRevisionConflict => "CONNECTOR_REVISION_CONFLICT",
            Self::ConnectorDisabled => "CONNECTOR_DISABLED",
        }
    }
}

impl std::fmt::Display for DocumentSyncError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ConfirmationRequired => "同步到飞书前必须明确确认外部写入",
            Self::ArticleRevisionConflict => "文章状态已变化，请刷新后重新确认",
            Self::FormatNotReady => "格式阶段尚未成功，请先生成最终格式",
            Self::ArtifactMissing => "当前文章缺少请求格式的最终内容",
            Self::ImageMissing => "当前文章缺少可同步的配图",
            Self::ConnectorRevisionConflict => "连接器已变化，请刷新后重新确认",
            Self::ConnectorDisabled => "连接器已停用，请先启用后再同步",
        })
    }
}

impl std::error::Error for DocumentSyncError {}

/// 校验一次外部同步的文章、连接器和乐观锁前置条件。
pub fn validate_document_sync(
    article: &ArticleTask,
    connector: &ConnectorConfig,
    format: DocumentFormat,
    expected_row_version: u64,
    expected_package_version: u32,
    expected_connector_revision: u32,
    confirmed: bool,
) -> Result<(), DocumentSyncError> {
    if !confirmed {
        return Err(DocumentSyncError::ConfirmationRequired);
    }
    if article.row_version != expected_row_version
        || article.package_version != expected_package_version
    {
        return Err(DocumentSyncError::ArticleRevisionConflict);
    }
    if connector.revision != expected_connector_revision {
        return Err(DocumentSyncError::ConnectorRevisionConflict);
    }
    if !connector.enabled {
        return Err(DocumentSyncError::ConnectorDisabled);
    }
    let format_stage = article.stages.get(WorkflowStage::Format);
    if format_stage.status != StageStatus::Succeeded
        || format_stage.artifact_version != Some(article.package_version)
    {
        return Err(DocumentSyncError::FormatNotReady);
    }
    let content_exists = match format {
        DocumentFormat::Markdown => article.artifacts.markdown.as_deref(),
        DocumentFormat::Html => article.artifacts.html.as_deref(),
    }
    .is_some_and(|content| !content.trim().is_empty());
    if !content_exists {
        return Err(DocumentSyncError::ArtifactMissing);
    }
    if !article
        .artifacts
        .image_ref
        .as_deref()
        .is_some_and(|reference| !reference.trim().is_empty())
    {
        return Err(DocumentSyncError::ImageMissing);
    }
    Ok(())
}

/// 建立稳定且不包含路径分隔符的最终上传文件名。
pub fn document_sync_file_name(
    title: &str,
    task_id: &str,
    package_version: u32,
    format: DocumentFormat,
) -> String {
    let sanitized = title
        .trim()
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .take(96)
        .collect::<String>();
    let stem = if sanitized.trim_matches([' ', '.', '_']).is_empty() {
        let stable = task_id.chars().take(24).collect::<String>();
        format!("article-{stable}")
    } else {
        sanitized.trim_matches([' ', '.']).to_string()
    };
    format!("{stem}-v{package_version}.{}", format.extension())
}

/// 将持久最终内容和当前图片 Data URL 合成为不依赖本地路径的文件内容。
pub fn build_self_contained_document(
    format: DocumentFormat,
    content: &str,
    image_data_url: &str,
    title: &str,
) -> Result<String, DocumentSyncError> {
    if content.trim().is_empty() {
        return Err(DocumentSyncError::ArtifactMissing);
    }
    if !image_data_url.starts_with("data:image/") || !image_data_url.contains(";base64,") {
        return Err(DocumentSyncError::ImageMissing);
    }
    Ok(match format {
        DocumentFormat::Markdown => {
            let alt = title
                .replace('\\', "\\\\")
                .replace('[', "\\[")
                .replace(']', "\\]");
            let image = format!("![{alt}]({image_data_url})");
            insert_after_first_markdown_h1(content, &image)
        }
        DocumentFormat::Html => {
            let escaped = escape_html(title);
            let image = format!(
                "<figure><img src=\"{image_data_url}\" alt=\"{escaped}\"><figcaption>{escaped}</figcaption></figure>"
            );
            insert_after_first_html_h1(content, &image)
        }
    })
}

fn insert_after_first_markdown_h1(markdown: &str, image: &str) -> String {
    let mut offset = 0;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.starts_with("# ") {
            let before = markdown[..offset + line.len()].trim_end();
            let after = markdown[offset + line.len()..].trim_start_matches(['\r', '\n']);
            return if after.is_empty() {
                format!("{before}\n\n{image}")
            } else {
                format!("{before}\n\n{image}\n\n{after}")
            };
        }
        offset += line.len();
    }
    format!("{image}\n\n{markdown}")
}

fn insert_after_first_html_h1(html: &str, image: &str) -> String {
    let lowercase = html.to_ascii_lowercase();
    if let Some(position) = lowercase.find("</h1>") {
        let insert_at = position + "</h1>".len();
        format!("{}\n{image}\n{}", &html[..insert_at], &html[insert_at..])
    } else {
        format!("{image}\n{html}")
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests;
