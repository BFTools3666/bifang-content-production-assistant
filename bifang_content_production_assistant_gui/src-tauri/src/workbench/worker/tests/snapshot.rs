use super::*;

/// 批次快照必须冻结每种 Prompt 的最新版本，而不是历史首版。
#[test]
fn execution_snapshot_uses_latest_prompt_versions() {
    let mut workspace = WorkspaceData::default();
    let mut revised = workspace.prompt_templates[0].clone();
    revised.version = 4;
    revised.content = "最新版 {{title}}".into();
    workspace.prompt_templates.push(revised);
    workspace.provider_configs.push(fixture_provider());

    let snapshot =
        ExecutionSnapshot::from_workspace_with_loader(&workspace, ProviderKind::Fixture, |_| {
            panic!("夹具不应读取凭据")
        })
        .unwrap();

    assert_eq!(snapshot.article_template.version, 4);
    assert_eq!(snapshot.article_template.content, "最新版 {{title}}");
}

/// 已停用的默认配置不会进入新快照，也不会触碰凭据或形成可运行批次。
#[test]
fn disabled_provider_is_excluded_before_credential_access() {
    let mut workspace = WorkspaceData::default();
    let mut provider = fixture_provider();
    provider.enabled = false;
    workspace.provider_configs.push(provider);

    let snapshot =
        ExecutionSnapshot::from_workspace_with_loader(&workspace, ProviderKind::Fixture, |_| {
            panic!("停用 Provider 不应读取凭据")
        })
        .unwrap();
    assert!(snapshot.article_provider.is_none());
    assert!(snapshot.summary_provider.is_none());
    assert!(snapshot.image_provider.is_none());
    assert!(snapshot.validate_for_new_run().is_err());
}

/// 单步快照只读取目标 AI 的凭据，格式阶段完全不触碰 Provider。
#[test]
fn stage_execution_snapshot_loads_only_target_provider() {
    let mut workspace = WorkspaceData::default();
    let mut article = ProviderConfig::open_ai_compatible_draft();
    article.id = "provider-article".into();
    article.article_model = Some("article-model".into());
    article.default_for_article = true;
    article.default_for_summary = false;
    article.default_for_image = false;
    let mut summary = ProviderConfig::open_ai_compatible_draft();
    summary.id = "provider-summary".into();
    summary.summary_model = Some("summary-model".into());
    summary.default_for_article = false;
    summary.default_for_summary = true;
    summary.default_for_image = false;
    let mut image = ProviderConfig::open_ai_compatible_draft();
    image.id = "provider-image".into();
    image.image_model = "image-model".into();
    image.default_for_article = false;
    image.default_for_summary = false;
    image.default_for_image = true;
    workspace.provider_configs = vec![article, summary, image];
    for provider_id in ["provider-article", "provider-summary", "provider-image"] {
        workspace
            .provider_credential_refs
            .insert(provider_id.into(), format!("credential-{provider_id}"));
    }
    let loaded = std::cell::RefCell::new(Vec::new());

    let snapshot = ExecutionSnapshot::from_workspace_for_stage_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        Some(WorkflowStage::Summary),
        |credential_ref| {
            loaded.borrow_mut().push(credential_ref.to_string());
            Ok("secret".into())
        },
    )
    .unwrap();

    assert!(snapshot.article_provider.is_none());
    assert!(snapshot.summary_provider.is_some());
    assert!(snapshot.image_provider.is_none());
    assert_eq!(loaded.into_inner(), vec!["credential-provider-summary"]);
    let format = ExecutionSnapshot::from_workspace_for_stage_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        Some(WorkflowStage::Format),
        |_| panic!("格式阶段不应读取凭据"),
    )
    .unwrap();
    assert!(format.article_provider.is_none());
    assert!(format.summary_provider.is_none());
    assert!(format.image_provider.is_none());
    assert_eq!(format.validate_for_stage(WorkflowStage::Format), Ok(()));
}

/// 系统凭据读取失败必须在排队前传播，不能被折叠成空密钥继续运行。
#[test]
fn credential_access_failure_is_not_swallowed() {
    let mut workspace = WorkspaceData::default();
    let mut provider = fixture_provider();
    provider.kind = ProviderKind::OpenAiCompatible;
    provider.base_url = "https://api.openai.com/v1".into();
    workspace.provider_configs.push(provider);
    workspace
        .provider_credential_refs
        .insert("provider-fixture".into(), "credential-ref".into());

    assert_eq!(
        ExecutionSnapshot::from_workspace_with_loader(
            &workspace,
            ProviderKind::OpenAiCompatible,
            |_| Err("系统凭据库不可用".into()),
        )
        .err(),
        Some("系统凭据库不可用".into())
    );
}

/// 运行确认预检只检查非秘密引用，且接受 OpenAI 文本与火山方舟图片的独立配置。
#[test]
fn execution_preflight_accepts_split_openai_text_and_ark_image_without_secret_access() {
    let mut workspace = WorkspaceData::default();
    let mut text_provider = ProviderConfig::open_ai_compatible_draft();
    text_provider.id = "provider-text".into();
    text_provider.name = "OpenAI 文本".into();
    text_provider.article_model = Some("text-article".into());
    text_provider.summary_model = Some("text-summary".into());
    text_provider.default_for_article = true;
    text_provider.default_for_summary = true;
    text_provider.default_for_image = false;
    let mut image_provider = ProviderConfig::volcengine_ark_image_draft();
    image_provider.id = "provider-ark".into();
    image_provider.name = "火山方舟图片".into();
    image_provider.image_model = "ep-image".into();
    workspace.provider_configs = vec![text_provider, image_provider];
    workspace
        .provider_credential_refs
        .insert("provider-text".into(), "credential-text".into());
    workspace
        .provider_credential_refs
        .insert("provider-ark".into(), "credential-ark".into());

    assert_eq!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible,),
        Ok(())
    );
}

/// 预检发现只配置了非秘密服务但没有凭据引用时，给出可操作错误且不尝试猜测密钥。
#[test]
fn execution_preflight_rejects_missing_credential_reference() {
    let mut workspace = WorkspaceData::default();
    let mut provider = ProviderConfig::open_ai_compatible_draft();
    provider.id = "provider-without-reference".into();
    provider.name = "未保存密钥的服务".into();
    provider.article_model = Some("article-model".into());
    provider.summary_model = Some("summary-model".into());
    provider.image_model = "image-model".into();
    workspace.provider_configs = vec![provider];

    let error =
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible)
            .unwrap_err();

    assert!(error.contains("正文"));
    assert!(error.contains("尚未登记 API 密钥"));
    assert!(error.contains("AI 服务配置"));
}

/// 显式 E2E 夹具没有系统密钥，预检不得把凭据引用当成必填项。
#[test]
fn fixture_execution_preflight_does_not_require_credential_reference() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs = vec![fixture_provider()];

    assert_eq!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::Fixture),
        Ok(())
    );
}

/// 批次快照同时冻结 Provider 修订与当时密钥，不受后续配置变更影响。
#[test]
fn execution_snapshot_freezes_provider_revision_and_secret() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(ProviderConfig {
        id: "provider-real".into(),
        name: "real".into(),
        revision: 7,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: "https://example.com/v1".into(),
        text_model: "text-v1".into(),
        article_model: Some("article-v1".into()),
        summary_model: Some("summary-v1".into()),
        image_model: "image-v1".into(),
        article_parameters: Some(TextGenerationParameters {
            temperature_milli: Some(700),
            max_output_tokens: Some(4_096),
            timeout_ms: 60_000,
        }),
        summary_parameters: Some(TextGenerationParameters {
            temperature_milli: Some(200),
            max_output_tokens: Some(512),
            timeout_ms: 45_000,
        }),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    });
    workspace
        .provider_credential_refs
        .insert("provider-real".into(), "credential-r7".into());

    let mut credential_loads = 0;
    let snapshot = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |reference| {
            credential_loads += 1;
            assert_eq!(reference, "credential-r7");
            Ok("secret-r7".into())
        },
    )
    .unwrap();
    assert_eq!(credential_loads, 1);
    assert_eq!(
        snapshot.prompt_renderer_version,
        CURRENT_PROMPT_RENDERER_VERSION
    );
    workspace.provider_configs[0].revision = 8;
    workspace.provider_configs[0].base_url = "https://changed.example/v1".into();
    workspace.provider_configs[0].article_model = Some("article-v2".into());
    workspace.provider_configs[0].article_parameters = Some(TextGenerationParameters {
        temperature_milli: Some(1_900),
        max_output_tokens: Some(128),
        timeout_ms: 10_000,
    });
    workspace
        .provider_credential_refs
        .insert("provider-real".into(), "credential-r8".into());
    workspace.prompt_templates[0].version = 99;
    workspace.prompt_templates[0].content = "已改变 {{title}}".into();

    let frozen = snapshot.article_provider.unwrap();
    assert_eq!(frozen.config.revision, 7);
    assert_eq!(frozen.config.base_url, "https://example.com/v1");
    assert_eq!(frozen.config.article_model(), "article-v1");
    assert_eq!(
        frozen.config.article_parameters(),
        TextGenerationParameters {
            temperature_milli: Some(700),
            max_output_tokens: Some(4_096),
            timeout_ms: 60_000,
        }
    );
    assert_eq!(frozen.credential_ref.as_deref(), Some("credential-r7"));
    assert_eq!(frozen.api_key.as_deref(), Some("secret-r7"));
    assert_eq!(snapshot.article_template.version, 1);
    assert_ne!(snapshot.article_template.content, "已改变 {{title}}");
}

/// 同一文章包的单步执行必须真正使用已冻结的 Prompt renderer，不能只在持久绑定中保留旧值。
#[tokio::test]
async fn stage_execution_uses_inherited_prompt_renderer_version() {
    let repository = Arc::new(
        WorkspaceRepository::open_database_url("sqlite::memory:")
            .await
            .unwrap(),
    );
    let worker = BatchWorker::new(repository, std::env::temp_dir()).unwrap();

    let snapshot = worker
        .prepare_stage_execution(
            &WorkspaceData::default(),
            WorkflowStage::Format,
            Some(LEGACY_PROMPT_RENDERER_VERSION),
        )
        .unwrap();

    assert_eq!(
        snapshot.prompt_renderer_version,
        LEGACY_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        snapshot
            .to_binding("task-legacy", 3)
            .prompt_renderer_version,
        LEGACY_PROMPT_RENDERER_VERSION
    );
}

/// 正式批次允许 OpenAI 文本与火山方舟图片分别绑定，并按图片类型冻结 Ark 契约。
#[test]
fn execution_snapshot_selects_volcengine_ark_for_image_only() {
    let mut workspace = WorkspaceData::default();
    let mut text_provider = ProviderConfig::open_ai_compatible_draft();
    text_provider.id = "provider-text".into();
    text_provider.article_model = Some("text-article".into());
    text_provider.summary_model = Some("text-summary".into());
    text_provider.default_for_article = true;
    text_provider.default_for_summary = true;
    text_provider.default_for_image = false;
    let mut image_provider = ProviderConfig::volcengine_ark_image_draft();
    image_provider.id = "provider-ark".into();
    workspace.provider_configs = vec![text_provider, image_provider];
    workspace
        .provider_credential_refs
        .insert("provider-text".into(), "credential-text".into());
    workspace
        .provider_credential_refs
        .insert("provider-ark".into(), "credential-ark".into());

    let snapshot = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |reference| Ok(format!("secret-{reference}")),
    )
    .unwrap();

    assert_eq!(
        snapshot.article_provider.as_ref().unwrap().config.id,
        "provider-text"
    );
    assert_eq!(
        snapshot.summary_provider.as_ref().unwrap().config.id,
        "provider-text"
    );
    assert_eq!(
        snapshot.image_provider.as_ref().unwrap().config.id,
        "provider-ark"
    );
    assert_eq!(
        snapshot.image_request_contract,
        ImageRequestContract::VolcengineArkUrlV1
    );
    assert_eq!(
        snapshot.image_provider.as_ref().unwrap().api_key.as_deref(),
        Some("secret-credential-ark")
    );
    assert_eq!(
        snapshot.to_binding("task-ark", 1).image_request_contract,
        Some(ImageRequestContract::VolcengineArkUrlV1)
    );
}

/// 三个 AI 阶段可冻结不同 Provider，持久绑定不含密钥且能恢复原修订与模板。
#[test]
fn execution_binding_round_trip_preserves_independent_stage_configs() {
    let mut workspace = WorkspaceData::default();
    let provider =
        |id: &str, revision: u32, article: bool, summary: bool, image: bool| ProviderConfig {
            id: id.into(),
            name: id.into(),
            revision,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: format!("https://{id}.example/v1"),
            text_model: format!("{id}-text"),
            article_model: Some(format!("{id}-article")),
            summary_model: Some(format!("{id}-summary")),
            image_model: format!("{id}-image"),
            article_parameters: Some(TextGenerationParameters {
                temperature_milli: Some(300),
                max_output_tokens: Some(8_192),
                timeout_ms: 70_000,
            }),
            summary_parameters: Some(TextGenerationParameters {
                temperature_milli: Some(100),
                max_output_tokens: Some(1_024),
                timeout_ms: 40_000,
            }),
            image_parameters: Some(ImageGenerationParameters {
                size: "1024x1536".into(),
                timeout_ms: 80_000,
                ..ImageGenerationParameters::default()
            }),
            default_for_article: article,
            default_for_summary: summary,
            default_for_image: image,
        };
    workspace.provider_configs = vec![
        provider("article", 2, true, false, false),
        provider("summary", 3, false, true, false),
        provider("image", 4, false, false, true),
    ];
    for id in ["article", "summary", "image"] {
        workspace
            .provider_credential_refs
            .insert(id.into(), format!("credential-{id}"));
    }
    let frozen = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |reference| Ok(format!("secret-for-{reference}")),
    )
    .unwrap();
    assert_eq!(
        frozen.article_provider.as_ref().unwrap().config.id,
        "article"
    );
    assert_eq!(
        frozen.summary_provider.as_ref().unwrap().config.id,
        "summary"
    );
    assert_eq!(frozen.image_provider.as_ref().unwrap().config.id, "image");

    let binding = frozen.to_binding("task-a", 7);
    assert_eq!(
        binding.prompt_renderer_version,
        CURRENT_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        binding.image_request_contract,
        Some(ImageRequestContract::CURRENT)
    );
    let serialized = serde_json::to_string(&binding).unwrap();
    assert!(serialized.contains(r#""imageRequestContract":"providerDefaultBase64V2""#));
    assert!(!serialized.contains("secret-for-"));
    assert!(serialized.contains("credential-article"));
    workspace.provider_configs.clear();
    workspace.prompt_templates.clear();

    let restored = ExecutionSnapshot::from_binding_with_loader(
        &binding,
        ProviderKind::OpenAiCompatible,
        |reference| Ok(format!("restored-{reference}")),
    )
    .unwrap();
    assert_eq!(
        restored.article_provider.as_ref().unwrap().config.revision,
        2
    );
    assert_eq!(
        restored.summary_provider.as_ref().unwrap().config.revision,
        3
    );
    assert_eq!(restored.image_provider.as_ref().unwrap().config.revision, 4);
    assert_eq!(
        restored
            .article_provider
            .as_ref()
            .unwrap()
            .config
            .article_model(),
        "article-article"
    );
    assert_eq!(
        restored
            .summary_provider
            .as_ref()
            .unwrap()
            .config
            .summary_model(),
        "summary-summary"
    );
    assert_eq!(
        restored
            .image_provider
            .as_ref()
            .unwrap()
            .config
            .image_parameters(),
        ImageGenerationParameters {
            size: "1024x1536".into(),
            timeout_ms: 80_000,
            ..ImageGenerationParameters::default()
        }
    );
    assert_eq!(
        restored
            .article_provider
            .as_ref()
            .unwrap()
            .api_key
            .as_deref(),
        Some("restored-credential-article")
    );
    assert_eq!(restored.article_template, frozen.article_template);
    assert_eq!(restored.summary_template, frozen.summary_template);
    assert_eq!(restored.image_template, frozen.image_template);
    assert_eq!(
        restored.image_request_contract,
        ImageRequestContract::CURRENT
    );
}

/// 幂等键对相同冻结请求稳定，Prompt 修订或渲染输入变化时一定改变。
#[test]
fn idempotency_key_tracks_template_and_rendered_input() {
    let task = ArticleTask::new("task-1", "标题", 0);
    let provider = fixture_provider();
    let template = PromptTemplate {
        id: "template".into(),
        name: "template".into(),
        kind: PromptTemplateKind::Article,
        content: "{{title}}".into(),
        version: 1,
    };
    let first = task.stage_idempotency_key(
        WorkflowStage::Article,
        &provider.id,
        provider.revision,
        &provider.base_url,
        &template.id,
        template.version,
        &template.content,
        "model",
        "标题",
        "",
    );
    let same = task.stage_idempotency_key(
        WorkflowStage::Article,
        &provider.id,
        provider.revision,
        &provider.base_url,
        &template.id,
        template.version,
        &template.content,
        "model",
        "标题",
        "",
    );
    let mut revised = template.clone();
    revised.version = 2;
    let revised_key = task.stage_idempotency_key(
        WorkflowStage::Article,
        &provider.id,
        provider.revision,
        &provider.base_url,
        &revised.id,
        revised.version,
        &revised.content,
        "model",
        "标题",
        "",
    );
    let changed_prompt = task.stage_idempotency_key(
        WorkflowStage::Article,
        &provider.id,
        provider.revision,
        &provider.base_url,
        &template.id,
        template.version,
        &template.content,
        "model",
        "不同标题",
        "",
    );

    assert_eq!(first, same);
    assert_eq!(first, "gfs-3e219ffe39f63935d8b0de60084f6768");
    assert_ne!(first, revised_key);
    assert_ne!(first, changed_prompt);
}

/// 派生包中复用旧正文时，血缘记录正文产物版本而非新包版本。
#[test]
fn derived_stage_uses_actual_article_artifact_version() {
    let mut task = ArticleTask::new("task", "标题", 0);
    task.package_version = 2;
    task.stages.article.status = bifang_content_production_assistant_core::StageStatus::Succeeded;
    task.stages.article.artifact_version = Some(1);

    assert_eq!(
        source_article_version(&task, WorkflowStage::Summary),
        Some(1)
    );
    assert_eq!(source_article_version(&task, WorkflowStage::Article), None);
}
