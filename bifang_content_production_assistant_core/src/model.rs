//! 文章任务、阶段和产物的稳定领域模型。

use serde::{Deserialize, Serialize};

/// 固定文章 SOP 中可独立执行和重试的阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowStage {
    /// 生成 Markdown 规范正文。
    Article,
    /// 基于正文生成文章简介。
    Summary,
    /// 基于标题和内容生成配图。
    Image,
    /// 从规范正文派生 HTML。
    Format,
}

impl WorkflowStage {
    /// 返回固定 SOP 的执行顺序。
    pub const fn ordered() -> [Self; 4] {
        [Self::Article, Self::Summary, Self::Image, Self::Format]
    }
}

/// 单个 SOP 阶段的生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum StageStatus {
    /// 尚未进入执行队列。
    #[default]
    NotStarted,
    /// 已冻结输入并等待 worker 执行。
    Queued,
    /// 当前正在执行外部调用或本地转换。
    Running,
    /// 已成功且存在匹配的完整产物。
    Succeeded,
    /// 执行失败，可依据错误类别决定是否重试。
    Failed,
    /// 上游产物被编辑或替换，当前结果已不再有效。
    Stale,
    /// 用户取消了尚未完成的阶段。
    Cancelled,
    /// 应用退出或执行所有权丢失，阶段可恢复重试。
    Interrupted,
}

/// 可安全展示和持久化的稳定失败信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    /// 供程序分支处理的稳定错误码。
    pub code: String,
    /// 不包含凭据和原始响应的用户可见说明。
    pub message: String,
    /// 标识普通重试是否可能恢复。
    pub retryable: bool,
    /// Provider 建议的最短重试等待时间；没有建议时为空。
    #[serde(default)]
    pub retry_after_ms: Option<u64>,
}

/// Provider 对一次生成实际返回的 token 用量。
///
/// 每个字段独立可缺失；core 不依据其他字段推算 Provider 未返回的值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Provider 明确返回的输入 token 数。
    #[serde(default)]
    pub input_tokens: Option<u64>,
    /// Provider 明确返回的输出 token 数。
    #[serde(default)]
    pub output_tokens: Option<u64>,
    /// Provider 明确返回的总 token 数。
    #[serde(default)]
    pub total_tokens: Option<u64>,
}

impl TokenUsage {
    /// 仅在 Provider 至少返回一个 token 字段时建立真实用量记录。
    pub fn reported(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> Option<Self> {
        (input_tokens.is_some() || output_tokens.is_some() || total_tokens.is_some()).then_some(
            Self {
                input_tokens,
                output_tokens,
                total_tokens,
            },
        )
    }

    /// 本地格式转换不调用 Provider，固定记录为零 token。
    pub const fn local_processing() -> Self {
        Self {
            input_tokens: Some(0),
            output_tokens: Some(0),
            total_tokens: Some(0),
        }
    }
}

/// 一次阶段尝试的持久终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageAttemptStatus {
    /// 外部调用或本地转换正在执行。
    Running,
    /// 尝试成功并形成了对应血缘。
    Succeeded,
    /// 尝试以稳定失败结束。
    Failed,
    /// 应用退出或执行所有权丢失。
    Interrupted,
    /// 用户取消了当前尝试。
    Cancelled,
}

/// 可追溯的一次阶段执行尝试。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageAttempt {
    /// 尝试的稳定 ID。
    pub id: String,
    /// 所属文章包版本。
    pub package_version: u32,
    /// 被执行的阶段。
    pub stage: WorkflowStage,
    /// 当前或最终状态。
    pub status: StageAttemptStatus,
    /// 尝试开始时间。
    pub started_at_ms: i64,
    /// 尝试结束时间；仍运行时为空。
    pub finished_at_ms: Option<i64>,
    /// 最终失败；成功或运行中为空。
    pub failure: Option<Failure>,
    /// Provider 实际返回的 token 用量；未返回时为空，本地格式转换固定为零。
    #[serde(default)]
    pub token_usage: Option<TokenUsage>,
    /// 成功产物的完整血缘；未成功时为空。
    pub lineage: Option<ArtifactLineage>,
}

/// 一个阶段的当前进度和尝试元数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StageProgress {
    /// 当前阶段状态。
    pub status: StageStatus,
    /// 当前文章包版本内已实际开始的尝试次数。
    pub attempts: u32,
    /// 成功产物所属的文章包版本。
    pub artifact_version: Option<u32>,
    /// 最近一次失败；非失败状态应为空。
    pub failure: Option<Failure>,
    /// 当前运行尝试 ID；非运行状态为空。
    #[serde(default)]
    pub active_attempt_id: Option<String>,
}

/// 一行文章任务的四个固定阶段状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StageStates {
    /// 正文生成阶段。
    pub article: StageProgress,
    /// 简介生成阶段。
    pub summary: StageProgress,
    /// 配图生成阶段。
    pub image: StageProgress,
    /// HTML 格式转换阶段。
    pub format: StageProgress,
}

impl StageStates {
    /// 按阶段取得不可变进度。
    pub fn get(&self, stage: WorkflowStage) -> &StageProgress {
        match stage {
            WorkflowStage::Article => &self.article,
            WorkflowStage::Summary => &self.summary,
            WorkflowStage::Image => &self.image,
            WorkflowStage::Format => &self.format,
        }
    }

    /// 按阶段取得可变进度。
    pub fn get_mut(&mut self, stage: WorkflowStage) -> &mut StageProgress {
        match stage {
            WorkflowStage::Article => &mut self.article,
            WorkflowStage::Summary => &mut self.summary,
            WorkflowStage::Image => &mut self.image,
            WorkflowStage::Format => &mut self.format,
        }
    }
}

/// 人工审核状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ReviewStatus {
    /// 产物尚未满足审核条件，或已退回修改。
    #[default]
    Draft,
    /// 所有必需产物完整，等待人工审核。
    Pending,
    /// 当前文章包版本已经批准。
    Approved,
}

/// 由阶段与审核状态推导的行级总体状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverallStatus {
    /// 尚未运行或仍在编辑。
    Draft,
    /// 至少一个阶段已排队且没有阶段运行。
    Queued,
    /// 至少一个阶段正在运行。
    Running,
    /// 全部必需阶段成功，等待人工审核。
    PendingReview,
    /// 当前文章包版本已经批准。
    Approved,
    /// 至少一个阶段失败，同时保留了其他阶段结果。
    PartialFailure,
    /// 用户取消了未完成工作。
    Cancelled,
    /// 应用退出或 worker 所有权丢失后等待恢复。
    Interrupted,
}

/// 产物与其直接输入和生成配置的血缘关系。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLineage {
    /// 产生该产物的文章包版本。
    pub package_version: u32,
    /// 直接依赖的正文版本；正文自身可为空。
    pub source_article_version: Option<u32>,
    /// 直接依赖的简介版本；只有配图等读取简介的产物需要记录。
    #[serde(default)]
    pub source_summary_version: Option<u32>,
    /// 使用的 Prompt 模板稳定 ID。
    pub template_id: Option<String>,
    /// 使用的 Prompt 模板版本。
    pub template_version: Option<u32>,
    /// 使用的 Provider 配置稳定 ID；本地格式转换可为空。
    pub provider_id: Option<String>,
    /// 使用的 Provider 不可变配置修订号；本地格式转换可为空。
    #[serde(default)]
    pub provider_revision: Option<u32>,
    /// 使用的模型名称；本地格式转换可为空。
    pub model: Option<String>,
    /// 本次尝试的稳定 ID。
    pub attempt_id: String,
}

/// 当前文章包内的可展示产物。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArticleArtifacts {
    /// Markdown 规范正文。
    pub markdown: Option<String>,
    /// 文章简介。
    pub summary: Option<String>,
    /// 本地图片引用或经过验证的远端图片引用。
    pub image_ref: Option<String>,
    /// 永不被人工编辑覆盖的 AI 原图引用；旧数据由适配器安全补齐。
    #[serde(default)]
    pub original_image_ref: Option<String>,
    /// 从规范正文派生的 HTML。
    pub html: Option<String>,
    /// 正文产物血缘。
    pub article_lineage: Option<ArtifactLineage>,
    /// 简介产物血缘。
    pub summary_lineage: Option<ArtifactLineage>,
    /// 配图产物血缘。
    pub image_lineage: Option<ArtifactLineage>,
    /// 当前人工配图编辑的独立血缘，不会伪装成 AI 阶段尝试。
    #[serde(default)]
    pub image_edit_lineage: Option<crate::ImageEditLineage>,
    /// 当前人工配图编辑修订号，供只读编辑器与保存乐观锁使用。
    #[serde(default)]
    pub image_edit_revision: u64,
    /// HTML 产物血缘。
    pub format_lineage: Option<ArtifactLineage>,
}

impl ArticleArtifacts {
    /// 检查四项必需产物及其血缘是否完整。
    pub fn is_complete(&self) -> bool {
        self.markdown
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
            && self
                .summary
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            && self
                .image_ref
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            && self
                .html
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            && self.article_lineage.is_some()
            && self.summary_lineage.is_some()
            && self.image_lineage.is_some()
            && self.format_lineage.is_some()
    }
}

/// 一份不可变的已批准文章包快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedPackage {
    /// 被批准的文章包版本。
    pub version: u32,
    /// 批准时的标题。
    #[serde(default)]
    pub title: String,
    /// 批准时的关键词。
    #[serde(default)]
    pub keywords: Vec<String>,
    /// 批准时的内容要求。
    #[serde(default)]
    pub requirements: String,
    /// 获批时的完整产物快照。
    pub artifacts: ArticleArtifacts,
    /// 审核完成时间，使用 Unix 毫秒。
    pub approved_at_ms: i64,
}

/// 创建新版本前冻结的完整文章包快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticlePackageSnapshot {
    /// 被替换的文章包版本。
    pub version: u32,
    /// 当时的标题。
    pub title: String,
    /// 当时的关键词。
    pub keywords: Vec<String>,
    /// 当时的内容要求。
    pub requirements: String,
    /// 当时的审核状态。
    pub review_status: ReviewStatus,
    /// 当时已经形成的全部产物。
    pub artifacts: ArticleArtifacts,
    /// 冻结快照的时间。
    pub captured_at_ms: i64,
}

/// Grid 中一行文章任务的权威领域状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleTask {
    /// 本地稳定任务 ID。
    pub id: String,
    /// 用户输入的文章标题。
    pub title: String,
    /// 可选关键词，按录入顺序保留。
    pub keywords: Vec<String>,
    /// 可选的行级内容要求。
    pub requirements: String,
    /// 当前文章包版本，从 1 开始。
    pub package_version: u32,
    /// 供持久层乐观锁使用的行版本。
    pub row_version: u64,
    /// 四个固定 SOP 阶段状态。
    pub stages: StageStates,
    /// 当前文章包的审核状态。
    pub review_status: ReviewStatus,
    /// 当前文章包产物。
    pub artifacts: ArticleArtifacts,
    /// 历史已批准版本的不可变快照。
    pub approved_history: Vec<ApprovedPackage>,
    /// 所有被新版本替换过的文章包快照。
    #[serde(default)]
    pub package_history: Vec<ArticlePackageSnapshot>,
    /// 每次实际阶段执行的持久尝试记录。
    #[serde(default)]
    pub attempt_history: Vec<StageAttempt>,
    /// 文章首次创建时间；创建后不随任何领域修改变化。
    #[serde(default)]
    pub created_at_ms: i64,
    /// 最近一次领域变化时间，使用 Unix 毫秒。
    pub updated_at_ms: i64,
}

impl ArticleTask {
    /// 创建一条保持原始标题、尚未排队的文章任务。
    pub fn new(id: impl Into<String>, title: impl Into<String>, updated_at_ms: i64) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            keywords: Vec::new(),
            requirements: String::new(),
            package_version: 1,
            row_version: 1,
            stages: StageStates::default(),
            review_status: ReviewStatus::Draft,
            artifacts: ArticleArtifacts::default(),
            approved_history: Vec::new(),
            package_history: Vec::new(),
            attempt_history: Vec::new(),
            created_at_ms: updated_at_ms,
            updated_at_ms,
        }
    }

    /// 为缺少创建时间的旧文章选择最早可证明的持久时间证据。
    ///
    /// 已存在的正数创建时间保持不变。迁移优先使用阶段开始、包历史、批准历史
    /// 与人工配图编辑时间；只有完全没有这些证据时才回退到最近更新时间。
    pub fn backfill_created_at_ms(&mut self) -> bool {
        if self.created_at_ms > 0 {
            return false;
        }
        let image_edit_times = std::iter::once(&self.artifacts)
            .chain(
                self.package_history
                    .iter()
                    .map(|snapshot| &snapshot.artifacts),
            )
            .chain(
                self.approved_history
                    .iter()
                    .map(|approved| &approved.artifacts),
            )
            .filter_map(|artifacts| {
                artifacts
                    .image_edit_lineage
                    .as_ref()
                    .map(|lineage| lineage.edited_at_ms)
            });
        let inferred = self
            .attempt_history
            .iter()
            .map(|attempt| attempt.started_at_ms)
            .chain(
                self.package_history
                    .iter()
                    .map(|snapshot| snapshot.captured_at_ms),
            )
            .chain(
                self.approved_history
                    .iter()
                    .map(|approved| approved.approved_at_ms),
            )
            .chain(image_edit_times)
            .filter(|timestamp| *timestamp > 0)
            .min()
            .unwrap_or(self.updated_at_ms);
        if inferred == self.created_at_ms {
            return false;
        }
        self.created_at_ms = inferred;
        true
    }

    /// 从阶段与审核状态推导总体状态，避免适配器维护第二份真值。
    pub fn overall_status(&self) -> OverallStatus {
        let statuses = WorkflowStage::ordered().map(|stage| self.stages.get(stage).status);
        if self.review_status == ReviewStatus::Approved {
            return OverallStatus::Approved;
        }
        if statuses.contains(&StageStatus::Running) {
            return OverallStatus::Running;
        }
        if statuses.contains(&StageStatus::Interrupted) {
            return OverallStatus::Interrupted;
        }
        if statuses.contains(&StageStatus::Failed) || statuses.contains(&StageStatus::Stale) {
            return OverallStatus::PartialFailure;
        }
        if statuses.contains(&StageStatus::Queued) {
            return OverallStatus::Queued;
        }
        if self.review_status == ReviewStatus::Pending {
            return OverallStatus::PendingReview;
        }
        if statuses.contains(&StageStatus::Cancelled) {
            return OverallStatus::Cancelled;
        }
        OverallStatus::Draft
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 旧数据使用最早历史证据回填创建时间，而不是直接采用最后更新时间。
    #[test]
    fn backfills_created_time_from_earliest_persisted_evidence() {
        let mut task = ArticleTask::new("legacy", "旧文章", 900);
        task.created_at_ms = 0;
        task.package_history.push(ArticlePackageSnapshot {
            version: 1,
            title: "旧文章".into(),
            keywords: Vec::new(),
            requirements: String::new(),
            review_status: ReviewStatus::Draft,
            artifacts: ArticleArtifacts::default(),
            captured_at_ms: 300,
        });
        task.approved_history.push(ApprovedPackage {
            version: 1,
            title: "旧文章".into(),
            keywords: Vec::new(),
            requirements: String::new(),
            artifacts: ArticleArtifacts::default(),
            approved_at_ms: 250,
        });
        task.attempt_history.push(StageAttempt {
            id: "attempt-oldest".into(),
            package_version: 1,
            stage: WorkflowStage::Article,
            status: StageAttemptStatus::Succeeded,
            started_at_ms: 200,
            finished_at_ms: Some(225),
            failure: None,
            token_usage: None,
            lineage: None,
        });

        assert!(task.backfill_created_at_ms());
        assert_eq!(task.created_at_ms, 200);
        assert_eq!(task.updated_at_ms, 900);
        assert!(!task.backfill_created_at_ms());
    }

    /// 没有任何历史时间证据的旧文章才回退到更新时间。
    #[test]
    fn backfills_created_time_from_updated_time_only_as_fallback() {
        let mut task = ArticleTask::new("legacy-empty", "旧文章", 700);
        task.created_at_ms = 0;

        assert!(task.backfill_created_at_ms());
        assert_eq!(task.created_at_ms, 700);
    }
}
