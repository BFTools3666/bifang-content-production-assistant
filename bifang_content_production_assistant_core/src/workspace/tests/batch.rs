use std::collections::BTreeMap;

use super::*;

fn binding_template(workspace: &WorkspaceData) -> PackageExecutionBinding {
    PackageExecutionBinding {
        task_id: String::new(),
        package_version: 0,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: None,
    }
}

fn retryable_failure() -> Failure {
    Failure {
        code: "TEMPORARY".into(),
        message: "可恢复错误".into(),
        retryable: true,
        retry_after_ms: None,
    }
}

#[test]
fn selected_batch_freezes_each_package_and_records_only_queued_rows() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("a", "标题 A", 1));
    workspace.articles.push(ArticleTask::new("b", "标题 B", 1));
    let template = binding_template(&workspace);
    let preparation = queue_selected_batch_in_workspace(
        &mut workspace,
        vec!["a".into(), "missing".into(), "a".into(), "b".into()],
        &template,
        "batch-1".into(),
        20,
        false,
    )
    .unwrap();
    assert_eq!(preparation.queued_task_ids, vec!["a", "b"]);
    assert_eq!(workspace.execution_bindings.len(), 2);
    assert_eq!(workspace.execution_bindings[0].task_id, "a");
    assert_eq!(workspace.execution_bindings[1].task_id, "b");
    assert_eq!(workspace.run_history[0].task_ids, vec!["a", "b"]);
    assert_eq!(workspace.run_history[0].started_at_ms, 20);
}

#[test]
fn retry_batch_uses_only_failed_rows_and_rejects_stale_preview() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("a", "标题 A", 1));
    workspace.articles.push(ArticleTask::new("b", "标题 B", 1));
    let template = binding_template(&workspace);
    queue_selected_batch_in_workspace(
        &mut workspace,
        vec!["a".into(), "b".into()],
        &template,
        "batch-initial".into(),
        2,
        false,
    )
    .unwrap();
    let article = find_article_mut(&mut workspace, "a").unwrap();
    article.start_stage(WorkflowStage::Article, 3).unwrap();
    article
        .fail_stage(WorkflowStage::Article, retryable_failure(), 4)
        .unwrap();
    let binding = find_execution_binding(&workspace, "a", 1).unwrap().clone();
    let bindings = BTreeMap::from([("a".into(), binding)]);
    let before = workspace.clone();
    assert!(
        queue_retry_selected_in_workspace(
            &mut workspace,
            vec!["a".into(), "b".into()],
            &["stale".into()],
            &bindings,
            "batch-stale".into(),
            5,
            false,
        )
        .is_err()
    );
    assert_eq!(workspace, before);
    assert!(
        queue_retry_selected_in_workspace(
            &mut workspace,
            vec!["a".into(), "b".into()],
            &["a".into()],
            &BTreeMap::new(),
            "batch-missing-binding".into(),
            5,
            false,
        )
        .is_err()
    );
    assert_eq!(workspace, before);

    let preparation = queue_retry_selected_in_workspace(
        &mut workspace,
        vec!["a".into(), "b".into()],
        &["a".into()],
        &bindings,
        "batch-retry".into(),
        6,
        false,
    )
    .unwrap();
    assert_eq!(preparation.queued_task_ids, vec!["a"]);
    assert_eq!(workspace.run_history[0].task_ids, vec!["a"]);
    assert_eq!(
        find_article_mut(&mut workspace, "a")
            .unwrap()
            .stages
            .article
            .status,
        StageStatus::Queued
    );
    assert_eq!(
        find_article_mut(&mut workspace, "b")
            .unwrap()
            .stages
            .article
            .status,
        StageStatus::Queued
    );
}

#[test]
fn single_retry_and_regeneration_bind_the_resulting_package() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("a", "标题 A", 1));
    let template = binding_template(&workspace);
    queue_selected_batch_in_workspace(
        &mut workspace,
        vec!["a".into()],
        &template,
        "batch-initial".into(),
        2,
        false,
    )
    .unwrap();
    let article = find_article_mut(&mut workspace, "a").unwrap();
    article.start_stage(WorkflowStage::Article, 3).unwrap();
    article
        .fail_stage(WorkflowStage::Article, retryable_failure(), 4)
        .unwrap();
    queue_article_retry_in_workspace(
        &mut workspace,
        "a",
        &template,
        "batch-retry".into(),
        5,
        false,
    )
    .unwrap();
    assert_eq!(workspace.execution_bindings.len(), 1);
    assert_eq!(workspace.run_history[0].id, "batch-retry");

    let mut fresh = WorkspaceData::default();
    fresh.articles.push(ArticleTask::new("b", "标题 B", 1));
    let template = binding_template(&fresh);
    queue_article_regeneration_in_workspace(
        &mut fresh,
        "b",
        &template,
        "batch-new-version".into(),
        9,
        false,
    )
    .unwrap();
    assert_eq!(fresh.articles[0].package_version, 2);
    assert_eq!(fresh.execution_bindings[0].package_version, 2);
    assert_eq!(fresh.run_history[0].id, "batch-new-version");
}
