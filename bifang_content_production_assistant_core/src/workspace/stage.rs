//! 单阶段运行在工作区中的预检、冻结绑定和排队事务。

use super::{
    PackageExecutionBinding, StageRequestError, StageStatus, WorkflowStage, WorkspaceData,
    find_execution_binding, record_queued_stage_run, single_stage_execution_binding,
    upsert_execution_binding, validate_single_stage_request,
};
use crate::WorkflowError;

/// 单阶段排队的领域拒绝原因；宿主负责映射到稳定 IPC 错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageQueueError {
    /// Grid 所见快照、付费确认或 Provider 修订已失效。
    Request(StageRequestError),
    /// 文章状态机拒绝目标阶段。
    Workflow(WorkflowError),
}

/// 先核对点击时快照与状态机依赖，再允许宿主读取任何执行凭据。
#[allow(clippy::too_many_arguments)]
pub fn preflight_single_stage_request(
    workspace: &WorkspaceData,
    task_id: &str,
    stage: WorkflowStage,
    expected_row_version: u64,
    expected_package_version: u32,
    confirmed: bool,
    expected_provider_id: Option<&str>,
    expected_provider_revision: Option<u32>,
    now_ms: i64,
) -> Result<(), StageQueueError> {
    validate_single_stage_request(
        workspace,
        task_id,
        stage,
        expected_row_version,
        expected_package_version,
        confirmed,
        expected_provider_id,
        expected_provider_revision,
    )
    .map_err(StageQueueError::Request)?;
    let mut preview = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .cloned()
        .ok_or(StageQueueError::Request(StageRequestError::ArticleNotFound))?;
    preview
        .queue_single_stage(stage, now_ms)
        .map_err(StageQueueError::Workflow)
}

/// 非正文或正文首次执行沿用同包旧 renderer；正文重新生成新包使用新版。
pub fn inherited_stage_prompt_renderer_version(
    workspace: &WorkspaceData,
    task_id: &str,
    stage: WorkflowStage,
    package_version: u32,
) -> Option<u32> {
    let article = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)?;
    if stage == WorkflowStage::Article
        && article.stages.get(stage).status != StageStatus::NotStarted
    {
        return None;
    }
    find_execution_binding(workspace, task_id, package_version)
        .map(|binding| binding.prompt_renderer_version)
}

/// 在同一工作区更新中重验、排队、合并冻结绑定并登记单阶段历史。
#[allow(clippy::too_many_arguments)]
pub fn queue_single_stage_in_workspace(
    workspace: &mut WorkspaceData,
    task_id: &str,
    stage: WorkflowStage,
    expected_row_version: u64,
    expected_package_version: u32,
    confirmed: bool,
    expected_provider_id: Option<&str>,
    expected_provider_revision: Option<u32>,
    proposed_binding: &PackageExecutionBinding,
    batch_id: String,
    now_ms: i64,
    fixture_enabled: bool,
) -> Result<(), StageQueueError> {
    preflight_single_stage_request(
        workspace,
        task_id,
        stage,
        expected_row_version,
        expected_package_version,
        confirmed,
        expected_provider_id,
        expected_provider_revision,
        now_ms,
    )?;
    let target_was_not_started = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .is_some_and(|article| article.stages.get(stage).status == StageStatus::NotStarted);
    let inherited_binding =
        find_execution_binding(workspace, task_id, expected_package_version).cloned();
    let article = workspace
        .articles
        .iter_mut()
        .find(|article| article.id == task_id)
        .ok_or(StageQueueError::Request(StageRequestError::ArticleNotFound))?;
    article
        .queue_single_stage(stage, now_ms)
        .map_err(StageQueueError::Workflow)?;
    let package_version = article.package_version;
    let mut binding = single_stage_execution_binding(
        inherited_binding,
        proposed_binding,
        stage,
        target_was_not_started,
    );
    binding.task_id = task_id.to_string();
    binding.package_version = package_version;
    upsert_execution_binding(workspace, binding);
    record_queued_stage_run(
        workspace,
        batch_id,
        task_id.to_string(),
        stage,
        now_ms,
        fixture_enabled,
    );
    Ok(())
}
