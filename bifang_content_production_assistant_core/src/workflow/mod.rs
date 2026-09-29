//! 固定内容 SOP 的状态转换、批量资格和审核规则。

use std::{collections::BTreeSet, error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    ApprovedPackage, ArticlePackageSnapshot, ArticleTask, ArtifactLineage, Failure, ReviewStatus,
    StageAttempt, StageAttemptStatus, StageStates, StageStatus, TokenUsage, WorkflowStage,
};

/// 某个阶段成功后要原子保存的类型化产物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactOutput {
    /// Markdown 规范正文。
    Article(String),
    /// 文章简介。
    Summary(String),
    /// 图片文件或经过验证的远端引用。
    Image(String),
    /// 从正文派生的 HTML。
    Format(String),
}

impl ArtifactOutput {
    /// 返回该产物所属的固定阶段。
    pub const fn stage(&self) -> WorkflowStage {
        match self {
            Self::Article(_) => WorkflowStage::Article,
            Self::Summary(_) => WorkflowStage::Summary,
            Self::Image(_) => WorkflowStage::Image,
            Self::Format(_) => WorkflowStage::Format,
        }
    }

    /// 返回产物文本或引用是否为空。
    fn is_empty(&self) -> bool {
        match self {
            Self::Article(value)
            | Self::Summary(value)
            | Self::Image(value)
            | Self::Format(value) => value.trim().is_empty(),
        }
    }
}

/// 状态机拒绝一次操作时返回的稳定领域错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    /// 标题只有空白字符，不能进入队列。
    EmptyTitle,
    /// 当前状态不允许所请求的转换。
    InvalidTransition {
        /// 目标阶段。
        stage: WorkflowStage,
        /// 当前阶段状态。
        status: StageStatus,
    },
    /// 上游成功产物尚不完整。
    DependencyNotReady {
        /// 被阻止的目标阶段。
        stage: WorkflowStage,
    },
    /// 产物类型、包版本、来源版本或尝试与目标阶段不匹配。
    ArtifactStageMismatch,
    /// 成功产物为空，不能写入成功状态。
    EmptyArtifact,
    /// 当前行没有失败或中断阶段可供普通重试。
    NothingToRetry,
    /// 当前文章包不完整或存在过期阶段，不能批准。
    NotReadyForApproval,
    /// 当前审核状态不允许退回修改。
    NotPendingReview,
    /// 行级输入超过了首版允许的稳定范围。
    InvalidInput(String),
}

impl fmt::Display for WorkflowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => formatter.write_str("文章标题不能为空"),
            Self::InvalidTransition { stage, status } => {
                write!(formatter, "阶段 {stage:?} 不能从 {status:?} 进入该状态")
            }
            Self::DependencyNotReady { stage } => {
                write!(formatter, "阶段 {stage:?} 的上游产物尚未就绪")
            }
            Self::ArtifactStageMismatch => formatter.write_str("产物类型或血缘与目标阶段不匹配"),
            Self::EmptyArtifact => formatter.write_str("成功产物不能为空"),
            Self::NothingToRetry => formatter.write_str("当前没有可重试阶段"),
            Self::NotReadyForApproval => formatter.write_str("当前文章包尚不满足批准条件"),
            Self::NotPendingReview => formatter.write_str("只有待审核文章可以退回修改"),
            Self::InvalidInput(message) => formatter.write_str(message),
        }
    }
}

impl Error for WorkflowError {}

/// 一条未进入批次的选中任务及其稳定原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTask {
    /// 被跳过的任务 ID。
    pub task_id: String,
    /// 供界面解释和自动化断言的稳定原因码。
    pub reason: String,
}

/// 一次“运行所选”的纯领域准备结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPreparation {
    /// 实际进入队列的任务 ID，保持 Grid 顺序。
    pub queued_task_ids: Vec<String>,
    /// 明确选中但不满足运行条件的任务。
    pub skipped: Vec<SkippedTask>,
}

mod editing;
mod stage_lifecycle;

/// 仅将用户明确选中且可运行的行加入队列。
pub fn prepare_selected_batch(
    tasks: &mut [ArticleTask],
    selected_ids: impl IntoIterator<Item = String>,
    now_ms: i64,
) -> BatchPreparation {
    let selected = selected_ids.into_iter().collect::<BTreeSet<_>>();
    let mut result = BatchPreparation {
        queued_task_ids: Vec::new(),
        skipped: Vec::new(),
    };

    for task in tasks.iter_mut().filter(|task| selected.contains(&task.id)) {
        match task.queue_initial(now_ms) {
            Ok(()) => result.queued_task_ids.push(task.id.clone()),
            Err(error) => result.skipped.push(SkippedTask {
                task_id: task.id.clone(),
                reason: match error {
                    WorkflowError::EmptyTitle => "EMPTY_TITLE",
                    WorkflowError::InvalidTransition { .. } => "NOT_RUNNABLE",
                    _ => "INVALID_STATE",
                }
                .into(),
            }),
        }
    }
    result
}

/// 只读预检一次“运行所选”，不改变传入任务。
pub fn preview_selected_batch(
    tasks: &[ArticleTask],
    selected_ids: impl IntoIterator<Item = String>,
) -> BatchPreparation {
    let mut candidates = tasks.to_vec();
    prepare_selected_batch(&mut candidates, selected_ids, 0)
}

/// 仅把明确选中且存在失败或中断阶段的文章加入重试批次。
pub fn prepare_retry_batch(
    tasks: &mut [ArticleTask],
    selected_ids: impl IntoIterator<Item = String>,
    now_ms: i64,
) -> BatchPreparation {
    let selected = selected_ids.into_iter().collect::<BTreeSet<_>>();
    let mut result = BatchPreparation {
        queued_task_ids: Vec::new(),
        skipped: Vec::new(),
    };
    for task in tasks.iter_mut().filter(|task| selected.contains(&task.id)) {
        match task.retry_failed(now_ms) {
            Ok(_) => result.queued_task_ids.push(task.id.clone()),
            Err(error) => result.skipped.push(SkippedTask {
                task_id: task.id.clone(),
                reason: match error {
                    WorkflowError::NothingToRetry => "NOTHING_TO_RETRY",
                    WorkflowError::DependencyNotReady { .. } => "DEPENDENCY_NOT_READY",
                    _ => "INVALID_STATE",
                }
                .into(),
            }),
        }
    }
    result
}

#[cfg(test)]
mod tests;
