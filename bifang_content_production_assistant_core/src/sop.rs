//! 固定内容 SOP 的只读产品描述与执行依赖事实。

use serde::Serialize;

use crate::WorkflowStage;

const NO_DEPENDENCIES: [WorkflowStage; 0] = [];
const ARTICLE_DEPENDENCY: [WorkflowStage; 1] = [WorkflowStage::Article];
const ARTICLE_AND_SUMMARY_DEPENDENCIES: [WorkflowStage; 2] =
    [WorkflowStage::Article, WorkflowStage::Summary];
const ALL_PREVIOUS_STAGE_DEPENDENCIES: [WorkflowStage; 3] = [
    WorkflowStage::Article,
    WorkflowStage::Summary,
    WorkflowStage::Image,
];
const REQUIRED_REVIEW_STAGES: [WorkflowStage; 4] = WorkflowStage::ordered();

/// 标识固定 SOP 每个阶段形成的业务产物种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowOutputKind {
    /// 供后续阶段复用的 Markdown 规范正文。
    MarkdownArticle,
    /// 绑定指定正文版本的文章简介。
    ArticleSummary,
    /// 绑定标题、正文和简介来源的文章配图。
    ArticleImage,
    /// 从规范正文派生且可复制的 HTML。
    HtmlDocument,
}

impl WorkflowOutputKind {
    /// 返回跨适配器稳定的产物标识，供人类界面渲染而不复制业务映射。
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::MarkdownArticle => "markdownArticle",
            Self::ArticleSummary => "articleSummary",
            Self::ArticleImage => "articleImage",
            Self::HtmlDocument => "htmlDocument",
        }
    }
}

/// 描述一个 SOP 阶段产物的稳定种类与业务语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowOutputDefinition {
    /// 供跨接口稳定识别的产物种类。
    pub id: WorkflowOutputKind,
    /// 说明产物如何形成以及在文章包中的用途。
    pub semantics: &'static str,
}

/// 描述一个可独立执行和重试的固定 SOP 阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStageDefinition {
    /// 复用领域状态机的稳定阶段标识。
    pub id: WorkflowStage,
    /// 阶段在固定执行顺序中的一基序号。
    pub order: u8,
    /// 阶段开始前必须成功的执行前置阶段。
    pub depends_on: &'static [WorkflowStage],
    /// 当前阶段成功后形成的业务产物。
    pub output: WorkflowOutputDefinition,
}

/// 描述进入人工审核前必须满足的固定领域门禁。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowReviewGate {
    /// 进入待审核前必须全部成功的固定阶段。
    pub required_stages: &'static [WorkflowStage],
    /// 标识阶段成功之外是否还必须具备完整产物与血缘。
    pub requires_complete_artifacts: bool,
}

/// 汇总固定内容 SOP 的阶段顺序、执行依赖、产物和审核门禁。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    /// 按实际执行顺序排列的四个阶段。
    pub stages: [WorkflowStageDefinition; 4],
    /// 全部阶段结束后进入待审核的条件。
    pub review_gate: WorkflowReviewGate,
}

impl WorkflowStage {
    /// 返回跨适配器稳定的阶段标识，避免 CLI 或 GUI 重建领域映射。
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Summary => "summary",
            Self::Image => "image",
            Self::Format => "format",
        }
    }

    /// 返回阶段开始前必须成功的执行前置阶段。
    pub const fn execution_dependencies(self) -> &'static [Self] {
        match self {
            Self::Article => &NO_DEPENDENCIES,
            Self::Summary => &ARTICLE_DEPENDENCY,
            Self::Image => &ARTICLE_AND_SUMMARY_DEPENDENCIES,
            Self::Format => &ALL_PREVIOUS_STAGE_DEPENDENCIES,
        }
    }

    /// 返回当前阶段成功后形成的产物定义。
    pub const fn output_definition(self) -> WorkflowOutputDefinition {
        match self {
            Self::Article => WorkflowOutputDefinition {
                id: WorkflowOutputKind::MarkdownArticle,
                semantics: "Markdown 规范正文，是简介、配图和 HTML 的规范内容来源。",
            },
            Self::Summary => WorkflowOutputDefinition {
                id: WorkflowOutputKind::ArticleSummary,
                semantics: "基于指定 Markdown 正文版本生成的文章简介。",
            },
            Self::Image => WorkflowOutputDefinition {
                id: WorkflowOutputKind::ArticleImage,
                semantics: "基于标题、正文摘要和图片 Prompt 生成的一张配图。",
            },
            Self::Format => WorkflowOutputDefinition {
                id: WorkflowOutputKind::HtmlDocument,
                semantics: "从规范 Markdown 正文派生的可复制 HTML；规范正文继续保留在同一文章包中。",
            },
        }
    }
}

/// 返回不读取外部状态、没有副作用的固定内容 SOP 产品描述。
pub fn describe_fixed_content_sop() -> WorkflowDefinition {
    let ordered = WorkflowStage::ordered();
    let stages = std::array::from_fn(|index| {
        let stage = ordered[index];
        WorkflowStageDefinition {
            id: stage,
            order: index as u8 + 1,
            depends_on: stage.execution_dependencies(),
            output: stage.output_definition(),
        }
    });

    WorkflowDefinition {
        stages,
        review_gate: WorkflowReviewGate {
            required_stages: &REQUIRED_REVIEW_STAGES,
            requires_complete_artifacts: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证公开 SOP 描述精确复用运行时顺序，并锁定执行依赖、产物和审核门禁。
    #[test]
    fn describes_runtime_order_dependencies_outputs_and_review_gate() {
        let definition = describe_fixed_content_sop();
        let described_stages = definition
            .stages
            .iter()
            .map(|stage| stage.id)
            .collect::<Vec<_>>();

        assert_eq!(described_stages, WorkflowStage::ordered());
        assert_eq!(
            definition
                .stages
                .iter()
                .map(|stage| stage.order)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(definition.stages[0].depends_on, &[]);
        assert_eq!(definition.stages[1].depends_on, &[WorkflowStage::Article]);
        assert_eq!(
            definition.stages[2].depends_on,
            &[WorkflowStage::Article, WorkflowStage::Summary]
        );
        assert_eq!(
            definition.stages[3].depends_on,
            &[
                WorkflowStage::Article,
                WorkflowStage::Summary,
                WorkflowStage::Image,
            ]
        );
        assert_eq!(
            definition.stages.map(|stage| stage.output.id),
            [
                WorkflowOutputKind::MarkdownArticle,
                WorkflowOutputKind::ArticleSummary,
                WorkflowOutputKind::ArticleImage,
                WorkflowOutputKind::HtmlDocument,
            ]
        );
        assert!(
            definition
                .stages
                .iter()
                .all(|stage| !stage.output.semantics.trim().is_empty())
        );
        assert_eq!(
            definition.review_gate.required_stages,
            &WorkflowStage::ordered()
        );
        assert!(definition.review_gate.requires_complete_artifacts);
    }
}
