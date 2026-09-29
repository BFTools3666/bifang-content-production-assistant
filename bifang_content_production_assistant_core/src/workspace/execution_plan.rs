//! 不含密钥的执行计划：模板、服务和 wire 契约都由 Core 决定。

use super::*;
use crate::{CURRENT_PROMPT_RENDERER_VERSION, PromptTemplateKind, ProviderCapability};

/// 一次运行冻结的全部非秘密输入，宿主只按引用装入密钥。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Prompt 渲染算法版本。
    pub prompt_renderer_version: u32,
    /// 图片请求与响应协议版本。
    pub image_request_contract: ImageRequestContract,
    /// 正文 Prompt 的不可变版本。
    pub article_template: PromptTemplate,
    /// 简介 Prompt 的不可变版本。
    pub summary_template: PromptTemplate,
    /// 配图 Prompt 的不可变版本。
    pub image_template: PromptTemplate,
    /// 正文 Provider 的非秘密修订与凭据引用。
    pub article_provider: Option<ProviderExecutionBinding>,
    /// 简介 Provider 的非秘密修订与凭据引用。
    pub summary_provider: Option<ProviderExecutionBinding>,
    /// 配图 Provider 的非秘密修订与凭据引用。
    pub image_provider: Option<ProviderExecutionBinding>,
}

impl ExecutionPlan {
    /// 从工作区当前配置选择最新模板和目标阶段所需的默认 Provider。
    pub fn from_workspace(
        workspace: &WorkspaceData,
        allowed_kind: ProviderKind,
        target_stage: Option<WorkflowStage>,
    ) -> Result<Self, String> {
        let template = |kind| {
            workspace
                .prompt_templates
                .iter()
                .filter(|template| template.kind == kind)
                .max_by_key(|template| template.version)
                .cloned()
                .ok_or_else(|| "缺少必需 Prompt 模板".to_string())
        };
        let bind = |provider: &ProviderConfig| ProviderExecutionBinding {
            credential_ref: (provider.kind != ProviderKind::Fixture).then(|| {
                workspace
                    .provider_credential_refs
                    .get(&provider.id)
                    .cloned()
                    .unwrap_or_else(|| provider.id.clone())
            }),
            config: provider.clone(),
        };
        let choose = |stage, capability, is_default: fn(&ProviderConfig) -> bool| {
            workspace
                .provider_configs
                .iter()
                .find(|provider| {
                    target_stage.is_none_or(|target| target == stage)
                        && provider.enabled
                        && provider_kind_allowed(provider.kind, allowed_kind, capability)
                        && is_default(provider)
                })
                .map(bind)
        };
        let article_provider = choose(
            WorkflowStage::Article,
            ProviderCapability::Text,
            |provider| provider.default_for_article,
        );
        let summary_provider = choose(
            WorkflowStage::Summary,
            ProviderCapability::Text,
            |provider| provider.default_for_summary,
        );
        let image_provider = choose(
            WorkflowStage::Image,
            ProviderCapability::Image,
            |provider| provider.default_for_image,
        );
        let image_request_contract = image_provider
            .as_ref()
            .map(|provider| ImageRequestContract::for_provider_kind(provider.config.kind))
            .unwrap_or(ImageRequestContract::CURRENT);
        Ok(Self {
            prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
            image_request_contract,
            article_template: template(PromptTemplateKind::Article)?,
            summary_template: template(PromptTemplateKind::Summary)?,
            image_template: template(PromptTemplateKind::Image)?,
            article_provider,
            summary_provider,
            image_provider,
        })
    }

    /// 普通重试使用历史冻结绑定，不读取当前模板或 Provider 修订。
    pub fn from_binding(
        binding: &PackageExecutionBinding,
        allowed_kind: ProviderKind,
    ) -> Result<Self, String> {
        for (provider, capability) in [
            (binding.article_provider.as_ref(), ProviderCapability::Text),
            (binding.summary_provider.as_ref(), ProviderCapability::Text),
            (binding.image_provider.as_ref(), ProviderCapability::Image),
        ] {
            if provider.is_some_and(|bound| {
                !provider_kind_allowed(bound.config.kind, allowed_kind, capability)
            }) {
                return Err("文章包绑定的 AI 服务与当前运行模式不一致".into());
            }
        }
        Ok(Self {
            prompt_renderer_version: binding.prompt_renderer_version,
            image_request_contract: binding
                .image_request_contract
                .ok_or_else(|| "执行绑定缺少图片请求契约".to_string())?,
            article_template: binding.article_template.clone(),
            summary_template: binding.summary_template.clone(),
            image_template: binding.image_template.clone(),
            article_provider: binding.article_provider.clone(),
            summary_provider: binding.summary_provider.clone(),
            image_provider: binding.image_provider.clone(),
        })
    }

    /// 新批次必须同时具有三类已启用的默认 AI 服务。
    pub fn validate_for_new_run(&self) -> Result<(), String> {
        validate_execution_providers(
            self.article_provider.is_some(),
            self.summary_provider.is_some(),
            self.image_provider.is_some(),
            None,
        )
    }

    /// 单步只要求目标 AI 服务，本地格式阶段无需 AI 服务。
    pub fn validate_for_stage(&self, stage: WorkflowStage) -> Result<(), String> {
        validate_execution_providers(
            self.article_provider.is_some(),
            self.summary_provider.is_some(),
            self.image_provider.is_some(),
            Some(stage),
        )
    }

    /// 同一包后续单步保留首次选择的渲染算法。
    pub fn use_prompt_renderer_version(&mut self, version: u32) -> Result<(), String> {
        validate_prompt_renderer_version(version)?;
        self.prompt_renderer_version = version;
        Ok(())
    }
}

/// 新批次预检先验证模板、默认服务与非秘密凭据引用，避免宿主猜测密钥。
pub fn validate_execution_configuration_for_kind(
    workspace: &WorkspaceData,
    allowed_kind: ProviderKind,
) -> Result<(), String> {
    for (kind, stage_name) in [
        (PromptTemplateKind::Article, "正文"),
        (PromptTemplateKind::Summary, "简介"),
        (PromptTemplateKind::Image, "配图"),
    ] {
        if !workspace
            .prompt_templates
            .iter()
            .any(|template| template.kind == kind)
        {
            return Err(format!(
                "缺少{stage_name} Prompt 模板，请前往 Prompt 模板配置后重试"
            ));
        }
    }

    let stages: [(&str, ProviderCapability, fn(&ProviderConfig) -> bool); 3] = [
        (
            "正文",
            ProviderCapability::Text,
            |provider: &ProviderConfig| provider.default_for_article,
        ),
        (
            "简介",
            ProviderCapability::Text,
            |provider: &ProviderConfig| provider.default_for_summary,
        ),
        (
            "配图",
            ProviderCapability::Image,
            |provider: &ProviderConfig| provider.default_for_image,
        ),
    ];
    for (stage_name, capability, is_default) in stages {
        let provider = workspace
            .provider_configs
            .iter()
            .find(|provider| is_default(provider))
            .ok_or_else(|| {
                format!("{stage_name}阶段尚未选择默认 AI 服务，请前往 AI 服务配置后重试")
            })?;
        if !provider.enabled {
            return Err(format!(
                "{stage_name}阶段的默认 AI 服务“{}”已停用，请先启用或选择其他服务",
                provider.name
            ));
        }
        if !provider_kind_allowed(provider.kind, allowed_kind, capability) {
            return Err(format!(
                "{stage_name}阶段的默认 AI 服务“{}”不支持当前阶段或运行模式，请选择其他服务",
                provider.name
            ));
        }
        if provider.kind != ProviderKind::Fixture
            && !workspace
                .provider_credential_refs
                .get(&provider.id)
                .is_some_and(|reference| !reference.trim().is_empty())
        {
            return Err(format!(
                "{stage_name}阶段的默认 AI 服务“{}”尚未登记 API 密钥，请前往 AI 服务配置保存密钥后重试",
                provider.name
            ));
        }
    }
    Ok(())
}

/// 校验当前工作区或历史绑定的 Provider 种类与运行模式。
pub fn provider_kind_allowed(
    kind: ProviderKind,
    allowed_kind: ProviderKind,
    capability: ProviderCapability,
) -> bool {
    if allowed_kind == ProviderKind::Fixture {
        kind == ProviderKind::Fixture
    } else {
        kind != ProviderKind::Fixture && kind.supports(capability)
    }
}

/// 校验批次或单阶段的服务完整性，保持原有稳定提示。
pub fn validate_execution_providers(
    article: bool,
    summary: bool,
    image: bool,
    target_stage: Option<WorkflowStage>,
) -> Result<(), String> {
    match target_stage {
        None if article && summary && image => Ok(()),
        None => Err("正文、简介和配图阶段都必须配置已启用的默认 AI 服务".into()),
        Some(WorkflowStage::Article) if article => Ok(()),
        Some(WorkflowStage::Summary) if summary => Ok(()),
        Some(WorkflowStage::Image) if image => Ok(()),
        Some(WorkflowStage::Format) => Ok(()),
        Some(stage) => {
            let name = match stage {
                WorkflowStage::Article => "正文",
                WorkflowStage::Summary => "简介",
                WorkflowStage::Image => "配图",
                WorkflowStage::Format => unreachable!(),
            };
            Err(format!("{name}阶段尚未配置已启用的默认 AI 服务"))
        }
    }
}

/// 旧包只允许使用已定义的 Prompt renderer。
pub fn validate_prompt_renderer_version(version: u32) -> Result<(), String> {
    if matches!(
        version,
        LEGACY_PROMPT_RENDERER_VERSION | CURRENT_PROMPT_RENDERER_VERSION
    ) {
        Ok(())
    } else {
        Err(format!("文章包绑定了不受支持的 Prompt renderer v{version}"))
    }
}
