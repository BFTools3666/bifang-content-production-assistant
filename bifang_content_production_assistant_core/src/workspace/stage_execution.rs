//! 纯领域执行请求：固定模型、Prompt、参数、幂等键、HTML 和产物血缘。

use std::collections::BTreeMap;

use pulldown_cmark::{Options, Parser, html};

use super::{ExecutionPlan, ImageRequestContract};
use crate::{
    ArticleTask, ArtifactLineage, ImageGenerationParameters, PromptTemplate, ProviderConfig,
    ProviderError, ProviderErrorKind, TextGenerationParameters, WorkflowStage,
    render_prompt_with_version,
};

/// 一次 AI 请求的类型化参数，避免文本与图片参数在宿主混用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageGenerationParameters {
    /// 正文或简介参数。
    Text(TextGenerationParameters),
    /// 配图参数。
    Image(ImageGenerationParameters),
}

/// 单阶段的非秘密执行计划；宿主只负责装载密钥、限流、I/O 和持久化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageExecutionPlan {
    /// 本地格式化，任何 AI 服务和凭据均不参与。
    LocalFormat {
        /// 已渲染的非空 HTML 字符串。
        html: String,
        /// 指向真实正文版本的产物血缘。
        lineage: ArtifactLineage,
    },
    /// 某个冻结 Provider 的请求。
    ProviderRequest {
        /// 冻结的非秘密 Provider 配置修订。
        provider: ProviderConfig,
        /// 阶段对应的显式模型。
        model: String,
        /// 使用冻结 renderer 与模板得到的 Prompt。
        prompt: String,
        /// 与阶段类型匹配的生成参数。
        parameters: StageGenerationParameters,
        /// 包含 wire 契约和参数的稳定幂等键。
        idempotency_key: String,
        /// 阶段产物的来源版本与执行修订。
        lineage: ArtifactLineage,
    },
}

impl StageExecutionPlan {
    /// 从文章与无密钥冻结绑定决定全部纯业务执行输入。
    pub fn from_task(
        task: &ArticleTask,
        stage: WorkflowStage,
        execution: &ExecutionPlan,
        attempt_id: String,
    ) -> Result<Self, ProviderError> {
        if stage == WorkflowStage::Format {
            let markdown = task.artifacts.markdown.as_deref().ok_or_else(|| {
                plan_error(
                    ProviderErrorKind::InvalidResponse,
                    "正文产物缺失，无法生成 HTML",
                )
            })?;
            let mut rendered = String::new();
            html::push_html(&mut rendered, Parser::new_ext(markdown, Options::all()));
            if rendered.trim().is_empty() {
                return Err(plan_error(
                    ProviderErrorKind::InvalidResponse,
                    "Markdown 无法生成非空 HTML",
                ));
            }
            return Ok(Self::LocalFormat {
                html: rendered,
                lineage: stage_lineage(task, stage, None, None, None, attempt_id),
            });
        }

        let (template, bound) = match stage {
            WorkflowStage::Article => (
                &execution.article_template,
                execution.article_provider.as_ref(),
            ),
            WorkflowStage::Summary => (
                &execution.summary_template,
                execution.summary_provider.as_ref(),
            ),
            WorkflowStage::Image => (&execution.image_template, execution.image_provider.as_ref()),
            WorkflowStage::Format => unreachable!("local format returned before Provider lookup"),
        };
        let provider = bound.map(|binding| &binding.config).ok_or_else(|| {
            plan_error(ProviderErrorKind::NotConfigured, "当前步骤尚未选择 AI 服务")
        })?;
        let (model, parameters, parameter_material) = match stage {
            WorkflowStage::Article => {
                let parameters = provider.article_parameters();
                (
                    provider.article_model().to_string(),
                    StageGenerationParameters::Text(parameters.clone()),
                    parameters.idempotency_material(),
                )
            }
            WorkflowStage::Summary => {
                let parameters = provider.summary_parameters();
                (
                    provider.summary_model().to_string(),
                    StageGenerationParameters::Text(parameters.clone()),
                    parameters.idempotency_material(),
                )
            }
            WorkflowStage::Image => {
                let parameters = provider.image_parameters();
                let material = image_request_idempotency_material(
                    &parameters.idempotency_material(),
                    execution.image_request_contract,
                );
                (
                    provider.image_model.clone(),
                    StageGenerationParameters::Image(parameters),
                    material,
                )
            }
            WorkflowStage::Format => unreachable!("local format returned before model lookup"),
        };
        let prompt = render_stage_prompt(task, template, execution.prompt_renderer_version)
            .map_err(|message| plan_error(ProviderErrorKind::InvalidRequest, message))?;
        let idempotency_key = task.stage_idempotency_key(
            stage,
            &provider.id,
            provider.revision,
            &provider.base_url,
            &template.id,
            template.version,
            &template.content,
            &model,
            &prompt,
            &parameter_material,
        );
        Ok(Self::ProviderRequest {
            provider: provider.clone(),
            model: model.clone(),
            prompt,
            parameters,
            idempotency_key,
            lineage: stage_lineage(
                task,
                stage,
                Some(template),
                Some(provider),
                Some(model),
                attempt_id,
            ),
        })
    }
}

fn plan_error(kind: ProviderErrorKind, safe_message: impl Into<String>) -> ProviderError {
    ProviderError {
        kind,
        safe_message: safe_message.into(),
        retry_after_ms: None,
    }
}

fn stage_lineage(
    task: &ArticleTask,
    stage: WorkflowStage,
    template: Option<&PromptTemplate>,
    provider: Option<&ProviderConfig>,
    model: Option<String>,
    attempt_id: String,
) -> ArtifactLineage {
    ArtifactLineage {
        package_version: task.package_version,
        source_article_version: source_article_version(task, stage),
        source_summary_version: source_summary_version(task, stage),
        template_id: template.map(|template| template.id.clone()),
        template_version: template.map(|template| template.version),
        provider_id: provider.map(|provider| provider.id.clone()),
        provider_revision: provider.map(|provider| provider.revision),
        model,
        attempt_id,
    }
}

/// 冻结 wire 契约与图片参数共同纳入幂等身份；首版空材料保持字节不变。
pub fn image_request_idempotency_material(
    parameter_material: &str,
    contract: ImageRequestContract,
) -> String {
    let contract_material = contract.idempotency_material();
    match (parameter_material.is_empty(), contract_material.is_empty()) {
        (true, true) => String::new(),
        (false, true) => parameter_material.to_string(),
        (true, false) => contract_material.to_string(),
        (false, false) => format!("{parameter_material};{contract_material}"),
    }
}

/// 用文章包冻结的 Prompt renderer 渲染目标阶段输入。
pub fn render_stage_prompt(
    task: &ArticleTask,
    template: &PromptTemplate,
    renderer_version: u32,
) -> Result<String, String> {
    let values = BTreeMap::from([
        ("title".into(), task.title.clone()),
        ("keywords".into(), task.keywords.join("、")),
        ("requirements".into(), task.requirements.clone()),
        (
            "article".into(),
            task.artifacts.markdown.clone().unwrap_or_default(),
        ),
        (
            "summary".into(),
            task.artifacts.summary.clone().unwrap_or_default(),
        ),
    ]);
    render_prompt_with_version(template, &values, renderer_version)
        .map_err(|error| error.to_string())
}

/// 派生阶段真实读取的正文产物版本，而非当前包版本。
pub fn source_article_version(task: &ArticleTask, stage: WorkflowStage) -> Option<u32> {
    (stage != WorkflowStage::Article)
        .then_some(task.stages.article.artifact_version)
        .flatten()
}

/// 当前仅图片直接读取简介产物版本。
pub fn source_summary_version(task: &ArticleTask, stage: WorkflowStage) -> Option<u32> {
    (stage == WorkflowStage::Image)
        .then_some(task.stages.summary.artifact_version)
        .flatten()
}
