use super::*;

/// 顺序执行选中行；并发上限为一，保证首版资源使用稳定。
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_batch(
    repository: Arc<WorkspaceRepository>,
    batch_id: String,
    task_ids: Vec<String>,
    executions: BTreeMap<String, ExecutionSnapshot>,
    provider_client: OpenAiCompatibleClient,
    asset_dir: PathBuf,
    provider_throttle: Arc<tokio::sync::Mutex<BTreeMap<String, Instant>>>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    target_stage: Option<WorkflowStage>,
) {
    let target_running_is_retryable = target_stage != Some(WorkflowStage::Image)
        || executions.values().all(|execution| {
            execution.image_request_contract != ImageRequestContract::VolcengineArkUrlV1
        });
    let result = run_batch_inner(
        &repository,
        &batch_id,
        &task_ids,
        &executions,
        &provider_client,
        &asset_dir,
        &provider_throttle,
        &paused,
        &cancelled,
        target_stage,
    )
    .await;
    if result.is_err() {
        let _ = recover_batch_after_internal_error(
            &repository,
            &batch_id,
            &task_ids,
            current_time_ms(),
            target_stage,
            target_running_is_retryable,
        )
        .await;
    }
}

/// 内部错误后先收口未完成阶段，再按真实行终态恢复批次而非硬编码失败。
pub(super) async fn recover_batch_after_internal_error(
    repository: &WorkspaceRepository,
    batch_id: &str,
    task_ids: &[String],
    now_ms: i64,
    target_stage: Option<WorkflowStage>,
    target_running_is_retryable: bool,
) -> Result<(), String> {
    let recovery = repository
        .update(|workspace| {
            recover_interrupted_run_articles(
                workspace,
                task_ids,
                now_ms,
                target_stage,
                target_running_is_retryable,
            );
            Ok(())
        })
        .await;
    let terminal = if recovery.is_ok() {
        derive_terminal_run_status(repository, task_ids, target_stage)
            .await
            .unwrap_or(RunStatus::PartialFailure)
    } else {
        RunStatus::PartialFailure
    };
    refresh_run_counts(repository, batch_id, terminal, Some(now_ms)).await
}

/// 执行一个批次并把任何内部状态错误向拥有者传播。
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_batch_inner(
    repository: &WorkspaceRepository,
    batch_id: &str,
    task_ids: &[String],
    executions: &BTreeMap<String, ExecutionSnapshot>,
    provider_client: &OpenAiCompatibleClient,
    asset_dir: &Path,
    provider_throttle: &tokio::sync::Mutex<BTreeMap<String, Instant>>,
    paused: &AtomicBool,
    cancelled: &AtomicBool,
    target_stage: Option<WorkflowStage>,
) -> Result<(), String> {
    mark_run_started(repository, batch_id).await?;
    for task_id in task_ids {
        let execution = executions
            .get(task_id)
            .ok_or_else(|| "批次缺少文章包执行绑定".to_string())?;
        if cancelled.load(Ordering::Acquire) {
            cancel_task(repository, task_id, target_stage).await?;
            continue;
        }
        run_task(
            repository,
            task_id,
            execution,
            provider_client,
            asset_dir,
            provider_throttle,
            paused,
            cancelled,
            target_stage,
        )
        .await?;
        let current_status = if paused.load(Ordering::Acquire) {
            RunStatus::Paused
        } else {
            RunStatus::Running
        };
        refresh_run_counts(repository, batch_id, current_status, None).await?;
    }
    let terminal = derive_terminal_run_status(repository, task_ids, target_stage).await?;
    refresh_run_counts(repository, batch_id, terminal, Some(current_time_ms())).await
}

/// 执行一行任务直到待审核、失败或取消。
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_task(
    repository: &WorkspaceRepository,
    task_id: &str,
    execution: &ExecutionSnapshot,
    provider_client: &OpenAiCompatibleClient,
    asset_dir: &Path,
    provider_throttle: &tokio::sync::Mutex<BTreeMap<String, Instant>>,
    paused: &AtomicBool,
    cancelled: &AtomicBool,
    target_stage: Option<WorkflowStage>,
) -> Result<(), String> {
    loop {
        while paused.load(Ordering::Acquire) && !cancelled.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let (stage, attempt_id) =
            match claim_next_stage(repository, task_id, paused, cancelled, target_stage).await? {
                StageClaim::Started { stage, attempt_id } => (stage, attempt_id),
                StageClaim::Paused => continue,
                StageClaim::Cancelled => {
                    return cancel_task(repository, task_id, target_stage).await;
                }
                StageClaim::Finished => return Ok(()),
            };
        let running_task = repository
            .snapshot()
            .await
            .articles
            .into_iter()
            .find(|article| article.id == task_id)
            .ok_or_else(|| "找不到运行中文章".to_string())?;
        let outcome = execute_stage(
            &running_task,
            stage,
            execution,
            provider_client,
            asset_dir,
            provider_throttle,
            attempt_id,
            paused,
            cancelled,
        )
        .await;
        match outcome {
            Ok((output, lineage, token_usage)) => {
                repository
                    .update(|workspace| {
                        let article = find_article_mut(workspace, task_id)?;
                        if target_stage.is_some() {
                            article
                                .complete_stage_only_with_usage(
                                    stage,
                                    output,
                                    lineage,
                                    token_usage,
                                    current_time_ms(),
                                )
                                .map_err(|error| error.to_string())
                        } else {
                            article
                                .complete_stage_with_usage(
                                    stage,
                                    output,
                                    lineage,
                                    token_usage,
                                    current_time_ms(),
                                )
                                .map_err(|error| error.to_string())
                        }
                    })
                    .await?;
                if target_stage.is_some() {
                    return Ok(());
                }
            }
            Err(error) if error.error.kind == ProviderErrorKind::Cancelled => {
                return cancel_task(repository, task_id, target_stage).await;
            }
            Err(error) => {
                repository
                    .update(|workspace| {
                        let article = find_article_mut(workspace, task_id)?;
                        article
                            .fail_stage_with_usage(
                                stage,
                                error.error.as_failure(),
                                error.token_usage,
                                current_time_ms(),
                            )
                            .map_err(|workflow_error| workflow_error.to_string())
                    })
                    .await?;
                return Ok(());
            }
        }
    }
}

/// 将控制标志复检与 start_stage 放进同一持久更新闭包；成功领取即进入 in-flight 边界。
pub(super) async fn claim_next_stage(
    repository: &WorkspaceRepository,
    task_id: &str,
    paused: &AtomicBool,
    cancelled: &AtomicBool,
    target_stage: Option<WorkflowStage>,
) -> Result<StageClaim, String> {
    repository
        .update(|workspace| {
            claim_next_stage_in_workspace(
                workspace,
                task_id,
                paused.load(Ordering::Acquire),
                cancelled.load(Ordering::Acquire),
                target_stage,
                current_time_ms(),
            )
        })
        .await
}

/// 取消一条任务尚未完成的阶段。
pub(super) async fn cancel_task(
    repository: &WorkspaceRepository,
    task_id: &str,
    target_stage: Option<WorkflowStage>,
) -> Result<(), String> {
    repository
        .update(|workspace| {
            cancel_task_in_workspace(workspace, task_id, target_stage, current_time_ms())
        })
        .await
}

/// 按批次中各行最终状态推导批次终态。
pub(super) async fn derive_terminal_run_status(
    repository: &WorkspaceRepository,
    task_ids: &[String],
    target_stage: Option<WorkflowStage>,
) -> Result<RunStatus, String> {
    derive_terminal_run_status_in_workspace(&repository.snapshot().await, task_ids, target_stage)
}

/// 将新批次从排队转为运行；已被快速暂停时保留暂停状态。
pub(super) async fn mark_run_started(
    repository: &WorkspaceRepository,
    batch_id: &str,
) -> Result<(), String> {
    repository
        .update(|workspace| mark_run_started_in_workspace(workspace, batch_id))
        .await
}

/// 只允许从指定非终态进入暂停或恢复，避免覆盖已完成批次。
pub(super) async fn transition_controlled_run_status(
    repository: &WorkspaceRepository,
    batch_id: &str,
    target: RunStatus,
    allowed_from: &[RunStatus],
) -> Result<(), String> {
    repository
        .update(|workspace| {
            transition_controlled_run_status_in_workspace(workspace, batch_id, target, allowed_from)
        })
        .await
}

/// 重新统计批次已完成与失败行数并写入终态。
pub(super) async fn refresh_run_counts(
    repository: &WorkspaceRepository,
    batch_id: &str,
    status: RunStatus,
    finished_at_ms: Option<i64>,
) -> Result<(), String> {
    repository
        .update(|workspace| {
            refresh_run_counts_in_workspace(workspace, batch_id, status, finished_at_ms)
        })
        .await
}

/// 将非 Provider 内部错误转换为稳定失败，供未来扩展使用。
#[allow(dead_code)]
pub(super) fn stable_internal_failure(message: &str) -> Failure {
    Failure {
        code: "WORKBENCH_INTERNAL".into(),
        message: message.into(),
        retryable: false,
        retry_after_ms: None,
    }
}
