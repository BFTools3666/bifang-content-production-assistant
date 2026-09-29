use super::*;

/// 人工编辑简介只让依赖它的配图过期，不影响正文和本地格式产物。
#[test]
fn summary_edit_only_stales_image() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    let summary_lineage = ArtifactLineage {
        package_version: task.package_version,
        source_article_version: Some(task.package_version),
        source_summary_version: None,
        template_id: None,
        template_version: None,
        provider_id: None,
        provider_revision: None,
        model: None,
        attempt_id: "manual-summary".into(),
    };

    task.edit_summary("人工简介", summary_lineage, 30).unwrap();

    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, StageStatus::Succeeded);
    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert_eq!(task.stages.format.status, StageStatus::Succeeded);
}

/// 编辑简介会把先前失败的配图一并改为过期，避免用新输入执行旧包普通重试。
#[test]
fn summary_edit_turns_failed_image_into_stale_state() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    for output in [
        ArtifactOutput::Article("# 正文".into()),
        ArtifactOutput::Summary("旧简介".into()),
    ] {
        let stage = output.stage();
        let attempt_id = task.start_stage(stage, 2).unwrap();
        let stage_lineage = lineage(&task, stage, attempt_id);
        task.complete_stage(stage, output, stage_lineage, 3)
            .unwrap();
    }
    task.start_stage(WorkflowStage::Image, 4).unwrap();
    task.fail_stage(
        WorkflowStage::Image,
        Failure {
            code: "TEMPORARY".into(),
            message: "临时失败".into(),
            retryable: true,
            retry_after_ms: None,
        },
        5,
    )
    .unwrap();

    task.edit_summary(
        "人工简介",
        ArtifactLineage {
            package_version: task.package_version,
            source_article_version: task.stages.article.artifact_version,
            source_summary_version: None,
            template_id: None,
            template_version: None,
            provider_id: None,
            provider_revision: None,
            model: None,
            attempt_id: "manual-summary-after-failure".into(),
        },
        6,
    )
    .unwrap();

    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert!(task.stages.image.failure.is_none());
    assert_eq!(task.retry_failed(7), Err(WorkflowError::NothingToRetry));
    assert_eq!(task.approve(8), Err(WorkflowError::NotReadyForApproval));
}

/// 用人工正文恢复失败阶段时会清除当前失败元数据并保留显式再生成门禁。
#[test]
fn article_edit_clears_its_previous_failure() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    task.start_stage(WorkflowStage::Article, 2).unwrap();
    task.fail_stage(
        WorkflowStage::Article,
        Failure {
            code: "TEMPORARY".into(),
            message: "临时失败".into(),
            retryable: true,
            retry_after_ms: None,
        },
        3,
    )
    .unwrap();

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
            attempt_id: "manual-article-after-failure".into(),
        },
        4,
    )
    .unwrap();

    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert!(task.stages.article.failure.is_none());
    assert!(task.stages.article.active_attempt_id.is_none());
    assert_eq!(task.retry_failed(5), Err(WorkflowError::NothingToRetry));
}

/// 只读批次预检不改变原任务状态。
#[test]
fn batch_preview_reports_without_queueing_original_tasks() {
    let tasks = vec![ArticleTask::new("a", "标题", 0)];
    let result = preview_selected_batch(&tasks, ["a".into()]);

    assert_eq!(result.queued_task_ids, vec!["a"]);
    assert_eq!(tasks[0].overall_status(), OverallStatus::Draft);
}

/// 批量重试只排队失败行，并保留已成功阶段。
#[test]
fn retry_batch_queues_only_rows_with_recoverable_stages() {
    let mut failed = ArticleTask::new("failed", "失败", 0);
    failed.queue_initial(1).unwrap();
    failed.start_stage(WorkflowStage::Article, 2).unwrap();
    failed
        .fail_stage(
            WorkflowStage::Article,
            Failure {
                code: "TEMPORARY".into(),
                message: "暂时失败".into(),
                retryable: true,
                retry_after_ms: None,
            },
            3,
        )
        .unwrap();
    let draft = ArticleTask::new("draft", "草稿", 0);
    let mut tasks = vec![failed, draft];

    let result = prepare_retry_batch(&mut tasks, ["failed".into(), "draft".into()], 4);

    assert_eq!(result.queued_task_ids, vec!["failed"]);
    assert_eq!(result.skipped[0].task_id, "draft");
    assert_eq!(tasks[0].stages.article.status, StageStatus::Queued);
}

/// 重新生成简介只让简介和配图重新运行，正文与格式保持成功。
#[test]
fn summary_regeneration_preserves_unrelated_successful_stages() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    let previous_artifacts = task.artifacts.clone();

    task.regenerate_stage(WorkflowStage::Summary, 30).unwrap();

    assert_eq!(task.package_version, 2);
    assert_eq!(task.package_history.len(), 1);
    assert_eq!(task.package_history[0].version, 1);
    assert_eq!(task.package_history[0].review_status, ReviewStatus::Pending);
    assert_eq!(task.package_history[0].artifacts, previous_artifacts);
    assert!(task.approved_history.is_empty());
    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, StageStatus::Queued);
    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert_eq!(task.stages.format.status, StageStatus::Succeeded);
    assert!(
        WorkflowStage::ordered()
            .into_iter()
            .all(|stage| task.stages.get(stage).attempts == 0)
    );
    let attempt_id = task.start_stage(WorkflowStage::Summary, 31).unwrap();
    assert!(attempt_id.ends_with(":v2:Summary:attempt-1"));
}

/// 每个重新生成入口都创建新包版本，并冻结尚未批准的旧包。
#[test]
fn every_stage_regeneration_versions_and_snapshots_unapproved_package() {
    let mut original = ArticleTask::new("a", "原始标题", 0);
    original
        .update_inputs(vec!["原始关键词".into()], "原始要求", 1)
        .unwrap();
    original.queue_initial(2).unwrap();
    complete_all(&mut original);
    let expected_artifacts = original.artifacts.clone();

    for (index, stage) in WorkflowStage::ordered().into_iter().enumerate() {
        let mut task = original.clone();
        task.regenerate_stage(stage, 100 + index as i64).unwrap();

        assert_eq!(task.package_version, 2, "{stage:?}");
        assert_eq!(task.package_history.len(), 1, "{stage:?}");
        let snapshot = &task.package_history[0];
        assert_eq!(snapshot.version, 1, "{stage:?}");
        assert_eq!(snapshot.title, "原始标题", "{stage:?}");
        assert_eq!(snapshot.keywords, vec!["原始关键词"], "{stage:?}");
        assert_eq!(snapshot.requirements, "原始要求", "{stage:?}");
        assert_eq!(snapshot.review_status, ReviewStatus::Pending, "{stage:?}");
        assert_eq!(snapshot.artifacts, expected_artifacts, "{stage:?}");
        assert_eq!(
            task.stages.get(stage).status,
            StageStatus::Queued,
            "{stage:?}"
        );
    }
}

/// 修改已有内容包的输入会创建新版本，同时保留批准历史。
#[test]
fn input_change_starts_clean_version_after_generation() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.approve(30).unwrap();

    task.update_inputs(vec!["新关键词".into()], "新要求", 31)
        .unwrap();

    assert_eq!(task.package_version, 2);
    assert_eq!(task.keywords, vec!["新关键词"]);
    assert_eq!(task.approved_history.len(), 1);
    assert_eq!(task.stages, StageStates::default());
}

/// 批准历史同时冻结当时的原始输入，后续新版本不得回写旧记录。
#[test]
fn approved_history_freezes_original_inputs() {
    let mut task = ArticleTask::new("a", "原始标题", 0);
    task.update_inputs(
        vec!["原始关键词".into(), "第二关键词".into()],
        "原始内容要求",
        1,
    )
    .unwrap();
    task.queue_initial(2).unwrap();
    complete_all(&mut task);
    let approved_artifacts = task.artifacts.clone();
    task.approve(40).unwrap();

    task.update_inputs(vec!["新关键词".into()], "新内容要求", 41)
        .unwrap();
    task.title = "新标题".into();

    let approved = &task.approved_history[0];
    assert_eq!(approved.version, 1);
    assert_eq!(approved.title, "原始标题");
    assert_eq!(approved.keywords, vec!["原始关键词", "第二关键词"]);
    assert_eq!(approved.requirements, "原始内容要求");
    assert_eq!(approved.artifacts, approved_artifacts);
    assert_eq!(task.package_version, 2);
    assert_eq!(task.keywords, vec!["新关键词"]);
}

/// 首次单步生成复用当前包，成功后不会暗中排队后续阶段。
#[test]
fn single_stage_first_run_stays_in_package_and_stops_at_target() {
    let mut task = ArticleTask::new("a", "标题", 0);

    task.queue_single_stage(WorkflowStage::Article, 1).unwrap();
    assert_eq!(task.package_version, 1);
    let attempt_id = task.start_stage(WorkflowStage::Article, 2).unwrap();
    task.complete_stage_only(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        lineage(&task, WorkflowStage::Article, attempt_id),
        3,
    )
    .unwrap();

    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, StageStatus::NotStarted);
    assert_eq!(task.overall_status(), OverallStatus::Draft);
}

/// 已执行阶段再次单步生成会创建派生包，同时仍只运行目标阶段。
#[test]
fn single_stage_regeneration_versions_and_stops_at_target() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);

    task.queue_single_stage(WorkflowStage::Summary, 30).unwrap();
    assert_eq!(task.package_version, 2);
    assert_eq!(task.stages.summary.status, StageStatus::Queued);
    assert_eq!(task.stages.image.status, StageStatus::Stale);
    let attempt_id = task.start_stage(WorkflowStage::Summary, 31).unwrap();
    task.complete_stage_only(
        WorkflowStage::Summary,
        ArtifactOutput::Summary("新简介".into()),
        lineage(&task, WorkflowStage::Summary, attempt_id),
        32,
    )
    .unwrap();

    assert_eq!(task.stages.summary.status, StageStatus::Succeeded);
    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert_eq!(task.overall_status(), OverallStatus::PartialFailure);
}

/// 简介重生成只让已经执行过的配图过期，首次配图继续复用派生包。
#[test]
fn single_stage_summary_regeneration_preserves_unstarted_image() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_single_stage(WorkflowStage::Article, 1).unwrap();
    let article_attempt = task.start_stage(WorkflowStage::Article, 2).unwrap();
    task.complete_stage_only(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        lineage(&task, WorkflowStage::Article, article_attempt),
        3,
    )
    .unwrap();
    task.queue_single_stage(WorkflowStage::Summary, 4).unwrap();
    task.start_stage(WorkflowStage::Summary, 5).unwrap();
    task.fail_stage(
        WorkflowStage::Summary,
        Failure {
            code: "PROVIDER_TIMEOUT".into(),
            message: "简介生成超时".into(),
            retryable: true,
            retry_after_ms: None,
        },
        6,
    )
    .unwrap();

    task.queue_single_stage(WorkflowStage::Summary, 7).unwrap();
    assert_eq!(task.package_version, 2);
    assert_eq!(task.stages.image.status, StageStatus::NotStarted);
    let summary_attempt = task.start_stage(WorkflowStage::Summary, 8).unwrap();
    task.complete_stage_only(
        WorkflowStage::Summary,
        ArtifactOutput::Summary("新简介".into()),
        lineage(&task, WorkflowStage::Summary, summary_attempt),
        9,
    )
    .unwrap();
    task.queue_single_stage(WorkflowStage::Image, 10).unwrap();

    assert_eq!(task.package_version, 2);
    assert_eq!(task.stages.image.status, StageStatus::Queued);
}

/// 单步补齐最后阶段后仍会进入待审核，而不是停留在草稿。
#[test]
fn single_stage_completion_enters_review_when_package_is_complete() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.queue_single_stage(WorkflowStage::Format, 30).unwrap();
    let attempt_id = task.start_stage(WorkflowStage::Format, 31).unwrap();
    task.complete_stage_only(
        WorkflowStage::Format,
        ArtifactOutput::Format("<h1>正文</h1>".into()),
        lineage(&task, WorkflowStage::Format, attempt_id),
        32,
    )
    .unwrap();

    assert_eq!(task.review_status, ReviewStatus::Pending);
    assert_eq!(task.overall_status(), OverallStatus::PendingReview);
}

/// 单步恢复和取消只触碰目标阶段，不扩大运行所有权。
#[test]
fn single_stage_recovery_and_cancel_are_scope_aware() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.stages.article.status = StageStatus::Queued;
    task.stages.summary.status = StageStatus::Queued;

    assert!(task.recover_interrupted_stage_with_policy(WorkflowStage::Article, 1, true,));
    assert_eq!(task.stages.article.status, StageStatus::Interrupted);
    assert_eq!(task.stages.summary.status, StageStatus::Queued);
    assert!(task.cancel_pending_stage(WorkflowStage::Summary, 2));
    assert_eq!(task.stages.article.status, StageStatus::Interrupted);
    assert_eq!(task.stages.summary.status, StageStatus::Cancelled);
}
