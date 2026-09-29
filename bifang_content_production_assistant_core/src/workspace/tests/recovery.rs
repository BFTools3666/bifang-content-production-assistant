use super::*;

#[test]
fn single_stage_restart_recovers_only_target_and_run_record() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task-single", "标题", 1);
    article.stages.article.status = StageStatus::Queued;
    article.stages.summary.status = StageStatus::Queued;
    workspace.articles.push(article);
    record_queued_stage_run(
        &mut workspace,
        "run-single".into(),
        "task-single".into(),
        WorkflowStage::Article,
        2,
        false,
    );

    assert_eq!(recover_interrupted_articles(&mut workspace, 3), 1);
    assert_eq!(
        workspace.articles[0].stages.article.status,
        StageStatus::Interrupted
    );
    assert_eq!(
        workspace.articles[0].stages.summary.status,
        StageStatus::Queued
    );
    assert_eq!(recover_abandoned_runs(&mut workspace, 4), 1);
    assert_eq!(workspace.run_history[0].status, RunStatus::PartialFailure);
    assert_eq!(workspace.run_history[0].failed_count, 1);
    assert_eq!(workspace.run_history[0].finished_at_ms, Some(4));
}

#[test]
fn ark_image_unknown_outcome_stays_non_retryable_after_restart() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task-ark", "标题", 1);
    article.stages.image.status = StageStatus::Running;
    let mut provider = ProviderConfig::volcengine_ark_image_draft();
    provider.id = "ark-image".into();
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: article.id.clone(),
        package_version: article.package_version,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::VolcengineArkUrlV1),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("credential-ark".into()),
        }),
    });
    workspace.articles.push(article);

    assert_eq!(recover_interrupted_articles(&mut workspace, 8), 1);
    let recovered = &workspace.articles[0];
    assert_eq!(recovered.stages.image.status, StageStatus::Interrupted);
    assert_eq!(
        recovered.stages.image.failure.as_ref().unwrap().code,
        "RUN_OUTCOME_UNKNOWN"
    );
    assert!(!recovered.stages.image.failure.as_ref().unwrap().retryable);
}
