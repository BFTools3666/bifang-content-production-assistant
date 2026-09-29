use super::*;

#[test]
fn fixture_run_marker_requires_explicit_mode_and_all_stage_defaults() {
    let mut workspace = WorkspaceData::default();
    let mut provider = fixture_provider("fixture");
    provider.default_for_summary = false;
    workspace.provider_configs.push(provider);
    assert!(!run_uses_fixture_for_mode(&workspace, false));
    assert!(!run_uses_fixture_for_mode(&workspace, true));
    workspace.provider_configs[0].default_for_summary = true;
    assert!(run_uses_fixture_for_mode(&workspace, true));
    workspace.provider_configs[0].default_for_image = false;
    assert!(!run_uses_fixture_for_mode(&workspace, true));
}

#[test]
fn queued_run_history_is_bounded_and_uses_supplied_time() {
    let mut workspace = WorkspaceData::default();
    for index in 0..101 {
        record_queued_run(
            &mut workspace,
            format!("batch-{index}"),
            vec![format!("article-{index}")],
            index,
            false,
        );
    }
    assert_eq!(workspace.run_history.len(), 100);
    assert_eq!(workspace.run_history[0].id, "batch-100");
    assert_eq!(workspace.run_history[0].started_at_ms, 100);
    assert_eq!(workspace.run_history[99].id, "batch-1");
    assert!(!workspace.run_history[0].is_test_fixture);
}

#[test]
fn single_stage_run_marks_only_selected_fixture_provider() {
    let mut workspace = WorkspaceData::default();
    let provider = fixture_provider("fixture");
    workspace.provider_configs.push(provider);
    record_queued_stage_run(
        &mut workspace,
        "run-image".into(),
        "article".into(),
        WorkflowStage::Image,
        42,
        true,
    );
    assert_eq!(
        workspace.run_history[0].target_stage,
        Some(WorkflowStage::Image)
    );
    assert!(workspace.run_history[0].is_test_fixture);
    record_queued_stage_run(
        &mut workspace,
        "run-format".into(),
        "article".into(),
        WorkflowStage::Format,
        43,
        true,
    );
    assert!(!workspace.run_history[0].is_test_fixture);
}

#[test]
fn stage_claim_checks_control_flags_before_creating_attempt() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task", "标题", 1);
    article.queue_initial(2).unwrap();
    workspace.articles.push(article);

    assert_eq!(
        claim_next_stage_in_workspace(&mut workspace, "task", true, false, None, 3).unwrap(),
        StageClaim::Paused
    );
    assert_eq!(
        claim_next_stage_in_workspace(&mut workspace, "task", false, true, None, 3).unwrap(),
        StageClaim::Cancelled
    );
    assert_eq!(workspace.articles[0].stages.article.attempts, 0);

    let claim =
        claim_next_stage_in_workspace(&mut workspace, "task", false, false, None, 4).unwrap();
    assert!(matches!(
        claim,
        StageClaim::Started {
            stage: WorkflowStage::Article,
            ..
        }
    ));
    assert_eq!(workspace.articles[0].stages.article.attempts, 1);
}

#[test]
fn run_control_and_count_refresh_preserve_paused_state() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task", "标题", 1);
    article.review_status = ReviewStatus::Pending;
    workspace.articles.push(article);
    record_queued_run(
        &mut workspace,
        "batch".into(),
        vec!["task".into()],
        1,
        false,
    );

    transition_controlled_run_status_in_workspace(
        &mut workspace,
        "batch",
        RunStatus::Paused,
        &[RunStatus::Queued, RunStatus::Running],
    )
    .unwrap();
    mark_run_started_in_workspace(&mut workspace, "batch").unwrap();
    refresh_run_counts_in_workspace(&mut workspace, "batch", RunStatus::Running, None).unwrap();

    assert_eq!(workspace.run_history[0].status, RunStatus::Paused);
    assert_eq!(workspace.run_history[0].completed_count, 1);
    assert_eq!(
        derive_terminal_run_status(&workspace, &["task".into()], None).unwrap(),
        RunStatus::Succeeded
    );
    refresh_run_counts_in_workspace(&mut workspace, "batch", RunStatus::Succeeded, Some(5))
        .unwrap();
    assert_eq!(workspace.run_history[0].status, RunStatus::Succeeded);
    assert_eq!(workspace.run_history[0].finished_at_ms, Some(5));
    assert!(
        transition_controlled_run_status_in_workspace(
            &mut workspace,
            "batch",
            RunStatus::Paused,
            &[RunStatus::Running],
        )
        .is_err()
    );
}

#[test]
fn single_stage_terminal_requires_one_article_and_a_terminal_stage() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("task", "标题", 1));
    assert_eq!(
        derive_terminal_run_status(
            &workspace,
            &["task".into(), "other".into()],
            Some(WorkflowStage::Image)
        ),
        Err("单步运行必须且只能包含一篇文章".into())
    );
    assert!(
        derive_terminal_run_status(&workspace, &["task".into()], Some(WorkflowStage::Image))
            .is_err()
    );
}
