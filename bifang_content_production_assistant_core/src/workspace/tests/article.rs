use super::*;

#[test]
fn title_input_preserves_duplicates_but_rejects_invalid_batches() {
    let (titles, ignored) = parse_article_titles(" 第一篇 \n\n第一篇\n 第二篇 ").unwrap();
    assert_eq!(titles, vec!["第一篇", "第一篇", "第二篇"]);
    assert_eq!(ignored, 1);
    assert_eq!(
        parse_article_titles(" \n ").unwrap_err(),
        ArticleTitleError::Empty
    );
    assert_eq!(
        parse_article_titles(&"标题\n".repeat(201)).unwrap_err(),
        ArticleTitleError::TooMany
    );
    assert_eq!(
        parse_article_titles(&"字".repeat(301)).unwrap_err(),
        ArticleTitleError::TooLong
    );
    assert_eq!(ArticleTitleError::TooLong.code(), "TITLE_TOO_LONG");
}

#[test]
fn title_addition_preserves_input_order_and_existing_duplicate_hint() {
    let mut workspace = WorkspaceData::default();
    workspace
        .articles
        .push(ArticleTask::new("old", "相同标题", 1));
    let ids = ["article-1", "article-2", "article-3"];
    let mut ids = ids.into_iter();
    let result = append_article_titles(
        &mut workspace,
        vec!["相同标题".into(), "不同标题".into(), "不同标题".into()],
        2,
        10,
        || ids.next().unwrap().into(),
    );
    assert_eq!(result.created_count, 3);
    assert_eq!(result.duplicate_count, 2);
    assert_eq!(result.ignored_blank_count, 2);
    assert_eq!(result.task_ids, vec!["article-1", "article-2", "article-3"]);
    assert_eq!(workspace.articles[1].created_at_ms, 10);
}

#[test]
fn manual_lineage_uses_actual_source_and_supplied_attempt_id() {
    let mut article = ArticleTask::new("task", "标题", 1);
    article.package_version = 2;
    article.stages.article.artifact_version = Some(1);
    article.review_status = ReviewStatus::Approved;
    let lineage = manual_lineage(&article, true, "manual-attempt".into());
    assert_eq!(lineage.package_version, 3);
    assert_eq!(lineage.source_article_version, Some(1));
    assert_eq!(lineage.attempt_id, "manual-attempt");
}

#[test]
fn deleting_article_queues_only_unreferenced_secret() {
    let mut workspace = WorkspaceData::default();
    workspace.articles = vec![
        ArticleTask::new("first", "第一篇", 0),
        ArticleTask::new("second", "第二篇", 0),
    ];
    let provider = real_provider("real");
    let bound = |task_id: &str| PackageExecutionBinding {
        task_id: task_id.into(),
        package_version: 1,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: Some(ProviderExecutionBinding {
            config: provider.clone(),
            credential_ref: Some("shared-secret".into()),
        }),
        summary_provider: None,
        image_provider: None,
    };
    workspace.execution_bindings = vec![bound("first"), bound("second")];
    assert!(
        remove_article_and_collect_unused_credentials(&mut workspace, "first")
            .unwrap()
            .is_empty()
    );
    assert!(
        !workspace
            .pending_credential_deletions
            .contains("shared-secret")
    );
    assert_eq!(
        remove_article_and_collect_unused_credentials(&mut workspace, "second").unwrap(),
        vec!["shared-secret"]
    );
    assert!(
        workspace
            .pending_credential_deletions
            .contains("shared-secret")
    );
}
