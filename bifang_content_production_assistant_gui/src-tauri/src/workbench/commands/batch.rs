use super::*;

/// 校验选中行、冻结批次记录并启动受管 worker。
#[tauri::command]
pub async fn start_selected_batch(
    task_ids: Vec<String>,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<BatchStartResult, CommandError> {
    if task_ids.is_empty() {
        return Err(CommandError::new("NO_SELECTION", "请先选择要运行的文章"));
    }
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    let execution = worker.prepare_execution().await?;
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let binding = execution.to_binding(String::new(), 0);
    let preparation = repository
        .update(|workspace| {
            queue_selected_batch_in_workspace(
                workspace,
                task_ids,
                &binding,
                batch_id.clone(),
                current_time_ms(),
                fixture_mode_enabled(),
            )
        })
        .await
        .map_err(store_error)?;
    let queued_ids = preparation.queued_task_ids.clone();
    if !queued_ids.is_empty() {
        let executions = queued_ids
            .iter()
            .map(|task_id| (task_id.clone(), execution.clone()))
            .collect();
        worker
            .start_prepared(batch_id.clone(), queued_ids, executions)
            .map_err(|message| CommandError::new("BATCH_START_FAILED", message))?;
    }
    Ok(BatchStartResult {
        batch_id: (!preparation.queued_task_ids.is_empty()).then_some(batch_id),
        queued_count: preparation.queued_task_ids.len(),
        queued_task_ids: preparation.queued_task_ids,
        skipped_count: preparation.skipped.len(),
        skipped: preparation.skipped,
    })
}

/// 只读预检“运行所选”的可运行与跳过行，不改变任务或发起外部调用。
#[tauri::command]
pub async fn preview_selected_batch(
    task_ids: Vec<String>,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<BatchStartResult, CommandError> {
    if task_ids.is_empty() {
        return Err(CommandError::new("NO_SELECTION", "请先选择要运行的文章"));
    }
    let workspace = repository.snapshot().await;
    worker.validate_execution_configuration(&workspace)?;
    let preparation = preview_batch(&workspace.articles, task_ids);
    Ok(BatchStartResult {
        batch_id: None,
        queued_count: preparation.queued_task_ids.len(),
        queued_task_ids: preparation.queued_task_ids,
        skipped_count: preparation.skipped.len(),
        skipped: preparation.skipped,
    })
}

/// 仅重试一条文章最早的失败或中断阶段。
#[tauri::command]
pub async fn retry_article(
    task_id: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<BatchStartResult, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    let workspace = repository.snapshot().await;
    let package_version = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .map(|article| article.package_version)
        .ok_or_else(|| CommandError::new("RETRY_FAILED", "找不到指定文章"))?;
    let binding =
        find_execution_binding(&workspace, &task_id, package_version).ok_or_else(|| {
            CommandError::new(
                "EXECUTION_BINDING_MISSING",
                "当前文章包缺少冻结的 Prompt/AI 服务绑定，请使用重新生成创建新版本",
            )
        })?;
    ensure_binding_providers_enabled(&workspace, binding).map_err(binding_provider_error)?;
    let execution = worker
        .prepare_bound_execution(binding)
        .map_err(|message| CommandError::new("BATCH_PREPARE_FAILED", message))?;
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let binding = execution.to_binding(String::new(), 0);
    repository
        .update(|workspace| {
            queue_article_retry_in_workspace(
                workspace,
                &task_id,
                &binding,
                batch_id.clone(),
                current_time_ms(),
                fixture_mode_enabled(),
            )
        })
        .await
        .map_err(|message| CommandError::new("RETRY_FAILED", message))?;
    worker
        .start_prepared(
            batch_id.clone(),
            vec![task_id.clone()],
            BTreeMap::from([(task_id.clone(), execution)]),
        )
        .map_err(|message| CommandError::new("BATCH_START_FAILED", message))?;
    Ok(BatchStartResult {
        batch_id: Some(batch_id),
        queued_count: 1,
        queued_task_ids: vec![task_id],
        skipped_count: 0,
        skipped: Vec::new(),
    })
}

/// 将选中行中需要恢复的最早阶段组成一个批次，成功阶段不会重复调用。
#[tauri::command]
pub async fn retry_selected_articles(
    task_ids: Vec<String>,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<BatchStartResult, CommandError> {
    if task_ids.is_empty() {
        return Err(CommandError::new("NO_SELECTION", "请先选择要重试的文章"));
    }
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    let snapshot = repository.snapshot().await;
    let mut preview_articles = snapshot.articles.clone();
    let preview = prepare_retry_batch(&mut preview_articles, task_ids.clone(), current_time_ms());
    let mut executions = BTreeMap::new();
    let mut bindings = BTreeMap::new();
    for task_id in &preview.queued_task_ids {
        let package_version = snapshot
            .articles
            .iter()
            .find(|article| article.id == *task_id)
            .map(|article| article.package_version)
            .ok_or_else(|| CommandError::new("RETRY_FAILED", "找不到指定文章"))?;
        let binding =
            find_execution_binding(&snapshot, task_id, package_version).ok_or_else(|| {
                CommandError::new(
                    "EXECUTION_BINDING_MISSING",
                    "选中的文章包缺少冻结的 Prompt/AI 服务绑定，请使用重新生成创建新版本",
                )
            })?;
        ensure_binding_providers_enabled(&snapshot, binding).map_err(binding_provider_error)?;
        let execution = worker
            .prepare_bound_execution(binding)
            .map_err(|message| CommandError::new("BATCH_PREPARE_FAILED", message))?;
        bindings.insert(task_id.clone(), execution.to_binding(String::new(), 0));
        executions.insert(task_id.clone(), execution);
    }
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let preparation = repository
        .update(|workspace| {
            queue_retry_selected_in_workspace(
                workspace,
                task_ids,
                &preview.queued_task_ids,
                &bindings,
                batch_id.clone(),
                current_time_ms(),
                fixture_mode_enabled(),
            )
        })
        .await
        .map_err(store_error)?;
    if !preparation.queued_task_ids.is_empty() {
        worker
            .start_prepared(
                batch_id.clone(),
                preparation.queued_task_ids.clone(),
                executions,
            )
            .map_err(|message| CommandError::new("BATCH_START_FAILED", message))?;
    }
    Ok(BatchStartResult {
        batch_id: (!preparation.queued_task_ids.is_empty()).then_some(batch_id),
        queued_count: preparation.queued_task_ids.len(),
        queued_task_ids: preparation.queued_task_ids,
        skipped_count: preparation.skipped.len(),
        skipped: preparation.skipped,
    })
}

/// 为一条文章创建新包版本并从正文阶段重新运行。
#[tauri::command]
pub async fn regenerate_article(
    task_id: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<BatchStartResult, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    let execution = worker.prepare_execution().await?;
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let binding = execution.to_binding(String::new(), 0);
    repository
        .update(|workspace| {
            queue_article_regeneration_in_workspace(
                workspace,
                &task_id,
                &binding,
                batch_id.clone(),
                current_time_ms(),
                fixture_mode_enabled(),
            )
        })
        .await
        .map_err(|message| CommandError::new("REGENERATE_FAILED", message))?;
    worker
        .start_prepared(
            batch_id.clone(),
            vec![task_id.clone()],
            BTreeMap::from([(task_id.clone(), execution)]),
        )
        .map_err(|message| CommandError::new("BATCH_START_FAILED", message))?;
    Ok(BatchStartResult {
        batch_id: Some(batch_id),
        queued_count: 1,
        queued_task_ids: vec![task_id],
        skipped_count: 0,
        skipped: Vec::new(),
    })
}

/// 暂停活动批次领取后续步骤。
#[tauri::command]
pub async fn pause_active_batch(worker: State<'_, BatchWorker>) -> Result<String, CommandError> {
    worker
        .pause()
        .await
        .map_err(|message| CommandError::new("PAUSE_FAILED", message))
}

/// 恢复活动批次。
#[tauri::command]
pub async fn resume_active_batch(worker: State<'_, BatchWorker>) -> Result<String, CommandError> {
    worker
        .resume()
        .await
        .map_err(|message| CommandError::new("RESUME_FAILED", message))
}

/// 请求取消活动批次。
#[tauri::command]
pub async fn cancel_active_batch(worker: State<'_, BatchWorker>) -> Result<String, CommandError> {
    worker
        .cancel()
        .await
        .map_err(|message| CommandError::new("CANCEL_FAILED", message))
}

/// 返回最近一百次批量运行记录。
#[tauri::command]
pub async fn list_run_history(
    repository: State<'_, Arc<WorkspaceRepository>>,
) -> Result<Vec<RunRecord>, CommandError> {
    Ok(repository.snapshot().await.run_history)
}

/// 确保启动单行批次前没有活动 worker。
pub(super) fn ensure_worker_available(worker: &BatchWorker) -> Result<(), CommandError> {
    if worker.has_active_batch() {
        Err(CommandError::new(
            "BATCH_ALREADY_RUNNING",
            "已有批次正在运行",
        ))
    } else {
        Ok(())
    }
}

/// 给 Core 的批次历史用例提供宿主时间与显式夹具模式。
#[cfg(test)]
pub(super) fn record_queued_run(
    workspace: &mut WorkspaceData,
    batch_id: String,
    task_ids: Vec<String>,
) {
    bifang_content_production_assistant_core::record_queued_run(
        workspace,
        batch_id,
        task_ids,
        current_time_ms(),
        fixture_mode_enabled(),
    );
}
