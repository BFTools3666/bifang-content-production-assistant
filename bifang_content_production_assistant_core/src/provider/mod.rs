//! AI Provider 的中立请求、结果和稳定错误分类。

use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};

use crate::prompt::PromptTemplateKind;

/// Provider 配置可声明的能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderCapability {
    /// 文本生成能力。
    Text,
    /// 图片生成能力。
    Image,
}

fn default_provider_revision() -> u32 {
    1
}

/// 当前支持的 Provider 配置类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    /// 使用 OpenAI-compatible 文本与图片契约。
    OpenAiCompatible,
    /// 使用火山方舟图片生成契约；不提供正文或简介文本能力。
    VolcengineArkImage,
    /// 仅供显式测试环境使用的确定性夹具。
    Fixture,
}

impl ProviderKind {
    /// 返回当前服务类型是否能够承担指定生成能力。
    pub const fn supports(self, capability: ProviderCapability) -> bool {
        match (self, capability) {
            (Self::OpenAiCompatible | Self::Fixture, _) => true,
            (Self::VolcengineArkImage, ProviderCapability::Image) => true,
            (Self::VolcengineArkImage, ProviderCapability::Text) => false,
        }
    }

    /// 返回当前服务类型新建配置使用的图片参数默认值。
    pub fn default_image_parameters(self) -> ImageGenerationParameters {
        match self {
            Self::VolcengineArkImage => ImageGenerationParameters::volcengine_ark_default(),
            Self::OpenAiCompatible | Self::Fixture => ImageGenerationParameters::default(),
        }
    }
}

mod config;
mod parameters;

pub use config::*;
pub use parameters::*;

/// 首次持久化三阶段独立模型与参数的模式版本。
pub const CURRENT_PROVIDER_CONFIG_SCHEMA_VERSION: u32 = 5;

/// 从一个已知工作区模式迁移 Provider 配置时采用的稳定规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConfigMigration {
    /// v1 只有单一文本默认位，需要复制为正文和简介默认位。
    FromSingleTextDefault,
    /// v2 已有独立默认位，但文本模型与参数仍需显式物化。
    FromSharedTextModel,
    /// v3 尚未持久化启用状态，现有配置应迁移为启用。
    FromEnabledStatus,
    /// v4 尚未声明供应商专属图片服务，既有配置保持 OpenAI-compatible。
    FromImageProviderKind,
    /// v5 数据仅执行幂等物化，修复缺失的显式默认参数。
    Current,
}

impl ProviderConfigMigration {
    /// 根据已持久化的模式版本选择迁移规则，未知版本失败关闭。
    pub fn for_schema_version(schema_version: u32) -> Result<Self, ProviderConfigMigrationError> {
        match schema_version {
            1 => Ok(Self::FromSingleTextDefault),
            2 => Ok(Self::FromSharedTextModel),
            3 => Ok(Self::FromEnabledStatus),
            4 => Ok(Self::FromImageProviderKind),
            CURRENT_PROVIDER_CONFIG_SCHEMA_VERSION => Ok(Self::Current),
            unsupported => Err(ProviderConfigMigrationError::UnsupportedVersion(
                unsupported,
            )),
        }
    }

    /// 返回迁移完成后应持久化的模式版本。
    pub const fn target_schema_version(self) -> u32 {
        CURRENT_PROVIDER_CONFIG_SCHEMA_VERSION
    }

    /// 对一个当前或历史 Provider 配置应用对应版本的领域迁移。
    pub fn migrate_config(self, config: &mut ProviderConfig) -> bool {
        let mut changed = false;
        let requires_enabled_backfill = matches!(
            self,
            Self::FromSingleTextDefault | Self::FromSharedTextModel | Self::FromEnabledStatus
        );
        if requires_enabled_backfill && !config.enabled {
            config.enabled = true;
            changed = true;
        }
        if self == Self::FromSingleTextDefault
            && config.default_for_summary != config.default_for_article
        {
            config.default_for_summary = config.default_for_article;
            changed = true;
        }
        changed | config.materialize_stage_configuration()
    }
}

/// Provider 配置所在的持久化模式不受当前 core 支持。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConfigMigrationError {
    /// 无法安全解释的模式版本。
    UnsupportedVersion(u32),
}

impl std::fmt::Display for ProviderConfigMigrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("本地工作区版本不受支持")
    }
}

impl std::error::Error for ProviderConfigMigrationError {}

/// 一次原子 Provider 配置更新的领域结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfigUpdate {
    /// 写入当前配置集合的新修订。
    pub saved: ProviderConfig,
    /// 此稳定 ID 是否替换了已有修订。
    pub replaced_existing: bool,
}

/// Provider 配置无法安全建立下一修订。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConfigUpdateError {
    /// 请求配置违反 core 持久化约束。
    InvalidConfig(ProviderConfigError),
    /// 目标或被挤下默认位的配置已达到修订号上限。
    RevisionExhausted,
    /// 指定稳定 ID 不存在于当前配置集合。
    NotFound,
}

impl ProviderConfigUpdateError {
    /// 返回供适配器和前端使用的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfig(error) => error.code(),
            Self::RevisionExhausted => "PROVIDER_REVISION_EXHAUSTED",
            Self::NotFound => "PROVIDER_NOT_FOUND",
        }
    }
}

impl std::fmt::Display for ProviderConfigUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(error) => error.fmt(formatter),
            Self::RevisionExhausted => {
                formatter.write_str("AI 服务修订号已达到上限，无法再保存或切换默认服务")
            }
            Self::NotFound => formatter.write_str("指定的 AI 服务不存在"),
        }
    }
}

impl std::error::Error for ProviderConfigUpdateError {}

impl From<ProviderConfigError> for ProviderConfigUpdateError {
    fn from(error: ProviderConfigError) -> Self {
        Self::InvalidConfig(error)
    }
}

/// 原子替换一个 Provider，并为被其挤下每个阶段默认位的配置建立新修订。
pub fn apply_provider_config_update(
    current: &mut Vec<ProviderConfig>,
    history: &mut Vec<ProviderConfig>,
    mut requested: ProviderConfig,
) -> Result<ProviderConfigUpdate, ProviderConfigUpdateError> {
    requested.validate_for_persistence()?;
    let target_position = current
        .iter()
        .position(|provider| provider.id == requested.id);
    let previous_target = target_position.map(|position| current[position].clone());
    requested.revision = previous_target
        .as_ref()
        .map(|provider| {
            provider
                .revision
                .checked_add(1)
                .ok_or(ProviderConfigUpdateError::RevisionExhausted)
        })
        .transpose()?
        .unwrap_or(1);

    let mut displaced_revisions = Vec::new();
    for (position, provider) in current.iter().enumerate() {
        if Some(position) == target_position {
            continue;
        }
        let disable_article = requested.default_for_article && provider.default_for_article;
        let disable_summary = requested.default_for_summary && provider.default_for_summary;
        let disable_image = requested.default_for_image && provider.default_for_image;
        if !disable_article && !disable_summary && !disable_image {
            continue;
        }
        let next_revision = provider
            .revision
            .checked_add(1)
            .ok_or(ProviderConfigUpdateError::RevisionExhausted)?;
        displaced_revisions.push((
            position,
            next_revision,
            disable_article,
            disable_summary,
            disable_image,
        ));
    }

    for (position, next_revision, disable_article, disable_summary, disable_image) in
        displaced_revisions
    {
        let previous = current[position].clone();
        let mut revised = previous.clone();
        revised.revision = next_revision;
        if disable_article {
            revised.default_for_article = false;
        }
        if disable_summary {
            revised.default_for_summary = false;
        }
        if disable_image {
            revised.default_for_image = false;
        }
        history.push(previous);
        current[position] = revised;
    }

    if let Some(position) = target_position {
        if let Some(previous) = previous_target {
            history.push(previous);
        }
        current[position] = requested.clone();
    } else {
        current.push(requested.clone());
    }

    Ok(ProviderConfigUpdate {
        saved: requested,
        replaced_existing: target_position.is_some(),
    })
}

/// 把一个现有 Provider 原子切换为启用或停用状态。
///
/// 停用会同时解除三个阶段默认位；重新启用不会自动恢复这些默认位。重复提交
/// 相同状态是幂等操作，不建立无意义的新修订。
pub fn set_provider_config_enabled(
    current: &mut Vec<ProviderConfig>,
    history: &mut Vec<ProviderConfig>,
    provider_id: &str,
    enabled: bool,
) -> Result<ProviderConfigUpdate, ProviderConfigUpdateError> {
    let Some(existing) = current
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
    else {
        return Err(ProviderConfigUpdateError::NotFound);
    };
    if existing.enabled == enabled {
        return Ok(ProviderConfigUpdate {
            saved: existing,
            replaced_existing: true,
        });
    }

    let mut requested = existing;
    requested.enabled = enabled;
    if !enabled {
        requested.default_for_article = false;
        requested.default_for_summary = false;
        requested.default_for_image = false;
    }
    apply_provider_config_update(current, history, requested)
}

/// 把一个已保存 Provider 显式分配给单个生成步骤，或从该步骤移除。
///
/// 分配时复用默认位的唯一性：新 Provider 会原子挤下该步骤原有 Provider，
/// 但不会改变另外两个步骤。移除只清目标默认位，保留模型、参数、连接和历史。
/// 重复提交相同状态是幂等操作，不建立无意义的新修订。
pub fn set_provider_stage_assignment(
    current: &mut Vec<ProviderConfig>,
    history: &mut Vec<ProviderConfig>,
    provider_id: &str,
    stage: PromptTemplateKind,
    assigned: bool,
) -> Result<ProviderConfigUpdate, ProviderConfigUpdateError> {
    let Some(existing) = current
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
    else {
        return Err(ProviderConfigUpdateError::NotFound);
    };
    let already_assigned = match stage {
        PromptTemplateKind::Article => existing.default_for_article,
        PromptTemplateKind::Summary => existing.default_for_summary,
        PromptTemplateKind::Image => existing.default_for_image,
    };
    if already_assigned == assigned {
        return Ok(ProviderConfigUpdate {
            saved: existing,
            replaced_existing: true,
        });
    }

    let mut requested = existing;
    match stage {
        PromptTemplateKind::Article => requested.default_for_article = assigned,
        PromptTemplateKind::Summary => requested.default_for_summary = assigned,
        PromptTemplateKind::Image => requested.default_for_image = assigned,
    }
    apply_provider_config_update(current, history, requested)
}

/// Provider 当前配置不能被安全删除的领域原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConfigDeleteError {
    /// 删除请求缺少稳定 ID。
    IdRequired,
    /// 指定稳定 ID 不存在于当前配置集合。
    NotFound,
    /// 配置仍被至少一个生成步骤使用。
    DefaultInUse,
}

impl ProviderConfigDeleteError {
    /// 返回供适配器和前端使用的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdRequired => "PROVIDER_ID_REQUIRED",
            Self::NotFound => "PROVIDER_NOT_FOUND",
            Self::DefaultInUse => "PROVIDER_DELETE_DEFAULT_IN_USE",
        }
    }
}

impl std::fmt::Display for ProviderConfigDeleteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::IdRequired => "AI 服务稳定 ID 不能为空",
            Self::NotFound => "指定的 AI 服务不存在",
            Self::DefaultInUse => "AI 服务仍被生成步骤使用，请先从对应步骤移除",
        })
    }
}

impl std::error::Error for ProviderConfigDeleteError {}

/// 删除一个不再承担任何步骤分配的当前 Provider 配置。
///
/// 最后一个当前修订会进入历史集合，使已经显示过的配置演进仍可追溯；
/// 已冻结执行绑定及其凭据生命周期由持有这些绑定的适配器管理。
pub fn delete_provider_config(
    current: &mut Vec<ProviderConfig>,
    history: &mut Vec<ProviderConfig>,
    provider_id: &str,
) -> Result<ProviderConfig, ProviderConfigDeleteError> {
    if provider_id.trim().is_empty() {
        return Err(ProviderConfigDeleteError::IdRequired);
    }
    let Some(position) = current
        .iter()
        .position(|provider| provider.id == provider_id)
    else {
        return Err(ProviderConfigDeleteError::NotFound);
    };
    let provider = &current[position];
    if provider.default_for_article || provider.default_for_summary || provider.default_for_image {
        return Err(ProviderConfigDeleteError::DefaultInUse);
    }
    let removed = current.remove(position);
    history.push(removed.clone());
    Ok(removed)
}

/// Provider 生成参数不在产品允许值域内。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderParameterError {
    /// 温度必须位于 0 到 2 之间。
    TemperatureOutOfRange,
    /// 最大输出 token 数必须位于首版硬上限内。
    MaxOutputTokensOutOfRange,
    /// 请求超时必须位于 1 秒到 5 分钟之间。
    TimeoutOutOfRange,
    /// 图片尺寸不在首版白名单中。
    UnsupportedImageSize,
    /// 当前图片服务协议不支持水印参数。
    UnsupportedImageWatermark,
}

impl std::fmt::Display for ProviderParameterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TemperatureOutOfRange => "温度必须在 0 到 2 之间",
            Self::MaxOutputTokensOutOfRange => "最大输出 token 必须在 1 到 128000 之间",
            Self::TimeoutOutOfRange => "请求超时必须在 1 到 300 秒之间",
            Self::UnsupportedImageSize => "图片尺寸不在支持列表中",
            Self::UnsupportedImageWatermark => "当前图片服务协议不支持水印参数",
        })
    }
}

impl std::error::Error for ProviderParameterError {}

/// 文本 Provider 的中立请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGenerationRequest {
    /// 调用使用的模型名。
    pub model: String,
    /// 已在本地渲染并冻结的 Prompt。
    pub prompt: String,
    /// 当前阶段冻结的有界生成参数。
    pub parameters: TextGenerationParameters,
    /// 同一任务包与阶段重复请求时保持稳定的幂等键。
    pub idempotency_key: String,
}

/// 文本 Provider 的中立成功结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGenerationResult {
    /// 经过结构校验的生成文本。
    pub text: String,
    /// Provider 返回的请求 ID；缺失时为空。
    pub provider_request_id: Option<String>,
    /// Provider 实际返回的逐字段 token 用量；完全缺失时为空。
    pub token_usage: Option<crate::TokenUsage>,
}

/// 图片 Provider 的中立请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationRequest {
    /// 调用使用的模型名。
    pub model: String,
    /// 已在本地渲染并冻结的图片 Prompt。
    pub prompt: String,
    /// 当前阶段冻结的有界生成参数。
    pub parameters: ImageGenerationParameters,
    /// 同一任务包与阶段重复请求时保持稳定的幂等键。
    pub idempotency_key: String,
}

/// 文本生成端口使用的受管异步结果。
pub type TextGenerationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TextGenerationResult, ProviderError>> + Send + 'a>>;

/// 图片生成端口使用的受管异步结果。
pub type ImageGenerationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ImageGenerationResult, ProviderError>> + Send + 'a>>;

/// 与厂商和宿主无关的文本生成端口。
pub trait TextGenerator: Send + Sync {
    /// 根据冻结请求生成文本。
    fn generate_text<'a>(&'a self, request: &'a TextGenerationRequest) -> TextGenerationFuture<'a>;
}

/// 与厂商和宿主无关的图片生成端口。
pub trait ImageGenerator: Send + Sync {
    /// 根据冻结请求生成图片。
    fn generate_image<'a>(
        &'a self,
        request: &'a ImageGenerationRequest,
    ) -> ImageGenerationFuture<'a>;
}

/// 图片 Provider 的中立成功结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationResult {
    /// Provider 返回的图片 URL。
    pub image_url: Option<String>,
    /// Provider 返回的 Base64 图片；适配器应尽快写入受管文件并清理。
    pub image_base64: Option<String>,
    /// Provider 返回的请求 ID；缺失时为空。
    pub provider_request_id: Option<String>,
    /// Provider 实际返回的逐字段 token 用量；完全缺失时为空。
    pub token_usage: Option<crate::TokenUsage>,
}

/// 外部 AI 失败的稳定领域分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderErrorKind {
    /// 该步骤尚未绑定完整 Provider 配置。
    NotConfigured,
    /// 凭据无效或权限不足。
    Authentication,
    /// 请求参数不被 Provider 接受。
    InvalidRequest,
    /// Provider 触发限流。
    RateLimited,
    /// 调用超过配置的等待边界。
    Timeout,
    /// DNS、TLS、连接或其他传输层失败。
    Transport,
    /// Provider 服务端失败。
    Server,
    /// 内容被安全策略拒绝。
    ContentRejected,
    /// 成功响应缺少必需字段或产物为空。
    InvalidResponse,
    /// 用户或应用生命周期取消了调用。
    Cancelled,
    /// 连接中断后无法确认 Provider 是否已处理请求。
    UnknownOutcome,
}

/// 可安全展示且不携带凭据或原始响应的 Provider 错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    /// 稳定错误分类。
    pub kind: ProviderErrorKind,
    /// 不含敏感信息的用户可见说明。
    pub safe_message: String,
    /// 建议等待多久后重试；仅限流等场景使用。
    pub retry_after_ms: Option<u64>,
}

impl ProviderError {
    /// 根据 HTTP 状态码建立稳定分类，不保留响应正文。
    pub fn from_http_status(status: u16, retry_after_ms: Option<u64>) -> Self {
        let (kind, safe_message) = match status {
            401 | 403 => (
                ProviderErrorKind::Authentication,
                "AI 服务鉴权失败，请检查凭据",
            ),
            400 | 404 | 409 | 422 => (
                ProviderErrorKind::InvalidRequest,
                "AI 服务拒绝了当前请求或模型配置",
            ),
            408 => (ProviderErrorKind::Timeout, "AI 服务响应超时，请稍后重试"),
            429 => (ProviderErrorKind::RateLimited, "AI 服务繁忙，请稍后重试"),
            500..=599 => (ProviderErrorKind::Server, "AI 服务暂时不可用，请稍后重试"),
            _ => (
                ProviderErrorKind::InvalidResponse,
                "AI 服务返回了无法识别的响应",
            ),
        };
        Self {
            kind,
            safe_message: safe_message.into(),
            retry_after_ms,
        }
    }

    /// 返回 core 是否允许对该分类执行有上限的普通重试。
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            ProviderErrorKind::RateLimited
                | ProviderErrorKind::Timeout
                | ProviderErrorKind::Transport
                | ProviderErrorKind::Server
                | ProviderErrorKind::UnknownOutcome
        )
    }

    /// 转换为文章状态机使用的稳定失败对象。
    pub fn as_failure(&self) -> crate::Failure {
        crate::Failure {
            code: match self.kind {
                ProviderErrorKind::NotConfigured => "PROVIDER_NOT_CONFIGURED",
                ProviderErrorKind::Authentication => "PROVIDER_AUTHENTICATION",
                ProviderErrorKind::InvalidRequest => "PROVIDER_INVALID_REQUEST",
                ProviderErrorKind::RateLimited => "PROVIDER_RATE_LIMITED",
                ProviderErrorKind::Timeout => "PROVIDER_TIMEOUT",
                ProviderErrorKind::Transport => "PROVIDER_TRANSPORT",
                ProviderErrorKind::Server => "PROVIDER_SERVER",
                ProviderErrorKind::ContentRejected => "PROVIDER_CONTENT_REJECTED",
                ProviderErrorKind::InvalidResponse => "PROVIDER_INVALID_RESPONSE",
                ProviderErrorKind::Cancelled => "PROVIDER_CANCELLED",
                ProviderErrorKind::UnknownOutcome => "PROVIDER_UNKNOWN_OUTCOME",
            }
            .into(),
            message: self.safe_message.clone(),
            retryable: self.is_retryable(),
            retry_after_ms: self.retry_after_ms,
        }
    }
}

#[cfg(test)]
mod tests;
