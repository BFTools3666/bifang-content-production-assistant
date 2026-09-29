use super::*;
use crate::{ArtifactLineage, OverallStatus};

/// 创建当前文章包使用的最小可追溯血缘。
fn lineage(
    task: &ArticleTask,
    stage: WorkflowStage,
    attempt_id: impl Into<String>,
) -> ArtifactLineage {
    ArtifactLineage {
        package_version: task.package_version,
        source_article_version: (stage != WorkflowStage::Article)
            .then_some(task.stages.article.artifact_version)
            .flatten(),
        source_summary_version: (stage == WorkflowStage::Image)
            .then_some(task.stages.summary.artifact_version)
            .flatten(),
        template_id: (stage != WorkflowStage::Format).then(|| format!("{stage:?}-template")),
        template_version: (stage != WorkflowStage::Format).then_some(1),
        provider_id: (stage != WorkflowStage::Format).then(|| "provider-1".into()),
        provider_revision: (stage != WorkflowStage::Format).then_some(1),
        model: (stage != WorkflowStage::Format).then(|| "model-1".into()),
        attempt_id: attempt_id.into(),
    }
}

/// 逐阶段完成一条任务，确保固定依赖和待审核状态正确推进。
fn complete_all(task: &mut ArticleTask) {
    let outputs = [
        ArtifactOutput::Article("# 正文".into()),
        ArtifactOutput::Summary("简介".into()),
        ArtifactOutput::Image("images/cover.png".into()),
        ArtifactOutput::Format("<h1>正文</h1>".into()),
    ];
    for (index, output) in outputs.into_iter().enumerate() {
        let stage = output.stage();
        let attempt_id = task.start_stage(stage, 10 + index as i64).unwrap();
        let stage_lineage = lineage(task, stage, attempt_id);
        task.complete_stage(stage, output, stage_lineage, 20 + index as i64)
            .unwrap();
    }
}

/// 批量准备只允许明确选中的有效行入队，重复标题仍保持独立任务。
#[test]
fn batch_preparation_isolates_unselected_and_preserves_duplicates() {
    let mut tasks = vec![
        ArticleTask::new("a", "相同标题", 0),
        ArticleTask::new("b", "相同标题", 0),
        ArticleTask::new("c", "未选标题", 0),
        ArticleTask::new("d", "   ", 0),
    ];
    let result = prepare_selected_batch(&mut tasks, ["a".into(), "b".into(), "d".into()], 1);

    assert_eq!(result.queued_task_ids, vec!["a", "b"]);
    assert_eq!(result.skipped[0].reason, "EMPTY_TITLE");
    assert_eq!(tasks[2].overall_status(), OverallStatus::Draft);
}

/// 非排队阶段不能直接开始，且下游依赖不会被绕过。
#[test]
fn rejects_illegal_transitions_and_missing_dependencies() {
    let mut task = ArticleTask::new("a", "标题", 0);
    assert!(matches!(
        task.start_stage(WorkflowStage::Article, 1),
        Err(WorkflowError::InvalidTransition { .. })
    ));
    task.stages.summary.status = StageStatus::Queued;
    assert_eq!(
        task.start_stage(WorkflowStage::Summary, 2),
        Err(WorkflowError::DependencyNotReady {
            stage: WorkflowStage::Summary
        })
    );
}

/// 阶段成功只接受当前尝试 ID，并把同一血缘写入产物与尝试历史。
#[test]
fn attempt_id_flows_from_start_through_artifact_lineage() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let attempt_id = task.start_stage(WorkflowStage::Article, 2).unwrap();

    assert_eq!(
        task.stages.article.active_attempt_id.as_deref(),
        Some(attempt_id.as_str())
    );
    assert_eq!(task.attempt_history.len(), 1);
    assert_eq!(task.attempt_history[0].id, attempt_id);
    assert_eq!(task.attempt_history[0].status, StageAttemptStatus::Running);

    let wrong_lineage = lineage(&task, WorkflowStage::Article, "another-attempt");
    assert_eq!(
        task.complete_stage(
            WorkflowStage::Article,
            ArtifactOutput::Article("# 错误尝试".into()),
            wrong_lineage,
            3,
        ),
        Err(WorkflowError::ArtifactStageMismatch)
    );
    assert!(task.artifacts.markdown.is_none());
    assert_eq!(task.attempt_history[0].status, StageAttemptStatus::Running);

    let expected_lineage = lineage(&task, WorkflowStage::Article, attempt_id.clone());
    task.complete_stage(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        expected_lineage.clone(),
        4,
    )
    .unwrap();

    assert!(task.stages.article.active_attempt_id.is_none());
    assert_eq!(
        task.artifacts.article_lineage.as_ref(),
        Some(&expected_lineage)
    );
    let recorded = &task.attempt_history[0];
    assert_eq!(recorded.status, StageAttemptStatus::Succeeded);
    assert_eq!(recorded.finished_at_ms, Some(4));
    assert_eq!(recorded.lineage.as_ref(), Some(&expected_lineage));
    assert_eq!(
        recorded.lineage.as_ref().unwrap().provider_revision,
        Some(1)
    );
}

/// 成功尝试逐字段保存 Provider usage，任何后续领域变化都不改变创建时间。
#[test]
fn persists_exact_token_usage_without_mutating_created_time() {
    let mut task = ArticleTask::new("usage", "标题", 100);
    task.queue_initial(200).unwrap();
    let attempt_id = task.start_stage(WorkflowStage::Article, 300).unwrap();
    let article_lineage = lineage(&task, WorkflowStage::Article, attempt_id);
    let usage = TokenUsage {
        input_tokens: Some(11),
        output_tokens: None,
        total_tokens: Some(29),
    };

    task.complete_stage_with_usage(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        article_lineage,
        Some(usage.clone()),
        450,
    )
    .unwrap();
    assert!(task.cancel_pending_stage(WorkflowStage::Summary, 500));
    task.update_inputs(vec!["关键词".into()], "新要求", 600)
        .unwrap();

    assert_eq!(task.attempt_history[0].token_usage, Some(usage));
    assert_eq!(task.created_at_ms, 100);
    assert_eq!(task.updated_at_ms, 600);
}

/// 派生产物的来源版本必须等于真实上游产物版本，错误血缘不得产生部分写入。
#[test]
fn derived_artifacts_reject_wrong_source_versions_atomically() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let article_attempt = task.start_stage(WorkflowStage::Article, 2).unwrap();
    let article_lineage = lineage(&task, WorkflowStage::Article, article_attempt);
    task.complete_stage(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        article_lineage,
        3,
    )
    .unwrap();

    let summary_attempt = task.start_stage(WorkflowStage::Summary, 4).unwrap();
    let mut wrong_summary_lineage = lineage(&task, WorkflowStage::Summary, summary_attempt.clone());
    wrong_summary_lineage.source_article_version = None;
    let before_summary = task.clone();
    assert_eq!(
        task.complete_stage(
            WorkflowStage::Summary,
            ArtifactOutput::Summary("错误简介".into()),
            wrong_summary_lineage,
            5,
        ),
        Err(WorkflowError::ArtifactStageMismatch)
    );
    assert_eq!(task, before_summary);

    let summary_lineage = lineage(&task, WorkflowStage::Summary, summary_attempt);
    task.complete_stage(
        WorkflowStage::Summary,
        ArtifactOutput::Summary("简介".into()),
        summary_lineage,
        6,
    )
    .unwrap();
    let image_attempt = task.start_stage(WorkflowStage::Image, 7).unwrap();
    let mut wrong_image_lineage = lineage(&task, WorkflowStage::Image, image_attempt);
    wrong_image_lineage.source_summary_version = None;
    let before_image = task.clone();
    assert_eq!(
        task.complete_stage(
            WorkflowStage::Image,
            ArtifactOutput::Image("images/wrong.png".into()),
            wrong_image_lineage,
            8,
        ),
        Err(WorkflowError::ArtifactStageMismatch)
    );
    assert_eq!(task, before_image);
}

/// 幂等键由 core 对完整冻结请求定界，并保持既有字节级算法不变。
#[test]
fn stage_idempotency_key_is_stable_for_the_same_frozen_request() {
    let task = ArticleTask::new("task-1", "标题", 0);
    let first = task.stage_idempotency_key(
        WorkflowStage::Article,
        "provider-fixture",
        1,
        "",
        "template",
        1,
        "{{title}}",
        "model",
        "标题",
        "",
    );
    let same = task.stage_idempotency_key(
        WorkflowStage::Article,
        "provider-fixture",
        1,
        "",
        "template",
        1,
        "{{title}}",
        "model",
        "标题",
        "",
    );
    let mut next_package = task.clone();
    next_package.package_version = 2;
    let changed_package = next_package.stage_idempotency_key(
        WorkflowStage::Article,
        "provider-fixture",
        1,
        "",
        "template",
        1,
        "{{title}}",
        "model",
        "标题",
        "",
    );

    assert_eq!(first, same);
    assert_eq!(first, "gfs-3e219ffe39f63935d8b0de60084f6768");
    assert_ne!(first, changed_package);

    let changed_parameters = task.stage_idempotency_key(
        WorkflowStage::Article,
        "provider-fixture",
        1,
        "",
        "template",
        1,
        "{{title}}",
        "model",
        "标题",
        "temperatureMilli=700;maxOutputTokens=4096;timeoutMs=60000",
    );
    assert_ne!(first, changed_parameters);
}

/// 一个阶段失败时保留此前产物，普通重试只重新排队该失败阶段。
#[test]
fn retry_only_requeues_failed_stage() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let article_attempt = task.start_stage(WorkflowStage::Article, 2).unwrap();
    let article_lineage = lineage(&task, WorkflowStage::Article, article_attempt);
    task.complete_stage(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        article_lineage,
        3,
    )
    .unwrap();
    let summary_attempt = task.start_stage(WorkflowStage::Summary, 4).unwrap();
    let summary_lineage = lineage(&task, WorkflowStage::Summary, summary_attempt);
    task.complete_stage(
        WorkflowStage::Summary,
        ArtifactOutput::Summary("简介".into()),
        summary_lineage,
        5,
    )
    .unwrap();
    let failed_attempt = task.start_stage(WorkflowStage::Image, 6).unwrap();
    task.fail_stage(
        WorkflowStage::Image,
        Failure {
            code: "RATE_LIMITED".into(),
            message: "服务繁忙，请稍后重试".into(),
            retryable: true,
            retry_after_ms: Some(1_000),
        },
        7,
    )
    .unwrap();
    let recorded_failure = task.attempt_history.last().unwrap();
    assert_eq!(recorded_failure.id, failed_attempt);
    assert_eq!(recorded_failure.status, StageAttemptStatus::Failed);
    assert_eq!(
        recorded_failure
            .failure
            .as_ref()
            .and_then(|failure| failure.retry_after_ms),
        Some(1_000)
    );
    let article_attempts = task.stages.article.attempts;
    let summary_attempts = task.stages.summary.attempts;

    assert_eq!(task.retry_failed(8).unwrap(), WorkflowStage::Image);
    assert_eq!(task.stages.image.status, StageStatus::Queued);
    assert_eq!(task.stages.article.attempts, article_attempts);
    assert_eq!(task.stages.summary.attempts, summary_attempts);
    assert_eq!(task.artifacts.markdown.as_deref(), Some("# 正文"));
    let retry_attempt = task.start_stage(WorkflowStage::Image, 9).unwrap();
    assert_ne!(retry_attempt, failed_attempt);
}

/// 不可重试失败不会被“仅重试失败”重新排队，用户必须明确重新生成。
#[test]
fn non_retryable_failure_requires_explicit_regeneration() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    task.start_stage(WorkflowStage::Article, 2).unwrap();
    task.fail_stage(
        WorkflowStage::Article,
        Failure {
            code: "PAID_RESULT_UNKNOWN".into(),
            message: "结果未知，禁止普通重试".into(),
            retryable: false,
            retry_after_ms: None,
        },
        3,
    )
    .unwrap();

    assert_eq!(task.retry_failed(4), Err(WorkflowError::NothingToRetry));
    assert_eq!(task.stages.article.status, StageStatus::Failed);
    assert_eq!(task.package_version, 1);

    task.regenerate_stage(WorkflowStage::Article, 5).unwrap();
    assert_eq!(task.stages.article.status, StageStatus::Queued);
    assert_eq!(task.package_version, 2);
}

/// 编辑正文会让全部依赖产物过期并阻止批准。
#[test]
fn article_edit_stales_dependants_and_blocks_approval() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    assert_eq!(task.review_status, ReviewStatus::Pending);

    task.edit_article(
        "# 人工修改",
        ArtifactLineage {
            package_version: task.package_version,
            source_article_version: None,
            source_summary_version: None,
            template_id: None,
            template_version: None,
            provider_id: None,
            provider_revision: None,
            model: None,
            attempt_id: "manual-edit".into(),
        },
        30,
    )
    .unwrap();

    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, StageStatus::Stale);
    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert_eq!(task.stages.format.status, StageStatus::Stale);
    assert_eq!(task.approve(31), Err(WorkflowError::NotReadyForApproval));
}

/// worker 拥有排队或运行阶段时，人工编辑不能改写其状态和产物。
#[test]
fn manual_edits_are_rejected_while_worker_owns_a_stage() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let lineage = ArtifactLineage {
        package_version: task.package_version,
        source_article_version: None,
        source_summary_version: None,
        template_id: None,
        template_version: None,
        provider_id: None,
        provider_revision: None,
        model: None,
        attempt_id: "manual".into(),
    };

    assert!(matches!(
        task.edit_article("# 人工", lineage.clone(), 2),
        Err(WorkflowError::InvalidTransition { .. })
    ));
    let _ = task.start_stage(WorkflowStage::Article, 3).unwrap();
    assert!(matches!(
        task.edit_summary("简介", lineage, 4),
        Err(WorkflowError::InvalidTransition { .. })
    ));
}

/// 任一阶段仍由 worker 持有时，重新生成不能切换包版本或遗留旧尝试。
#[test]
fn regeneration_is_rejected_while_any_stage_is_owned() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    let article_attempt = task.start_stage(WorkflowStage::Article, 2).unwrap();
    let article_lineage = lineage(&task, WorkflowStage::Article, article_attempt);
    task.complete_stage(
        WorkflowStage::Article,
        ArtifactOutput::Article("# 正文".into()),
        article_lineage,
        3,
    )
    .unwrap();
    task.start_stage(WorkflowStage::Summary, 4).unwrap();
    let before = task.clone();

    assert_eq!(
        task.regenerate_stage(WorkflowStage::Article, 5),
        Err(WorkflowError::InvalidTransition {
            stage: WorkflowStage::Summary,
            status: StageStatus::Running,
        })
    );
    assert_eq!(task, before);

    let mut blank = ArticleTask::new("blank", "  ", 0);
    let blank_before = blank.clone();
    assert_eq!(
        blank.regenerate_stage(WorkflowStage::Article, 6),
        Err(WorkflowError::EmptyTitle)
    );
    assert_eq!(blank, blank_before);
}

/// 批准版本形成不可变快照，重新生成不会覆盖历史产物。
#[test]
fn approved_snapshot_survives_regeneration() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.approve(40).unwrap();
    let approved = task.approved_history[0].clone();

    task.regenerate_stage(WorkflowStage::Article, 41).unwrap();

    assert_eq!(task.package_version, 2);
    assert_eq!(task.review_status, ReviewStatus::Draft);
    assert_eq!(task.approved_history, vec![approved]);
    assert_eq!(task.artifacts, Default::default());
}

/// 编辑已批准正文会创建派生新版本，保留旧包并让下游产物过期。
#[test]
fn editing_approved_article_creates_derived_version() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.approve(30).unwrap();
    let previous = task.artifacts.clone();
    let next_version = task.package_version + 1;

    task.edit_article(
        "# 人工新正文",
        ArtifactLineage {
            package_version: next_version,
            source_article_version: None,
            source_summary_version: None,
            template_id: None,
            template_version: None,
            provider_id: None,
            provider_revision: None,
            model: None,
            attempt_id: "manual-approved-edit".into(),
        },
        31,
    )
    .unwrap();

    assert_eq!(task.package_version, next_version);
    assert_eq!(task.package_history.last().unwrap().artifacts, previous);
    assert_eq!(task.artifacts.markdown.as_deref(), Some("# 人工新正文"));
    assert_eq!(task.stages.article.status, StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, StageStatus::Stale);
    assert_eq!(task.stages.image.status, StageStatus::Stale);
    assert_eq!(task.stages.format.status, StageStatus::Stale);
}

/// 已批准包的错误编辑血缘在返回失败前不得修改版本或历史。
#[test]
fn rejected_approved_edit_is_atomic() {
    let mut task = ArticleTask::new("a", "标题", 0);
    task.queue_initial(1).unwrap();
    complete_all(&mut task);
    task.approve(30).unwrap();
    let before = task.clone();

    let result = task.edit_article(
        "# 错误版本",
        ArtifactLineage {
            package_version: task.package_version,
            source_article_version: None,
            source_summary_version: None,
            template_id: None,
            template_version: None,
            provider_id: None,
            provider_revision: None,
            model: None,
            attempt_id: "wrong-version".into(),
        },
        31,
    );

    assert_eq!(result, Err(WorkflowError::ArtifactStageMismatch));
    assert_eq!(task, before);
}

/// 应用退出后的运行中阶段恢复为明确的中断状态且可重试。
mod recovery;
mod versioning;
