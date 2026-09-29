use super::*;

/// 一次批量运行的持久状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    /// 已创建但尚未开始执行。
    Queued,
    /// 正在执行至少一行。
    Running,
    /// 暂停领取后续步骤。
    Paused,
    /// 全部行完成且没有失败。
    Succeeded,
    /// 至少一行失败，但其他行保留成功产物。
    PartialFailure,
    /// 用户取消了未完成工作。
    Cancelled,
}

/// 仅由全部行的真实终态推导批次终态，运行期与重启恢复必须共用。
pub fn terminal_run_status(statuses: &[OverallStatus]) -> Result<RunStatus, String> {
    if statuses.is_empty()
        || statuses.iter().any(|status| {
            !matches!(
                status,
                OverallStatus::PendingReview
                    | OverallStatus::Approved
                    | OverallStatus::PartialFailure
                    | OverallStatus::Interrupted
                    | OverallStatus::Cancelled
            )
        })
    {
        return Err("批次结束时仍包含未收口任务".into());
    }
    if statuses.iter().all(|status| {
        matches!(
            status,
            OverallStatus::PendingReview | OverallStatus::Approved
        )
    }) {
        return Ok(RunStatus::Succeeded);
    }
    if statuses.iter().any(|status| {
        matches!(
            status,
            OverallStatus::PartialFailure | OverallStatus::Interrupted
        )
    }) {
        return Ok(RunStatus::PartialFailure);
    }
    Ok(RunStatus::Cancelled)
}

/// 由显式单步运行目标阶段的真实终态推导运行记录终态。
pub fn terminal_stage_run_status(status: StageStatus) -> Result<RunStatus, String> {
    match status {
        StageStatus::Succeeded => Ok(RunStatus::Succeeded),
        StageStatus::Failed | StageStatus::Interrupted | StageStatus::Stale => {
            Ok(RunStatus::PartialFailure)
        }
        StageStatus::Cancelled => Ok(RunStatus::Cancelled),
        StageStatus::NotStarted | StageStatus::Queued | StageStatus::Running => {
            Err("单步运行结束时目标阶段仍未收口".into())
        }
    }
}

/// 可查询和恢复的一次批量运行记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    /// 本地稳定批次 ID。
    pub id: String,
    /// 批次冻结的任务 ID。
    pub task_ids: Vec<String>,
    /// 单步运行的唯一目标阶段；普通完整批次为空。
    #[serde(default)]
    pub target_stage: Option<WorkflowStage>,
    /// 批次状态。
    pub status: RunStatus,
    /// 批次开始时间，使用 Unix 毫秒。
    pub started_at_ms: i64,
    /// 批次结束时间，未结束时为空。
    pub finished_at_ms: Option<i64>,
    /// 已进入待审核或批准的行数。
    pub completed_count: usize,
    /// 当前存在失败或中断的行数。
    pub failed_count: usize,
    /// 本批次是否使用了显式调试夹具，供所有后续界面持续标注。
    #[serde(default)]
    pub is_test_fixture: bool,
}

/// 返回指定阶段当前默认 Provider；Format 不使用 Provider。
pub fn default_provider_for_stage(
    workspace: &WorkspaceData,
    stage: WorkflowStage,
) -> Option<&ProviderConfig> {
    workspace
        .provider_configs
        .iter()
        .find(|provider| match stage {
            WorkflowStage::Article => provider.default_for_article,
            WorkflowStage::Summary => provider.default_for_summary,
            WorkflowStage::Image => provider.default_for_image,
            WorkflowStage::Format => false,
        })
}

/// 只有显式夹具模式且三个 AI 阶段默认端均为夹具时才标记测试运行。
pub fn run_uses_fixture_for_mode(workspace: &WorkspaceData, fixture_enabled: bool) -> bool {
    fixture_enabled
        && workspace.provider_configs.iter().any(|provider| {
            provider.enabled
                && provider.kind == ProviderKind::Fixture
                && provider.default_for_article
        })
        && workspace.provider_configs.iter().any(|provider| {
            provider.enabled
                && provider.kind == ProviderKind::Fixture
                && provider.default_for_summary
        })
        && workspace.provider_configs.iter().any(|provider| {
            provider.enabled && provider.kind == ProviderKind::Fixture && provider.default_for_image
        })
}

/// 统一写入有界批次历史，时间与夹具模式由宿主显式提供。
pub fn record_queued_run(
    workspace: &mut WorkspaceData,
    batch_id: String,
    task_ids: Vec<String>,
    now_ms: i64,
    fixture_enabled: bool,
) {
    let is_test_fixture = run_uses_fixture_for_mode(workspace, fixture_enabled);
    workspace.run_history.insert(
        0,
        RunRecord {
            id: batch_id,
            task_ids,
            target_stage: None,
            status: RunStatus::Queued,
            started_at_ms: now_ms,
            finished_at_ms: None,
            completed_count: 0,
            failed_count: 0,
            is_test_fixture,
        },
    );
    workspace.run_history.truncate(100);
}

/// 创建只拥有目标阶段的有界运行记录。
pub fn record_queued_stage_run(
    workspace: &mut WorkspaceData,
    batch_id: String,
    task_id: String,
    stage: WorkflowStage,
    now_ms: i64,
    fixture_enabled: bool,
) {
    let is_test_fixture = fixture_enabled
        && default_provider_for_stage(workspace, stage)
            .is_some_and(|provider| provider.kind == ProviderKind::Fixture);
    workspace.run_history.insert(
        0,
        RunRecord {
            id: batch_id,
            task_ids: vec![task_id],
            target_stage: Some(stage),
            status: RunStatus::Queued,
            started_at_ms: now_ms,
            finished_at_ms: None,
            completed_count: 0,
            failed_count: 0,
            is_test_fixture,
        },
    );
    workspace.run_history.truncate(100);
}

/// 在原子控制标志和工作区状态共同检查后取得的下一阶段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageClaim {
    /// 已将阶段原子转为运行中。
    Started {
        /// 被领取的工作流阶段。
        stage: WorkflowStage,
        /// 新建的阶段尝试稳定 ID。
        attempt_id: String,
    },
    /// 暂停时不领取新阶段。
    Paused,
    /// 取消时不领取新阶段。
    Cancelled,
    /// 已无可运行阶段。
    Finished,
}

/// 控制标志复检与阶段领取必须在同一次仓储更新中调用。
pub fn claim_next_stage_in_workspace(
    workspace: &mut WorkspaceData,
    task_id: &str,
    paused: bool,
    cancelled: bool,
    target_stage: Option<WorkflowStage>,
    now_ms: i64,
) -> Result<StageClaim, String> {
    if cancelled {
        return Ok(StageClaim::Cancelled);
    }
    if paused {
        return Ok(StageClaim::Paused);
    }
    let article = super::find_article_mut(workspace, task_id)?;
    let stage = if let Some(stage) = target_stage {
        (article.stages.get(stage).status == StageStatus::Queued).then_some(stage)
    } else {
        article.next_runnable_stage()
    };
    let Some(stage) = stage else {
        return Ok(StageClaim::Finished);
    };
    let attempt_id = article
        .start_stage(stage, now_ms)
        .map_err(|error| error.to_string())?;
    Ok(StageClaim::Started { stage, attempt_id })
}

/// 内部错误后收口指定文章的未完成阶段。
pub fn recover_interrupted_run_articles(
    workspace: &mut WorkspaceData,
    task_ids: &[String],
    now_ms: i64,
    target_stage: Option<WorkflowStage>,
    target_running_is_retryable: bool,
) {
    for task_id in task_ids {
        if let Some(article) = workspace
            .articles
            .iter_mut()
            .find(|article| article.id == *task_id)
        {
            if let Some(stage) = target_stage {
                article.recover_interrupted_stage_with_policy(
                    stage,
                    now_ms,
                    target_running_is_retryable,
                );
            } else {
                article.recover_interrupted(now_ms);
            }
        }
    }
}

/// 取消指定文章当前仍未完成的全批次或单步阶段。
pub fn cancel_task_in_workspace(
    workspace: &mut WorkspaceData,
    task_id: &str,
    target_stage: Option<WorkflowStage>,
    now_ms: i64,
) -> Result<(), String> {
    let article = super::find_article_mut(workspace, task_id)?;
    if let Some(stage) = target_stage {
        article.cancel_pending_stage(stage, now_ms);
    } else {
        article.cancel_pending(now_ms);
    }
    Ok(())
}

/// 按目标阶段和批次行的真实终态推导运行记录终态。
pub fn derive_terminal_run_status(
    workspace: &WorkspaceData,
    task_ids: &[String],
    target_stage: Option<WorkflowStage>,
) -> Result<RunStatus, String> {
    if let Some(stage) = target_stage {
        if task_ids.len() != 1 {
            return Err("单步运行必须且只能包含一篇文章".into());
        }
        let status = workspace
            .articles
            .iter()
            .find(|article| article.id == task_ids[0])
            .map(|article| article.stages.get(stage).status)
            .ok_or_else(|| "单步运行中的文章已经不存在".to_string())?;
        return terminal_stage_run_status(status);
    }
    let statuses = task_ids
        .iter()
        .map(|task_id| {
            workspace
                .articles
                .iter()
                .find(|article| article.id == *task_id)
                .map(ArticleTask::overall_status)
                .ok_or_else(|| "批次中的文章已经不存在".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    terminal_run_status(&statuses)
}

/// 将新批次从排队转为运行；快速暂停时保留暂停状态。
pub fn mark_run_started_in_workspace(
    workspace: &mut WorkspaceData,
    batch_id: &str,
) -> Result<(), String> {
    let run = workspace
        .run_history
        .iter_mut()
        .find(|run| run.id == batch_id)
        .ok_or_else(|| "找不到指定批次".to_string())?;
    match run.status {
        RunStatus::Queued => {
            run.status = RunStatus::Running;
            Ok(())
        }
        RunStatus::Paused => Ok(()),
        _ => Err("批次已不在可启动状态".to_string()),
    }
}

/// 只允许从指定非终态进入暂停或恢复，避免覆盖已完成批次。
pub fn transition_controlled_run_status_in_workspace(
    workspace: &mut WorkspaceData,
    batch_id: &str,
    target: RunStatus,
    allowed_from: &[RunStatus],
) -> Result<(), String> {
    let run = workspace
        .run_history
        .iter_mut()
        .find(|run| run.id == batch_id)
        .ok_or_else(|| "找不到指定批次".to_string())?;
    if run.status == target {
        return Ok(());
    }
    if !allowed_from.contains(&run.status) {
        return Err("批次已不在可控制状态".to_string());
    }
    run.status = target;
    Ok(())
}

/// 重新统计批次已完成与失败行数并写入终态。
pub fn refresh_run_counts_in_workspace(
    workspace: &mut WorkspaceData,
    batch_id: &str,
    status: RunStatus,
    finished_at_ms: Option<i64>,
) -> Result<(), String> {
    let (task_ids, target_stage) = workspace
        .run_history
        .iter()
        .find(|run| run.id == batch_id)
        .map(|run| (run.task_ids.clone(), run.target_stage))
        .ok_or_else(|| "找不到指定批次".to_string())?;
    let completed_count = workspace
        .articles
        .iter()
        .filter(|article| {
            task_ids.contains(&article.id)
                && target_stage.map_or_else(
                    || {
                        matches!(
                            article.overall_status(),
                            OverallStatus::PendingReview | OverallStatus::Approved
                        )
                    },
                    |stage| article.stages.get(stage).status == StageStatus::Succeeded,
                )
        })
        .count();
    let failed_count = workspace
        .articles
        .iter()
        .filter(|article| {
            task_ids.contains(&article.id)
                && target_stage.map_or_else(
                    || {
                        matches!(
                            article.overall_status(),
                            OverallStatus::PartialFailure | OverallStatus::Interrupted
                        )
                    },
                    |stage| {
                        matches!(
                            article.stages.get(stage).status,
                            StageStatus::Failed | StageStatus::Interrupted | StageStatus::Stale
                        )
                    },
                )
        })
        .count();
    let run = workspace
        .run_history
        .iter_mut()
        .find(|run| run.id == batch_id)
        .ok_or_else(|| "找不到指定批次".to_string())?;
    let is_progress_refresh = finished_at_ms.is_none()
        && matches!(status, RunStatus::Running | RunStatus::Paused)
        && matches!(run.status, RunStatus::Running | RunStatus::Paused);
    if !is_progress_refresh {
        run.status = status;
    }
    run.finished_at_ms = finished_at_ms;
    run.completed_count = completed_count;
    run.failed_count = failed_count;
    Ok(())
}
