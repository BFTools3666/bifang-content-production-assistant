use super::*;

#[test]
fn running_stage_recovers_as_retryable_interruption() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let attempt_id = task.start_stage(WorkflowStage::Article, 2).unwrap();

    assert!(task.recover_interrupted(3));
    assert_eq!(task.overall_status(), OverallStatus::Interrupted);
    assert_eq!(
        task.stages.article.failure.as_ref().unwrap().code,
        "RUN_INTERRUPTED"
    );
    let interrupted_attempt = task.attempt_history.last().unwrap();
    assert_eq!(interrupted_attempt.id, attempt_id);
    assert_eq!(interrupted_attempt.status, StageAttemptStatus::Interrupted);
    assert_eq!(interrupted_attempt.finished_at_ms, Some(3));
    assert_eq!(task.retry_failed(4).unwrap(), WorkflowStage::Article);
}

/// 无幂等保证的运行中外部调用在重启后保持结果未知，必须显式重新生成。
#[test]
fn running_stage_policy_can_block_automatic_retry_after_recovery() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let attempt_id = task.start_stage(WorkflowStage::Article, 2).unwrap();

    assert!(task.recover_interrupted_with_policy(3, |stage| { stage != WorkflowStage::Article }));
    assert_eq!(task.overall_status(), OverallStatus::Interrupted);
    let failure = task.stages.article.failure.as_ref().unwrap();
    assert_eq!(failure.code, "RUN_OUTCOME_UNKNOWN");
    assert!(!failure.retryable);
    let interrupted_attempt = task.attempt_history.last().unwrap();
    assert_eq!(interrupted_attempt.id, attempt_id);
    assert_eq!(interrupted_attempt.status, StageAttemptStatus::Interrupted);
    assert_eq!(interrupted_attempt.failure.as_ref(), Some(failure));
    assert_eq!(task.retry_failed(4), Err(WorkflowError::NothingToRetry));

    task.regenerate_stage(WorkflowStage::Article, 5).unwrap();
    assert_eq!(task.stages.article.status, StageStatus::Queued);
}

/// 过期产物不属于普通失败重试，必须通过显式重新生成进入新版本。
#[test]
fn stale_stage_is_not_eligible_for_failure_retry() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.edit_summary(
        "人工简介",
        ArtifactLineage {
            package_version: task.package_version,
            source_article_version: Some(task.package_version),
            source_summary_version: None,
            template_id: None,
            template_version: None,
            provider_id: None,
            provider_revision: None,
            model: None,
            attempt_id: "manual-summary".into(),
        },
        30,
    )
    .unwrap();

    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert_eq!(task.retry_failed(31), Err(WorkflowError::NothingToRetry));
    assert_eq!(task.stages.image.status, StageStatus::Stale);
}

/// 取消派生重生成会收口队列和过期下游，使行与批次都能得到取消终态。
#[test]
fn cancelling_derived_regeneration_closes_stale_dependants() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.edit_article(
        "# 人工正文",
        ArtifactLineage {
            package_version: task.package_version,
            source_article_version: None,
            source_summary_version: None,
            template_id: None,
            template_version: None,
            provider_id: None,
            provider_revision: None,
            model: None,
            attempt_id: "manual-before-cancel".into(),
        },
        30,
    )
    .unwrap();
    task.regenerate_stage(WorkflowStage::Summary, 31).unwrap();

    assert!(task.cancel_pending(32));
    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, StageStatus::Cancelled);
    assert_eq!(task.stages.image.status, StageStatus::Cancelled);
    assert_eq!(task.stages.format.status, StageStatus::Cancelled);
    assert_eq!(task.overall_status(), OverallStatus::Cancelled);
    assert_eq!(task.retry_failed(33), Err(WorkflowError::NothingToRetry));
}

/// 不允许用空产物伪造成功阶段。
#[test]
fn empty_artifact_cannot_mark_stage_successful() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let article_attempt = task.start_stage(WorkflowStage::Article, 2).unwrap();
    let article_lineage = lineage(&task, WorkflowStage::Article, article_attempt);
    assert_eq!(
        task.complete_stage(
            WorkflowStage::Article,
            ArtifactOutput::Article("  ".into()),
            article_lineage,
            3,
        ),
        Err(WorkflowError::EmptyArtifact)
    );
    assert_eq!(task.stages.article.status, StageStatus::Running);
}
