use super::*;

/// 一次批量输入 20 个有效标题时保留重复项、忽略空行并生成独立任务 ID。
#[test]
fn bulk_title_input_creates_twenty_independent_tasks() {
    let mut lines = (1..=19)
        .map(|index| format!("SEO 文章标题 {index}"))
        .collect::<Vec<_>>();
    lines.push("SEO 文章标题 1".into());
    lines.insert(7, "   ".into());
    let input = lines.join("\n");
    let (titles, ignored_blank_count) = parse_article_titles(&input).unwrap();
    let mut workspace = WorkspaceData::default();

    let result = append_article_titles(&mut workspace, titles, ignored_blank_count, 123);

    assert_eq!(result.created_count, 20);
    assert_eq!(result.ignored_blank_count, 1);
    assert_eq!(result.duplicate_count, 1);
    assert_eq!(workspace.articles.len(), 20);
    assert_eq!(
        result
            .task_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        20
    );
    assert_eq!(
        workspace
            .articles
            .iter()
            .filter(|article| article.title == "SEO 文章标题 1")
            .count(),
        2
    );
}

/// 删除文章只回收最后一个冻结绑定引用已经消失的旧凭据。
#[test]
fn article_deletion_collects_only_last_unused_credential_reference() {
    let mut workspace = WorkspaceData::default();
    workspace.articles = vec![
        bifang_content_production_assistant_core::ArticleTask::new("delete-first", "第一篇", 0),
        bifang_content_production_assistant_core::ArticleTask::new("delete-last", "第二篇", 0),
    ];
    let provider = ProviderConfig {
        id: "provider".into(),
        name: "provider".into(),
        revision: 1,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: "https://example.com/v1".into(),
        text_model: "text".into(),
        article_model: Some("article".into()),
        summary_model: Some("summary".into()),
        image_model: "image".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    };
    let templates = workspace.prompt_templates.clone();
    let binding = |task_id: &str| PackageExecutionBinding {
        task_id: task_id.into(),
        package_version: 1,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: templates[0].clone(),
        summary_template: templates[1].clone(),
        image_template: templates[2].clone(),
        article_provider: Some(ProviderExecutionBinding {
            config: provider.clone(),
            credential_ref: Some("credential-shared-old".into()),
        }),
        summary_provider: None,
        image_provider: None,
    };
    workspace.execution_bindings = vec![binding("delete-first"), binding("delete-last")];

    assert!(
        remove_article_and_collect_unused_credentials(&mut workspace, "delete-first")
            .unwrap()
            .is_empty()
    );
    assert_eq!(workspace.execution_bindings.len(), 1);
    assert_eq!(
        remove_article_and_collect_unused_credentials(&mut workspace, "delete-last").unwrap(),
        vec!["credential-shared-old"]
    );
    assert!(workspace.execution_bindings.is_empty());
    assert!(
        workspace
            .pending_credential_deletions
            .contains("credential-shared-old")
    );

    let mut legacy_fallback = WorkspaceData::default();
    legacy_fallback
        .articles
        .push(bifang_content_production_assistant_core::ArticleTask::new(
            "legacy",
            "旧文章",
            0,
        ));
    let legacy_binding = binding("legacy");
    legacy_fallback.provider_configs.push(ProviderConfig {
        id: "credential-shared-old".into(),
        ..provider
    });
    legacy_fallback.execution_bindings.push(legacy_binding);
    assert!(
        remove_article_and_collect_unused_credentials(&mut legacy_fallback, "legacy")
            .unwrap()
            .is_empty()
    );
}

/// 人工派生产物同时记录目标包版本和真实正文产物版本。
#[test]
fn manual_lineage_uses_actual_article_source_version() {
    let mut article = bifang_content_production_assistant_core::ArticleTask::new("task", "标题", 0);
    article.package_version = 2;
    article.stages.article.artifact_version = Some(1);
    article.review_status = ReviewStatus::Approved;

    let lineage = manual_lineage(&article, "manual-summary", true);

    assert_eq!(lineage.package_version, 3);
    assert_eq!(lineage.source_article_version, Some(1));
    assert!(lineage.provider_revision.is_none());
}
