//! 连接测试与方舟诊断的无秘密领域选择。

use crate::{
    ImageGenerationParameters, ImageGenerationRequest, ProviderCapability, ProviderConfig,
    ProviderKind, TextGenerationParameters, TextGenerationRequest, WorkflowStage,
};

use super::WorkspaceData;

const TEXT_PROBE_PROMPT: &str = "请只回复：连接成功";
const IMAGE_PROBE_PROMPT: &str = "一张极简测试图片：白色背景中央只有一个蓝色圆形，画面清晰。";

/// 连接测试在访问凭据或网络前的稳定拒绝原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderProbeError {
    /// 找不到所选服务。
    ProviderNotFound,
    /// 文本测试只接受正文和简介阶段。
    TextStageRequired,
    /// 测试夹具未显式启用。
    FixtureDisabled,
    /// 服务已停用。
    ProviderDisabled,
    /// 服务不支持所选文本阶段。
    TextStageUnsupported,
    /// 文本模型未保存。
    TextModelRequired,
    /// 可能计费的图片测试未获本次确认。
    ImageConfirmationRequired,
    /// 服务修订已变化。
    ProviderRevisionConflict,
    /// 测试夹具不能发起真实图片测试。
    FixtureReadOnly,
    /// 服务不支持图片生成。
    ImageStageUnsupported,
    /// 图片模型未保存。
    ImageModelRequired,
    /// 图片参数无效。
    ImageParametersInvalid,
    /// 找不到已启用的方舟服务。
    ArkProviderNotFound,
    /// 多个方舟服务且没有唯一默认项。
    ArkProviderAmbiguous,
    /// 多个方舟服务被设为默认项。
    ArkDefaultAmbiguous,
    /// 方舟模型未保存。
    ArkModelRequired,
    /// 方舟图片参数无效。
    ArkParametersInvalid,
    /// 所选方舟凭据正在等待删除。
    ArkCredentialPendingDeletion,
    /// 方舟真实诊断未获本次付费确认。
    ArkPaidConfirmationRequired,
}

impl ProviderProbeError {
    /// 返回适配器可直接传递的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::ProviderNotFound => "PROVIDER_NOT_FOUND",
            Self::TextStageRequired => "PROVIDER_TEXT_STAGE_REQUIRED",
            Self::FixtureDisabled => "FIXTURE_DISABLED",
            Self::ProviderDisabled => "PROVIDER_DISABLED",
            Self::TextStageUnsupported | Self::ImageStageUnsupported => {
                "PROVIDER_STAGE_UNSUPPORTED"
            }
            Self::TextModelRequired => "PROVIDER_TEXT_MODEL_REQUIRED",
            Self::ImageConfirmationRequired => "PROVIDER_IMAGE_TEST_CONFIRMATION_REQUIRED",
            Self::ProviderRevisionConflict => "PROVIDER_REVISION_CONFLICT",
            Self::FixtureReadOnly => "FIXTURE_READ_ONLY",
            Self::ImageModelRequired => "PROVIDER_IMAGE_MODEL_REQUIRED",
            Self::ImageParametersInvalid => "PROVIDER_PARAMETER_INVALID",
            Self::ArkProviderNotFound => "PROBE_ARK_PROVIDER_NOT_FOUND",
            Self::ArkProviderAmbiguous => "PROBE_ARK_PROVIDER_AMBIGUOUS",
            Self::ArkDefaultAmbiguous => "PROBE_ARK_DEFAULT_AMBIGUOUS",
            Self::ArkModelRequired => "PROBE_ARK_MODEL_REQUIRED",
            Self::ArkParametersInvalid => "PROBE_ARK_PARAMETERS_INVALID",
            Self::ArkCredentialPendingDeletion => "PROBE_CREDENTIAL_PENDING_DELETION",
            Self::ArkPaidConfirmationRequired => "PROBE_PAID_CONFIRMATION_REQUIRED",
        }
    }

    /// 返回不包含凭据或外部响应的用户提示。
    pub const fn message(self) -> &'static str {
        match self {
            Self::ProviderNotFound => "找不到指定 AI 服务",
            Self::TextStageRequired => "连接测试只接受正文或简介文本阶段",
            Self::FixtureDisabled => "测试夹具没有在当前环境启用",
            Self::ProviderDisabled => "该 AI 服务已停用，请先启用后再测试连接",
            Self::TextStageUnsupported => "该 AI 服务仅支持图片阶段，不能用于正文或简介连接测试",
            Self::TextModelRequired => "所选文本阶段尚未保存模型",
            Self::ImageConfirmationRequired => "生成测试图片前必须明确确认可能产生费用",
            Self::ProviderRevisionConflict => "AI 服务已被其他操作更新，请刷新后重试",
            Self::FixtureReadOnly => "测试夹具不能用于真实图片连接测试",
            Self::ImageStageUnsupported => "该 AI 服务不支持图片生成",
            Self::ImageModelRequired => "图片测试需要先保存图片模型",
            Self::ImageParametersInvalid => "AI 图片生成参数超出允许范围",
            Self::ArkProviderNotFound => "没有找到已启用的火山方舟图片服务",
            Self::ArkProviderAmbiguous => "存在多个已启用的方舟服务，请明确选择",
            Self::ArkDefaultAmbiguous => "存在多个默认方舟图片服务，请先修正配置",
            Self::ArkModelRequired => "火山方舟图片模型尚未配置",
            Self::ArkParametersInvalid => "火山方舟图片参数超出允许范围",
            Self::ArkCredentialPendingDeletion => "图片服务凭据正在等待安全清理，请先重新保存密钥",
            Self::ArkPaidConfirmationRequired => "执行真实方舟图片生成前必须明确确认可能产生费用",
        }
    }
}

/// 文本连接测试的零出站夹具或真实请求计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextProbeSelection {
    /// 显式启用的测试夹具，仅返回非真实验证结果。
    Fixture,
    /// 已通过领域校验且不含密钥的真实测试计划。
    Request(TextProbePlan),
}

/// 一次正文或简介文本连接测试的冻结非秘密输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextProbePlan {
    /// 已选择的服务配置。
    pub provider: ProviderConfig,
    /// 系统凭据引用，不是密钥内容。
    pub credential_ref: String,
    /// 所选阶段的模型。
    pub model: String,
    /// 最小测试请求参数。
    pub parameters: TextGenerationParameters,
    /// 不含文章数据的固定提示词。
    pub prompt: &'static str,
}

impl TextProbePlan {
    /// 使用宿主生成的一次性 nonce 构造最小文本请求。
    pub fn request_with_nonce(&self, nonce: &str) -> TextGenerationRequest {
        TextGenerationRequest {
            model: self.model.clone(),
            prompt: self.prompt.into(),
            parameters: self.parameters.clone(),
            idempotency_key: format!("provider-test:{}:{nonce}", self.provider.id),
        }
    }
}

/// 一次真实图片连接测试的冻结非秘密输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageProbePlan {
    /// 已选择的服务配置。
    pub provider: ProviderConfig,
    /// 系统凭据引用，不是密钥内容。
    pub credential_ref: String,
    /// 已保存图片参数。
    pub parameters: ImageGenerationParameters,
    /// 不含文章数据的固定提示词。
    pub prompt: &'static str,
}

impl ImageProbePlan {
    /// 使用宿主生成的一次性 nonce 构造图片测试请求。
    pub fn request_with_nonce(&self, nonce: &str) -> ImageGenerationRequest {
        ImageGenerationRequest {
            model: self.provider.image_model.clone(),
            prompt: self.prompt.into(),
            parameters: self.parameters.clone(),
            idempotency_key: format!(
                "provider-image-test:{}:{}:{nonce}",
                self.provider.id, self.provider.revision
            ),
        }
    }
}

/// 方舟单次诊断的冻结非秘密输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArkProbePlan {
    /// 已选择的方舟服务配置。
    pub provider: ProviderConfig,
    /// 系统凭据引用，不是密钥内容。
    pub credential_ref: String,
    /// 已保存图片参数。
    pub parameters: ImageGenerationParameters,
    /// 不含文章数据的固定提示词。
    pub prompt: &'static str,
}

impl ArkProbePlan {
    /// 使用宿主生成的一次性 nonce 构造方舟单次诊断请求。
    pub fn request_with_nonce(&self, nonce: &str) -> ImageGenerationRequest {
        ImageGenerationRequest {
            model: self.provider.image_model.clone(),
            prompt: self.prompt.into(),
            parameters: self.parameters.clone(),
            idempotency_key: format!("ark-image-probe:{nonce}"),
        }
    }
}

/// 仅选择正文或简介服务，并固定连接测试的最小请求参数。
pub fn select_text_probe(
    workspace: &WorkspaceData,
    provider_id: &str,
    stage: WorkflowStage,
    fixture_enabled: bool,
) -> Result<TextProbeSelection, ProviderProbeError> {
    let provider = workspace
        .provider_configs
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or(ProviderProbeError::ProviderNotFound)?;
    if !matches!(stage, WorkflowStage::Article | WorkflowStage::Summary) {
        return Err(ProviderProbeError::TextStageRequired);
    }
    if provider.kind == ProviderKind::Fixture {
        return if fixture_enabled {
            Ok(TextProbeSelection::Fixture)
        } else {
            Err(ProviderProbeError::FixtureDisabled)
        };
    }
    if !provider.enabled {
        return Err(ProviderProbeError::ProviderDisabled);
    }
    if !provider.kind.supports(ProviderCapability::Text) {
        return Err(ProviderProbeError::TextStageUnsupported);
    }
    let (model, mut parameters) = match stage {
        WorkflowStage::Article => (
            provider.article_model().to_string(),
            provider.article_parameters(),
        ),
        WorkflowStage::Summary => (
            provider.summary_model().to_string(),
            provider.summary_parameters(),
        ),
        WorkflowStage::Image | WorkflowStage::Format => unreachable!("text stage checked above"),
    };
    if model.trim().is_empty() {
        return Err(ProviderProbeError::TextModelRequired);
    }
    parameters.temperature_milli = None;
    parameters.max_output_tokens = None;
    parameters.timeout_ms = 30_000;
    Ok(TextProbeSelection::Request(TextProbePlan {
        credential_ref: credential_ref(workspace, &provider),
        provider,
        model,
        parameters,
        prompt: TEXT_PROBE_PROMPT,
    }))
}

/// 在任何可能计费的请求前要求本次明确确认。
pub fn require_image_probe_confirmation(confirmed: bool) -> Result<(), ProviderProbeError> {
    confirmed
        .then_some(())
        .ok_or(ProviderProbeError::ImageConfirmationRequired)
}

/// 选择已保存图片服务修订及其固定测试输入，不读取密钥。
pub fn select_image_probe(
    workspace: &WorkspaceData,
    provider_id: &str,
    expected_revision: u32,
    confirmed: bool,
) -> Result<ImageProbePlan, ProviderProbeError> {
    require_image_probe_confirmation(confirmed)?;
    let provider = workspace
        .provider_configs
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
        .ok_or(ProviderProbeError::ProviderNotFound)?;
    if provider.revision != expected_revision {
        return Err(ProviderProbeError::ProviderRevisionConflict);
    }
    if provider.kind == ProviderKind::Fixture {
        return Err(ProviderProbeError::FixtureReadOnly);
    }
    if !provider.enabled {
        return Err(ProviderProbeError::ProviderDisabled);
    }
    if !provider.kind.supports(ProviderCapability::Image) {
        return Err(ProviderProbeError::ImageStageUnsupported);
    }
    if provider.image_model.trim().is_empty() {
        return Err(ProviderProbeError::ImageModelRequired);
    }
    let parameters = provider.image_parameters();
    parameters
        .validate_for_provider_kind(provider.kind)
        .map_err(|_| ProviderProbeError::ImageParametersInvalid)?;
    Ok(ImageProbePlan {
        credential_ref: credential_ref(workspace, &provider),
        provider,
        parameters,
        prompt: IMAGE_PROBE_PROMPT,
    })
}

/// 选择唯一、已启用的方舟图片服务，并验证模型与图片参数。
pub fn select_ark_probe_provider(
    workspace: &WorkspaceData,
    requested_id: Option<&str>,
) -> Result<ArkProbePlan, ProviderProbeError> {
    let mut candidates = workspace.provider_configs.iter().filter(|provider| {
        provider.kind == ProviderKind::VolcengineArkImage
            && provider.enabled
            && requested_id.is_none_or(|id| provider.id == id)
    });
    let first = candidates
        .next()
        .cloned()
        .ok_or(ProviderProbeError::ArkProviderNotFound)?;
    let provider = if requested_id.is_some() {
        first
    } else {
        let defaults = workspace
            .provider_configs
            .iter()
            .filter(|provider| {
                provider.kind == ProviderKind::VolcengineArkImage
                    && provider.enabled
                    && provider.default_for_image
            })
            .collect::<Vec<_>>();
        match defaults.as_slice() {
            [provider] => (*provider).clone(),
            [] if candidates.next().is_none() => first,
            [] => return Err(ProviderProbeError::ArkProviderAmbiguous),
            _ => return Err(ProviderProbeError::ArkDefaultAmbiguous),
        }
    };
    if provider.image_model.trim().is_empty() {
        return Err(ProviderProbeError::ArkModelRequired);
    }
    let parameters = provider.image_parameters();
    parameters
        .validate_for_provider_kind(ProviderKind::VolcengineArkImage)
        .map_err(|_| ProviderProbeError::ArkParametersInvalid)?;
    Ok(ArkProbePlan {
        credential_ref: credential_ref(workspace, &provider),
        provider,
        parameters,
        prompt: IMAGE_PROBE_PROMPT,
    })
}

/// 拒绝已进入清理队列的方舟凭据引用。
pub fn ensure_ark_probe_credential_ready(
    workspace: &WorkspaceData,
    plan: &ArkProbePlan,
) -> Result<(), ProviderProbeError> {
    if workspace
        .pending_credential_deletions
        .contains(&plan.credential_ref)
    {
        Err(ProviderProbeError::ArkCredentialPendingDeletion)
    } else {
        Ok(())
    }
}

/// 方舟真实执行必须取得独立于只读预检的一次付费确认。
pub fn require_ark_probe_paid_confirmation(confirmed: bool) -> Result<(), ProviderProbeError> {
    confirmed
        .then_some(())
        .ok_or(ProviderProbeError::ArkPaidConfirmationRequired)
}

fn credential_ref(workspace: &WorkspaceData, provider: &ProviderConfig) -> String {
    workspace
        .provider_credential_refs
        .get(&provider.id)
        .cloned()
        .unwrap_or_else(|| provider.id.clone())
}
