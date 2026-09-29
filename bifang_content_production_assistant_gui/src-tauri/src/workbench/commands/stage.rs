use super::*;

/// 从 Grid 或详情只运行一个目标阶段，并以行/包/图片 Provider 修订共同防止陈旧点击。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn generate_article_stage(
    task_id: String,
    stage: WorkflowStage,
    expected_row_version: u64,
    expected_package_version: u32,
    confirmed: bool,
    expected_provider_id: Option<String>,
    expected_provider_revision: Option<u32>,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<BatchStartResult, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    let snapshot = repository.snapshot().await;
    validate_single_stage_request(
        &snapshot,
        &task_id,
        stage,
        expected_row_version,
        expected_package_version,
        confirmed,
        expected_provider_id.as_deref(),
        expected_provider_revision,
    )?;
    let inherited_prompt_renderer_version = inherited_stage_prompt_renderer_version(
        &snapshot,
        &task_id,
        stage,
        expected_package_version,
    );
    let execution = worker
        .prepare_stage_execution(&snapshot, stage, inherited_prompt_renderer_version)
        .map_err(|message| CommandError::new("STAGE_GENERATE_FAILED", message))?;
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let proposed_binding = execution.to_binding(task_id.clone(), expected_package_version);
    repository
        .update(|workspace| {
            queue_single_stage_in_workspace(
                workspace,
                &task_id,
                stage,
                expected_row_version,
                expected_package_version,
                confirmed,
                expected_provider_id.as_deref(),
                expected_provider_revision,
                &proposed_binding,
                batch_id.clone(),
                current_time_ms(),
                fixture_mode_enabled(),
            )
            .map_err(|error| encode_command_error(map_stage_queue_error(error)))
        })
        .await
        .map_err(decode_stage_generate_store_error)?;
    worker
        .start_prepared_stage(batch_id.clone(), task_id.clone(), execution, stage)
        .map_err(|message| CommandError::new("BATCH_START_FAILED", message))?;
    Ok(BatchStartResult {
        batch_id: Some(batch_id),
        queued_count: 1,
        queued_task_ids: vec![task_id],
        skipped_count: 0,
        skipped: Vec::new(),
    })
}

/// 在加载凭据和修改工作区前验证 Grid 所见快照与当前权威状态一致。
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_single_stage_request(
    workspace: &WorkspaceData,
    task_id: &str,
    stage: WorkflowStage,
    expected_row_version: u64,
    expected_package_version: u32,
    confirmed: bool,
    expected_provider_id: Option<&str>,
    expected_provider_revision: Option<u32>,
) -> Result<(), CommandError> {
    preflight_single_stage_request(
        workspace,
        task_id,
        stage,
        expected_row_version,
        expected_package_version,
        confirmed,
        expected_provider_id,
        expected_provider_revision,
        current_time_ms(),
    )
    .map_err(map_stage_queue_error)
}

fn map_stage_queue_error(error: StageQueueError) -> CommandError {
    match error {
        StageQueueError::Request(error) => CommandError::new(error.code(), error.message()),
        StageQueueError::Workflow(error) => map_single_stage_workflow_error(error),
    }
}

/// 将 Core 的乐观锁及付费确认拒绝映射为稳定 IPC 错误。
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_single_stage_request_inner(
    workspace: &WorkspaceData,
    task_id: &str,
    stage: WorkflowStage,
    expected_row_version: u64,
    expected_package_version: u32,
    confirmed: bool,
    expected_provider_id: Option<&str>,
    expected_provider_revision: Option<u32>,
) -> Result<(), CommandError> {
    bifang_content_production_assistant_core::validate_single_stage_request(
        workspace,
        task_id,
        stage,
        expected_row_version,
        expected_package_version,
        confirmed,
        expected_provider_id,
        expected_provider_revision,
    )
    .map_err(|error| CommandError::new(error.code(), error.message()))
}

pub(super) fn map_single_stage_workflow_error(error: WorkflowError) -> CommandError {
    match error {
        WorkflowError::DependencyNotReady { .. } => CommandError::new(
            "DEPENDENCY_NOT_READY",
            "目标阶段的上游产物尚未就绪，请先生成依赖阶段",
        ),
        error => CommandError::new("STAGE_GENERATE_FAILED", error.to_string()),
    }
}
