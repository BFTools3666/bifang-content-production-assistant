use super::*;
use crate::{ArtifactLineage, ArtifactOutput, WorkflowError};

fn stage_binding(workspace: &WorkspaceData, renderer: u32) -> PackageExecutionBinding {
    PackageExecutionBinding {
        task_id: String::new(),
        package_version: 0,
        prompt_renderer_version: renderer,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: None,
    }
}

fn bound_provider(id: &str) -> ProviderExecutionBinding {
    ProviderExecutionBinding {
        config: real_provider(id),
        credential_ref: Some(format!("vault-{id}")),
    }
}

#[test]
fn first_stage_keeps_old_renderer_and_unrelated_frozen_binding() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("task", "标题", 1));
    let mut old = stage_binding(&workspace, LEGACY_PROMPT_RENDERER_VERSION);
    old.task_id = "task".into();
    old.package_version = 1;
    old.summary_provider = Some(bound_provider("summary-old"));
    upsert_execution_binding(&mut workspace, old);
    let mut proposed = stage_binding(&workspace, CURRENT_PROMPT_RENDERER_VERSION);
    proposed.article_provider = Some(bound_provider("article-new"));

    assert_eq!(
        inherited_stage_prompt_renderer_version(&workspace, "task", WorkflowStage::Article, 1),
        Some(LEGACY_PROMPT_RENDERER_VERSION)
    );
    queue_single_stage_in_workspace(
        &mut workspace,
        "task",
        WorkflowStage::Article,
        1,
        1,
        false,
        None,
        None,
        &proposed,
        "batch-first".into(),
        20,
        false,
    )
    .unwrap();
    let binding = find_execution_binding(&workspace, "task", 1).unwrap();
    assert_eq!(
        binding.prompt_renderer_version,
        LEGACY_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        binding.article_provider.as_ref().unwrap().config.id,
        "article-new"
    );
    assert_eq!(
        binding.summary_provider.as_ref().unwrap().config.id,
        "summary-old"
    );
    assert_eq!(
        workspace.articles[0].stages.article.status,
        StageStatus::Queued
    );
    assert_eq!(
        workspace.run_history[0].target_stage,
        Some(WorkflowStage::Article)
    );
    assert_eq!(workspace.run_history[0].task_ids, vec!["task"]);
    assert_eq!(workspace.run_history[0].started_at_ms, 20);
}

#[test]
fn regenerating_article_uses_current_renderer_in_new_package() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task", "标题", 1);
    let lineage = manual_lineage(&article, false, "manual-article".into());
    article.edit_article("# 正文", lineage, 2).unwrap();
    workspace.articles.push(article);
    let mut old = stage_binding(&workspace, LEGACY_PROMPT_RENDERER_VERSION);
    old.task_id = "task".into();
    old.package_version = 1;
    old.summary_provider = Some(bound_provider("summary-old"));
    upsert_execution_binding(&mut workspace, old);
    let mut proposed = stage_binding(&workspace, CURRENT_PROMPT_RENDERER_VERSION);
    proposed.article_provider = Some(bound_provider("article-new"));
    let row = workspace.articles[0].row_version;

    assert_eq!(
        inherited_stage_prompt_renderer_version(&workspace, "task", WorkflowStage::Article, 1),
        None
    );
    queue_single_stage_in_workspace(
        &mut workspace,
        "task",
        WorkflowStage::Article,
        row,
        1,
        false,
        None,
        None,
        &proposed,
        "batch-new".into(),
        3,
        false,
    )
    .unwrap();
    let binding = find_execution_binding(&workspace, "task", 2).unwrap();
    assert_eq!(
        binding.prompt_renderer_version,
        CURRENT_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        binding.article_provider.as_ref().unwrap().config.id,
        "article-new"
    );
    assert!(binding.summary_provider.is_none());
    assert_eq!(workspace.articles[0].package_version, 2);
    assert!(find_execution_binding(&workspace, "task", 1).is_some());
}

#[test]
fn regenerating_summary_inherits_article_but_clears_image_binding() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task", "标题", 1);
    let article_lineage = manual_lineage(&article, false, "manual-article".into());
    article.edit_article("# 正文", article_lineage, 2).unwrap();
    let summary_lineage = manual_lineage(&article, true, "manual-summary".into());
    article.edit_summary("简介", summary_lineage, 3).unwrap();
    workspace.articles.push(article);
    let mut old = stage_binding(&workspace, LEGACY_PROMPT_RENDERER_VERSION);
    old.task_id = "task".into();
    old.package_version = 1;
    old.article_provider = Some(bound_provider("article-old"));
    old.image_provider = Some(bound_provider("image-old"));
    upsert_execution_binding(&mut workspace, old);
    let mut proposed = stage_binding(&workspace, CURRENT_PROMPT_RENDERER_VERSION);
    proposed.summary_provider = Some(bound_provider("summary-new"));
    let row = workspace.articles[0].row_version;

    assert_eq!(
        inherited_stage_prompt_renderer_version(&workspace, "task", WorkflowStage::Summary, 1),
        Some(LEGACY_PROMPT_RENDERER_VERSION)
    );
    queue_single_stage_in_workspace(
        &mut workspace,
        "task",
        WorkflowStage::Summary,
        row,
        1,
        false,
        None,
        None,
        &proposed,
        "batch-summary".into(),
        4,
        false,
    )
    .unwrap();
    let binding = find_execution_binding(&workspace, "task", 2).unwrap();
    assert_eq!(
        binding.prompt_renderer_version,
        LEGACY_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        binding.article_provider.as_ref().unwrap().config.id,
        "article-old"
    );
    assert_eq!(
        binding.summary_provider.as_ref().unwrap().config.id,
        "summary-new"
    );
    assert!(binding.image_provider.is_none());
    assert_eq!(
        workspace.run_history[0].target_stage,
        Some(WorkflowStage::Summary)
    );
}

#[test]
fn stage_preflight_and_stale_update_reject_without_side_effects() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("task", "标题", 1));
    let provider = fixture_provider("image");
    workspace.provider_configs.push(provider.clone());
    let proposed = stage_binding(&workspace, CURRENT_PROMPT_RENDERER_VERSION);
    let original = workspace.clone();

    assert_eq!(
        preflight_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Summary,
            1,
            1,
            false,
            None,
            None,
            2,
        ),
        Err(StageQueueError::Workflow(
            WorkflowError::DependencyNotReady {
                stage: WorkflowStage::Summary,
            }
        ))
    );
    assert_eq!(
        preflight_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Image,
            1,
            1,
            false,
            Some("image"),
            Some(provider.revision),
            2,
        ),
        Err(StageQueueError::Request(
            StageRequestError::ImageConfirmationRequired
        ))
    );
    assert_eq!(
        preflight_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Image,
            1,
            1,
            true,
            Some("image"),
            Some(provider.revision + 1),
            2,
        ),
        Err(StageQueueError::Request(StageRequestError::ProviderChanged))
    );
    assert_eq!(
        queue_single_stage_in_workspace(
            &mut workspace,
            "task",
            WorkflowStage::Article,
            2,
            1,
            false,
            None,
            None,
            &proposed,
            "batch-stale".into(),
            2,
            false,
        ),
        Err(StageQueueError::Request(
            StageRequestError::RevisionConflict
        ))
    );
    assert_eq!(workspace, original);
}

#[test]
fn format_stage_needs_no_provider_and_fixture_mark_uses_target_stage() {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task", "标题", 1);
    let lineage = manual_lineage(&article, false, "manual-article".into());
    article.edit_article("# 正文", lineage, 2).unwrap();
    let lineage = manual_lineage(&article, true, "manual-summary".into());
    article.edit_summary("简介", lineage, 3).unwrap();
    article.queue_single_stage(WorkflowStage::Image, 4).unwrap();
    let attempt_id = article.start_stage(WorkflowStage::Image, 5).unwrap();
    let image_lineage = ArtifactLineage {
        package_version: article.package_version,
        source_article_version: article.stages.article.artifact_version,
        source_summary_version: article.stages.summary.artifact_version,
        template_id: Some("image-template".into()),
        template_version: Some(1),
        provider_id: Some("fixture".into()),
        provider_revision: Some(1),
        model: Some("fixture-image".into()),
        attempt_id,
    };
    article
        .complete_stage_only(
            WorkflowStage::Image,
            ArtifactOutput::Image("images/cover.png".into()),
            image_lineage,
            6,
        )
        .unwrap();
    workspace.articles.push(article);
    workspace.provider_configs.push(fixture_provider("fixture"));
    let proposed = stage_binding(&workspace, CURRENT_PROMPT_RENDERER_VERSION);
    let row = workspace.articles[0].row_version;

    queue_single_stage_in_workspace(
        &mut workspace,
        "task",
        WorkflowStage::Format,
        row,
        1,
        false,
        None,
        None,
        &proposed,
        "batch-format".into(),
        7,
        true,
    )
    .unwrap();
    assert_eq!(
        workspace.articles[0].stages.format.status,
        StageStatus::Queued
    );
    assert!(!workspace.run_history[0].is_test_fixture);
    assert!(find_execution_binding(&workspace, "task", 1).is_some());
}
