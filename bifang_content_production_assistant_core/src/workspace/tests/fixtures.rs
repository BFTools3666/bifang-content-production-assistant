use super::*;

#[test]
fn fixture_normalization_replaces_real_provider_state() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(real_provider("real"));
    workspace
        .provider_config_history
        .push(real_provider("real-old"));
    workspace
        .provider_credential_refs
        .insert("real".into(), "secret-ref".into());

    assert!(workspace.normalize_environment_for_mode(true));
    assert_eq!(workspace.provider_configs.len(), 1);
    assert_eq!(workspace.provider_configs[0].kind, ProviderKind::Fixture);
    assert!(workspace.provider_config_history.is_empty());
    assert!(workspace.provider_credential_refs.is_empty());
    assert!(
        workspace
            .pending_credential_deletions
            .contains("secret-ref")
    );
}

/// 环境正规化仍须保留冻结绑定使用的旧凭据，不能提前加入删除队列。
#[test]
fn fixture_normalization_retains_a_bound_credential_reference() {
    let mut workspace = WorkspaceData::default();
    let provider = real_provider("real-bound");
    workspace.provider_configs.push(provider.clone());
    workspace
        .provider_credential_refs
        .insert(provider.id.clone(), "secret-bound".into());
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "task-bound".into(),
        package_version: 1,
        prompt_renderer_version: V3_NEW_BINDING_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("secret-bound".into()),
        }),
        summary_provider: None,
        image_provider: None,
    });

    assert!(workspace.normalize_environment_for_mode(true));
    assert!(workspace.provider_credential_refs.is_empty());
    assert!(
        !workspace
            .pending_credential_deletions
            .contains("secret-bound")
    );
}

/// 正常模式会删除持久数据中的夹具及其孤立凭据引用。
#[test]
fn normal_normalization_removes_fixture_provider_state() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(ProviderConfig {
        id: "fixture".into(),
        name: "fixture".into(),
        revision: 1,
        kind: ProviderKind::Fixture,
        enabled: true,
        base_url: String::new(),
        text_model: "fixture".into(),
        article_model: Some("fixture-article".into()),
        summary_model: Some("fixture-summary".into()),
        image_model: "fixture".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    });
    workspace
        .provider_credential_refs
        .insert("fixture".into(), "fixture-ref".into());

    assert!(workspace.normalize_environment_for_mode(false));
    assert!(workspace.provider_configs.is_empty());
    assert!(workspace.provider_credential_refs.is_empty());
    assert!(
        workspace
            .pending_credential_deletions
            .contains("fixture-ref")
    );
}

/// v7 当前、包历史与批准历史都以各自图片初始化原图引用，不能串用别的版本。
#[test]
fn image_editor_migration_initializes_each_snapshot_original() {
    let mut workspace = WorkspaceData {
        schema_version: PRE_IMAGE_EDITOR_SCHEMA_VERSION,
        ..WorkspaceData::default()
    };
    let mut article = ArticleTask::new("task", "标题", 1);
    article.artifacts.image_ref = Some("/managed/current.png".into());
    article.package_history.push(ArticlePackageSnapshot {
        version: 1,
        title: "旧标题".into(),
        keywords: Vec::new(),
        requirements: String::new(),
        review_status: ReviewStatus::Draft,
        artifacts: ArticleArtifacts {
            image_ref: Some("/managed/history.png".into()),
            ..ArticleArtifacts::default()
        },
        captured_at_ms: 2,
    });
    article.approved_history.push(ApprovedPackage {
        version: 1,
        title: "批准标题".into(),
        keywords: Vec::new(),
        requirements: String::new(),
        artifacts: ArticleArtifacts {
            image_ref: Some("/managed/approved.png".into()),
            ..ArticleArtifacts::default()
        },
        approved_at_ms: 3,
    });
    workspace.articles.push(article);

    assert!(workspace.migrate_to_current_schema().unwrap());
    let article = &workspace.articles[0];
    assert_eq!(
        article.artifacts.original_image_ref.as_deref(),
        Some("/managed/current.png")
    );
    assert_eq!(
        article.package_history[0]
            .artifacts
            .original_image_ref
            .as_deref(),
        Some("/managed/history.png")
    );
    assert_eq!(
        article.approved_history[0]
            .artifacts
            .original_image_ref
            .as_deref(),
        Some("/managed/approved.png")
    );
}

/// v8 JSON 缺少新字段时以最早尝试时间回填创建时间，usage 保持“未提供”。
#[test]
fn generation_metrics_migration_accepts_legacy_json_and_is_idempotent() {
    let mut workspace = WorkspaceData {
        schema_version: PRE_GENERATION_METRICS_SCHEMA_VERSION,
        ..WorkspaceData::default()
    };
    let mut article = ArticleTask::new("legacy-metrics", "旧文章", 900);
    article.queue_initial(150).unwrap();
    article.start_stage(WorkflowStage::Article, 200).unwrap();
    workspace.articles.push(article);
    let mut legacy = serde_json::to_value(workspace).unwrap();
    let legacy_article = legacy["articles"][0].as_object_mut().unwrap();
    legacy_article.remove("createdAtMs");
    legacy_article["attemptHistory"][0]
        .as_object_mut()
        .unwrap()
        .remove("tokenUsage");
    let mut reopened: WorkspaceData = serde_json::from_value(legacy).unwrap();

    assert!(reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(reopened.articles[0].created_at_ms, 200);
    assert!(
        reopened.articles[0].attempt_history[0]
            .token_usage
            .is_none()
    );
    assert!(!reopened.migrate_to_current_schema().unwrap());
}
