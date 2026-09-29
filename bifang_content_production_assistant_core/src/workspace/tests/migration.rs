use super::*;

/// 图片请求契约必须由 Provider 类型唯一决定，并使新协议进入幂等身份。
#[test]
fn selects_and_serializes_image_contract_by_provider_kind() {
    assert_eq!(
        ImageRequestContract::for_provider_kind(ProviderKind::OpenAiCompatible),
        ImageRequestContract::ProviderDefaultBase64V2
    );
    assert_eq!(
        ImageRequestContract::for_provider_kind(ProviderKind::Fixture),
        ImageRequestContract::ProviderDefaultBase64V2
    );
    assert_eq!(
        ImageRequestContract::for_provider_kind(ProviderKind::VolcengineArkImage),
        ImageRequestContract::VolcengineArkUrlV1
    );
    assert_eq!(
        ImageRequestContract::VolcengineArkUrlV1.idempotency_material(),
        "imageRequestContract=volcengineArkUrlV1"
    );
    assert_eq!(
        serde_json::to_value(ImageRequestContract::VolcengineArkUrlV1).unwrap(),
        serde_json::json!("volcengineArkUrlV1")
    );
}

/// v1 的单一文本默认位会迁移成正文与简介两个独立默认位。
#[test]
fn migrates_v1_default_for_text_without_losing_meaning() {
    let payload = serde_json::json!({
        "schemaVersion": 1,
        "articles": [],
        "promptTemplates": [],
        "providerConfigs": [{
            "id": "legacy",
            "name": "旧配置",
            "kind": "openAiCompatible",
            "baseUrl": "https://example.com/v1",
            "textModel": "text",
            "imageModel": "image",
            "defaultForText": true,
            "defaultForImage": false
        }],
        "runHistory": []
    });
    let mut workspace: WorkspaceData = serde_json::from_value(payload).unwrap();
    assert!(workspace.provider_configs[0].default_for_article);
    assert!(!workspace.provider_configs[0].default_for_summary);
    assert!(workspace.provider_configs[0].article_model.is_none());
    assert_eq!(workspace.provider_configs[0].revision, 1);

    assert!(workspace.migrate_to_current_schema().unwrap());

    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert!(workspace.provider_configs[0].enabled);
    assert!(workspace.provider_configs[0].default_for_article);
    assert!(workspace.provider_configs[0].default_for_summary);
    assert!(!workspace.provider_configs[0].default_for_image);
    assert_eq!(workspace.provider_configs[0].article_model(), "text");
    assert_eq!(workspace.provider_configs[0].summary_model(), "text");
    assert_eq!(
        workspace.provider_configs[0].article_parameters(),
        TextGenerationParameters::default()
    );
    assert_eq!(
        workspace.provider_configs[0].image_parameters(),
        ImageGenerationParameters::default()
    );
    let serialized = serde_json::to_value(workspace).unwrap();
    assert!(
        serialized["providerConfigs"][0]["defaultForArticle"]
            .as_bool()
            .unwrap()
    );
    assert!(
        serialized["providerConfigs"][0]["defaultForSummary"]
            .as_bool()
            .unwrap()
    );
    assert!(
        serialized["providerConfigs"][0]
            .get("defaultForText")
            .is_none()
    );
    assert_eq!(serialized["providerConfigs"][0]["articleModel"], "text");
    assert_eq!(serialized["providerConfigs"][0]["summaryModel"], "text");
}

/// schema v2 的共享文本模型会迁移到当前模式，并冻结首版图片请求契约。
#[test]
fn materializes_v2_stage_configuration_once() {
    let legacy_provider = serde_json::json!({
        "id": "legacy-v2",
        "name": "旧 v2 配置",
        "revision": 4,
        "kind": "openAiCompatible",
        "baseUrl": "https://example.com/v1",
        "textModel": "shared-text",
        "imageModel": "image",
        "defaultForArticle": false,
        "defaultForSummary": true,
        "defaultForImage": false
    });
    let templates = serde_json::json!([
        {"id":"article","name":"正文","kind":"article","content":"{{title}}","version":1},
        {"id":"summary","name":"简介","kind":"summary","content":"{{article}}","version":1},
        {"id":"image","name":"配图","kind":"image","content":"{{title}} {{summary}}","version":1}
    ]);
    let payload = serde_json::json!({
        "schemaVersion": 2,
        "articles": [],
        "promptTemplates": templates,
        "providerConfigs": [legacy_provider.clone()],
        "providerConfigHistory": [legacy_provider.clone()],
        "providerCredentialRefs": {"legacy-v2": "credential-r4"},
        "executionBindings": [{
            "taskId": "task-v2",
            "packageVersion": 1,
            "articleTemplate": {"id":"article","name":"正文","kind":"article","content":"{{title}}","version":1},
            "summaryTemplate": {"id":"summary","name":"简介","kind":"summary","content":"{{article}}","version":1},
            "imageTemplate": {"id":"image","name":"配图","kind":"image","content":"{{title}} {{summary}}","version":1},
            "articleProvider": {"config": legacy_provider.clone(), "credentialRef": "credential-r4"},
            "summaryProvider": null,
            "imageProvider": {"config": legacy_provider, "credentialRef": "credential-r4"}
        }],
        "runHistory": []
    });
    let mut workspace: WorkspaceData = serde_json::from_value(payload).unwrap();

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(workspace.provider_configs[0].article_model(), "shared-text");
    assert_eq!(workspace.provider_configs[0].summary_model(), "shared-text");
    assert_eq!(
        workspace.provider_config_history[0].article_model(),
        "shared-text"
    );
    assert_eq!(
        workspace.provider_credential_refs["legacy-v2"],
        "credential-r4"
    );
    let binding = &workspace.execution_bindings[0];
    assert_eq!(
        binding
            .article_provider
            .as_ref()
            .unwrap()
            .config
            .article_parameters(),
        TextGenerationParameters::default()
    );
    assert_eq!(
        binding.prompt_renderer_version,
        LEGACY_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        binding.image_request_contract,
        Some(ImageRequestContract::ExplicitBase64V1)
    );
    assert_eq!(
        binding
            .image_provider
            .as_ref()
            .unwrap()
            .config
            .image_parameters(),
        ImageGenerationParameters::default()
    );
    let first_migration = workspace.clone();
    let mut reopened: WorkspaceData =
        serde_json::from_value(serde_json::to_value(workspace).unwrap()).unwrap();
    assert!(!reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened, first_migration);
}

/// v4 缺失的启用字段必须在当前、历史和冻结绑定中全部迁移为启用。
#[test]
fn migrates_v4_enabled_state_across_current_history_and_bindings() {
    let mut provider = serde_json::to_value(real_provider("legacy-v4")).unwrap();
    provider.as_object_mut().unwrap().remove("enabled");
    let defaults = WorkspaceData::default();
    let payload = serde_json::json!({
        "schemaVersion": 4,
        "articles": [],
        "promptTemplates": defaults.prompt_templates,
        "providerConfigs": [provider.clone()],
        "providerConfigHistory": [provider.clone()],
        "providerCredentialRefs": {"legacy-v4": "credential-v4"},
        "pendingCredentialDeletions": [],
        "executionBindings": [{
            "taskId": "task-v4",
            "packageVersion": 1,
            "promptRendererVersion": CURRENT_PROMPT_RENDERER_VERSION,
            "imageRequestContract": "providerDefaultBase64V2",
            "articleTemplate": {"id":"article","name":"正文","kind":"article","content":"{{title}}","version":1},
            "summaryTemplate": {"id":"summary","name":"简介","kind":"summary","content":"{{article}}","version":1},
            "imageTemplate": {"id":"image","name":"配图","kind":"image","content":"{{title}} {{summary}}","version":1},
            "articleProvider": {"config": provider.clone(), "credentialRef": "credential-v4"},
            "summaryProvider": {"config": provider.clone(), "credentialRef": "credential-v4"},
            "imageProvider": {"config": provider, "credentialRef": "credential-v4"}
        }],
        "runHistory": []
    });
    let mut workspace: WorkspaceData = serde_json::from_value(payload).unwrap();
    assert!(!workspace.provider_configs[0].enabled);
    assert!(!workspace.provider_config_history[0].enabled);
    assert!(
        !workspace.execution_bindings[0]
            .article_provider
            .as_ref()
            .unwrap()
            .config
            .enabled
    );

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert!(workspace.provider_configs[0].enabled);
    assert!(workspace.provider_config_history[0].enabled);
    for provider in [
        workspace.execution_bindings[0].article_provider.as_ref(),
        workspace.execution_bindings[0].summary_provider.as_ref(),
        workspace.execution_bindings[0].image_provider.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(provider.config.enabled);
    }
}

/// v5 未持久化水印开关，升级时必须在当前、历史和冻结绑定中保持旧行为。
#[test]
fn migrates_v5_image_parameters_without_enabling_watermarks() {
    let mut provider = serde_json::to_value(real_provider("legacy-v5")).unwrap();
    provider["imageParameters"]
        .as_object_mut()
        .unwrap()
        .remove("watermark");
    let mut disabled_provider = serde_json::to_value(real_provider("disabled-v5")).unwrap();
    disabled_provider["enabled"] = serde_json::json!(false);
    disabled_provider["defaultForArticle"] = serde_json::json!(false);
    disabled_provider["defaultForSummary"] = serde_json::json!(false);
    disabled_provider["defaultForImage"] = serde_json::json!(false);
    disabled_provider["imageParameters"]
        .as_object_mut()
        .unwrap()
        .remove("watermark");
    let defaults = WorkspaceData::default();
    let payload = serde_json::json!({
        "schemaVersion": 5,
        "articles": [],
        "promptTemplates": defaults.prompt_templates,
        "providerConfigs": [provider.clone(), disabled_provider],
        "providerConfigHistory": [provider.clone()],
        "providerCredentialRefs": {"legacy-v5": "credential-v5"},
        "pendingCredentialDeletions": [],
        "executionBindings": [{
            "taskId": "task-v5",
            "packageVersion": 1,
            "promptRendererVersion": CURRENT_PROMPT_RENDERER_VERSION,
            "imageRequestContract": "providerDefaultBase64V2",
            "articleTemplate": {"id":"article","name":"正文","kind":"article","content":"{{title}}","version":1},
            "summaryTemplate": {"id":"summary","name":"简介","kind":"summary","content":"{{article}}","version":1},
            "imageTemplate": {"id":"image","name":"配图","kind":"image","content":"{{title}} {{summary}}","version":1},
            "articleProvider": null,
            "summaryProvider": null,
            "imageProvider": {"config": provider, "credentialRef": "credential-v5"}
        }],
        "runHistory": []
    });
    let mut workspace: WorkspaceData = serde_json::from_value(payload).unwrap();

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert!(!workspace.provider_configs[0].image_parameters().watermark);
    assert!(!workspace.provider_configs[1].enabled);
    assert!(!workspace.provider_configs[1].image_parameters().watermark);
    assert!(
        !workspace.provider_config_history[0]
            .image_parameters()
            .watermark
    );
    let binding = &workspace.execution_bindings[0];
    assert!(
        !binding
            .image_provider
            .as_ref()
            .unwrap()
            .config
            .image_parameters()
            .watermark
    );
    assert_eq!(
        binding.image_request_contract,
        Some(ImageRequestContract::ProviderDefaultBase64V2)
    );

    let first_migration = workspace.clone();
    let mut reopened: WorkspaceData =
        serde_json::from_value(serde_json::to_value(workspace).unwrap()).unwrap();
    assert!(!reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened, first_migration);
}

/// v6 当前集合中精确命中的旧 Ark 图片配置升级协议并保留身份与超时。
#[test]
fn migrates_matching_v6_legacy_volcengine_ark_image_providers_once() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION;
    let direct = legacy_volcengine_ark_image_provider("legacy-ark-direct");
    let mut trailing_slash = legacy_volcengine_ark_image_provider("legacy-ark-slash");
    trailing_slash.base_url.push('/');
    trailing_slash.revision = 12;
    trailing_slash.default_for_image = false;
    trailing_slash.image_parameters.as_mut().unwrap().timeout_ms = 241_000;
    workspace.provider_configs = vec![direct.clone(), trailing_slash.clone()];

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    for (provider, original) in workspace
        .provider_configs
        .iter()
        .zip([direct, trailing_slash])
    {
        assert_eq!(provider.kind, ProviderKind::VolcengineArkImage);
        assert_eq!(provider.base_url, VOLCENGINE_ARK_API_ROOT);
        assert_eq!(provider.id, original.id);
        assert_eq!(provider.revision, original.revision);
        assert_eq!(provider.default_for_image, original.default_for_image);
        assert_eq!(provider.image_model, original.image_model);
        assert_eq!(
            provider.image_parameters(),
            ImageGenerationParameters {
                size: "2K".into(),
                watermark: true,
                timeout_ms: original.image_parameters().timeout_ms,
            }
        );
    }

    let migrated = workspace.clone();
    assert!(!workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace, migrated);
}

/// v6 迁移不触碰协议、地址、文本占用或图片模型任一条件不匹配的配置。
#[test]
fn leaves_nonmatching_v6_provider_configs_unchanged() {
    let base = legacy_volcengine_ark_image_provider("base");
    let mut providers = Vec::new();
    let mut wrong_kind = base.clone();
    wrong_kind.id = "wrong-kind".into();
    wrong_kind.kind = ProviderKind::VolcengineArkImage;
    providers.push(wrong_kind);
    let mut wrong_url = base.clone();
    wrong_url.id = "wrong-url".into();
    wrong_url.base_url = VOLCENGINE_ARK_API_ROOT.into();
    providers.push(wrong_url);
    let mut shared_text_model = base.clone();
    shared_text_model.id = "shared-text-model".into();
    shared_text_model.text_model = "legacy-text-model".into();
    providers.push(shared_text_model);
    let mut article_model = base.clone();
    article_model.id = "article-model".into();
    article_model.article_model = Some("text-model".into());
    providers.push(article_model);
    let mut summary_model = base.clone();
    summary_model.id = "summary-model".into();
    summary_model.summary_model = Some("text-model".into());
    providers.push(summary_model);
    let mut article_default = base.clone();
    article_default.id = "article-default".into();
    article_default.default_for_article = true;
    providers.push(article_default);
    let mut summary_default = base.clone();
    summary_default.id = "summary-default".into();
    summary_default.default_for_summary = true;
    providers.push(summary_default);
    let mut missing_image_model = base;
    missing_image_model.id = "missing-image-model".into();
    missing_image_model.image_model.clear();
    providers.push(missing_image_model);

    let expected = providers.clone();
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION;
    workspace.provider_configs = providers;

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(workspace.provider_configs, expected);
}

/// v6 只升级当前配置，凭据引用、历史配置和冻结执行绑定必须原样保留。
#[test]
fn preserves_credentials_history_and_bindings_during_v6_ark_upgrade() {
    let current = legacy_volcengine_ark_image_provider("legacy-ark-current");
    let mut historical = legacy_volcengine_ark_image_provider("legacy-ark-history");
    historical.revision = 2;
    let binding = PackageExecutionBinding {
        task_id: "task-frozen-ark".into(),
        package_version: 4,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::ProviderDefaultBase64V2),
        article_template: WorkspaceData::default().prompt_templates[0].clone(),
        summary_template: WorkspaceData::default().prompt_templates[1].clone(),
        image_template: WorkspaceData::default().prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: historical.clone(),
            credential_ref: Some("credential-frozen-r2".into()),
        }),
    };
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = VOLCENGINE_ARK_IMAGE_SCHEMA_VERSION;
    workspace.provider_configs.push(current);
    workspace.provider_config_history.push(historical);
    workspace
        .provider_credential_refs
        .insert("legacy-ark-current".into(), "credential-current-r8".into());
    workspace.execution_bindings.push(binding);
    let credential_refs = workspace.provider_credential_refs.clone();
    let provider_history = workspace.provider_config_history.clone();
    let execution_bindings = workspace.execution_bindings.clone();

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(
        workspace.provider_configs[0].kind,
        ProviderKind::VolcengineArkImage
    );
    assert_eq!(workspace.provider_credential_refs, credential_refs);
    assert_eq!(workspace.provider_config_history, provider_history);
    assert_eq!(workspace.execution_bindings, execution_bindings);
}

/// 当前 v7 的显式停用状态必须在幂等重开中保持停用。
#[test]
fn preserves_explicit_disabled_state_in_current_schema() {
    let mut workspace = WorkspaceData::default();
    let mut provider = real_provider("disabled-v5");
    provider.enabled = false;
    provider.default_for_article = false;
    provider.default_for_summary = false;
    provider.default_for_image = false;
    workspace.provider_configs.push(provider);

    assert!(!workspace.migrate_to_current_schema().unwrap());
    assert!(!workspace.provider_configs[0].enabled);
    let mut reopened: WorkspaceData =
        serde_json::from_value(serde_json::to_value(workspace).unwrap()).unwrap();
    assert!(!reopened.migrate_to_current_schema().unwrap());
    assert!(!reopened.provider_configs[0].enabled);
}

/// 初始行只在普通空白工作区写入一次，删除后不会复活。
#[test]
fn applies_initial_articles_once_and_never_revives_them() {
    let mut workspace = WorkspaceData::default();
    assert!(workspace.apply_initial_content(false, 42).unwrap());
    assert_eq!(
        workspace.initial_data_revision,
        CURRENT_INITIAL_CONTENT_REVISION
    );
    assert_eq!(workspace.articles.len(), 14);
    assert!(
        workspace
            .articles
            .iter()
            .all(|article| article.package_version == 1)
    );
    assert!(!workspace.apply_initial_content(false, 43).unwrap());
    assert_eq!(workspace.articles.len(), 14);

    workspace.articles.clear();
    assert!(!workspace.apply_initial_content(false, 44).unwrap());
    assert!(workspace.articles.is_empty());
}

/// 已有内容和显式夹具环境只推进初始账本，不混入业务种子。
#[test]
fn marks_initial_content_processed_without_seeding_in_ineligible_workspaces() {
    let mut existing = WorkspaceData::default();
    existing
        .articles
        .push(ArticleTask::new("existing", "已有文章", 1));
    assert!(existing.apply_initial_content(false, 2).unwrap());
    assert_eq!(existing.articles.len(), 1);
    assert_eq!(
        existing.initial_data_revision,
        CURRENT_INITIAL_CONTENT_REVISION
    );

    let mut fixture = WorkspaceData::default();
    assert!(fixture.apply_initial_content(true, 2).unwrap());
    assert!(fixture.articles.is_empty());
    assert_eq!(
        fixture.initial_data_revision,
        CURRENT_INITIAL_CONTENT_REVISION
    );
}

/// v2 经旧程序迁到 v3 后可能已执行过两种 body，必须拒绝猜测历史契约。
#[test]
fn rejects_a_v2_image_binding_already_migrated_to_ambiguous_v3() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = 3;
    let provider = real_provider("legacy-v2-via-v3");
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "task-v2-via-v3".into(),
        package_version: 1,
        prompt_renderer_version: LEGACY_PROMPT_RENDERER_VERSION,
        image_request_contract: None,
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("credential-v2-via-v3".into()),
        }),
    });

    assert_eq!(
        workspace.migrate_to_current_schema(),
        Err("v3 工作区的旧图片请求契约存在歧义".into())
    );
    assert_eq!(workspace.schema_version, 3);
    assert!(
        workspace.execution_bindings[0]
            .image_request_contract
            .is_none()
    );
}

/// Ark 类型在 v3 尚未存在，不得为伪造的旧绑定猜测 Base64 契约。
#[test]
fn rejects_volcengine_ark_provider_in_v3_binding() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = 3;
    let mut provider = ProviderConfig::volcengine_ark_image_draft();
    provider.id = "ark-via-v3".into();
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "task-ark-via-v3".into(),
        package_version: 1,
        prompt_renderer_version: V3_NEW_BINDING_PROMPT_RENDERER_VERSION,
        image_request_contract: None,
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("credential-ark-via-v3".into()),
        }),
    });

    assert_eq!(
        workspace.migrate_to_current_schema(),
        Err("v3 工作区包含当时不受支持的火山方舟图片契约".into())
    );
    assert_eq!(workspace.schema_version, 3);
}

/// 即使伪造旧工作区已经带有新 wire 枚举，也不能绕过 Ark 引入版本门禁。
#[test]
fn rejects_persisted_volcengine_ark_contract_before_v6() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = 5;
    let mut provider = ProviderConfig::volcengine_ark_image_draft();
    provider.id = "ark-via-v5".into();
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "task-ark-via-v5".into(),
        package_version: 1,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::VolcengineArkUrlV1),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("credential-ark-via-v5".into()),
        }),
    });

    assert_eq!(
        workspace.migrate_to_current_schema(),
        Err("v5 工作区包含当时不受支持的火山方舟图片契约".into())
    );
    assert_eq!(workspace.schema_version, 5);
}

/// c60 文本-only 绑定经 Task 5 初版写成 v3 后没有图片请求历史，可安全保持 v2 契约。
#[test]
fn migrates_c60_binding_without_image_provider_after_v3_upgrade() {
    let provider = serde_json::to_value(real_provider("text-only-via-v3")).unwrap();
    let payload = serde_json::json!({
        "schemaVersion": 3,
        "articles": [],
        "promptTemplates": [],
        "providerConfigs": [],
        "executionBindings": [{
            "taskId": "task-text-only-via-v3",
            "packageVersion": 1,
            "promptRendererVersion": LEGACY_PROMPT_RENDERER_VERSION,
            "articleTemplate": {"id":"article","name":"正文","kind":"article","content":"{{title}}","version":1},
            "summaryTemplate": {"id":"summary","name":"简介","kind":"summary","content":"{{article}}","version":1},
            "imageTemplate": {"id":"image","name":"配图","kind":"image","content":"{{title}} {{summary}}","version":1},
            "articleProvider": {"config": provider.clone(), "credentialRef": "credential-text-only"},
            "summaryProvider": {"config": provider, "credentialRef": "credential-text-only"},
            "imageProvider": null
        }],
        "runHistory": []
    });
    let mut workspace: WorkspaceData = serde_json::from_value(payload).unwrap();

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(
        workspace.execution_bindings[0].image_request_contract,
        Some(ImageRequestContract::ExplicitBase64V1)
    );
    assert!(workspace.execution_bindings[0].image_provider.is_none());
    let first_migration = workspace.clone();
    let mut reopened: WorkspaceData =
        serde_json::from_value(serde_json::to_value(workspace).unwrap()).unwrap();
    assert!(!reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened, first_migration);
}

/// Fixture 图片阶段始终本地执行，不存在外部 wire body 歧义。
#[test]
fn migrates_v3_legacy_fixture_image_binding_without_wire_ambiguity() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = 3;
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "task-fixture-via-v3".into(),
        package_version: 1,
        prompt_renderer_version: LEGACY_PROMPT_RENDERER_VERSION,
        image_request_contract: None,
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: fixture_provider("fixture-via-v3"),
            credential_ref: None,
        }),
    });

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(
        workspace.execution_bindings[0].image_request_contract,
        Some(ImageRequestContract::ExplicitBase64V1)
    );
    assert_eq!(
        workspace.execution_bindings[0]
            .image_provider
            .as_ref()
            .unwrap()
            .config
            .kind,
        ProviderKind::Fixture
    );
    let first_migration = workspace.clone();
    let mut reopened: WorkspaceData =
        serde_json::from_value(serde_json::to_value(workspace).unwrap()).unwrap();
    assert!(!reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened, first_migration);
}

/// 已写出的 v3 新绑定保持省略字段和旧幂等键，避免恢复时改变请求身份。
#[test]
fn migrates_unversioned_v3_image_contract_without_changing_its_wire_shape() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = 3;
    let provider = real_provider("legacy-v3");
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "task-v3".into(),
        package_version: 1,
        prompt_renderer_version: V3_NEW_BINDING_PROMPT_RENDERER_VERSION,
        image_request_contract: None,
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("credential-v3".into()),
        }),
    });

    assert!(workspace.migrate_to_current_schema().unwrap());
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(
        workspace.execution_bindings[0].image_request_contract,
        Some(ImageRequestContract::ProviderDefaultBase64Unversioned)
    );
    assert_eq!(
        workspace.execution_bindings[0]
            .image_request_contract
            .unwrap()
            .idempotency_material(),
        ""
    );
}

/// v3 的未知 renderer 无法证明历史请求体，必须拒绝猜测契约。
#[test]
fn rejects_ambiguous_v3_image_contract() {
    let mut workspace = WorkspaceData::default();
    workspace.schema_version = 3;
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "ambiguous-v3".into(),
        package_version: 1,
        prompt_renderer_version: 99,
        image_request_contract: None,
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: None,
    });

    assert_eq!(
        workspace.migrate_to_current_schema(),
        Err("旧工作区的图片请求契约无法判定".into())
    );
    assert_eq!(workspace.schema_version, 3);
}

/// 当前 v4 绑定缺少执行语义字段时失败关闭，不能猜测一个 wire 契约。
#[test]
fn rejects_v4_binding_without_an_image_request_contract() {
    let mut workspace = WorkspaceData::default();
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "broken-v4".into(),
        package_version: 1,
        prompt_renderer_version: LEGACY_PROMPT_RENDERER_VERSION,
        image_request_contract: None,
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: None,
    });

    assert_eq!(
        workspace.migrate_to_current_schema(),
        Err("当前工作区缺少图片请求契约".into())
    );
}

/// 当前 v4 绑定缺少 renderer 时不得静默降级为首版渲染算法。
#[test]
fn rejects_v4_binding_without_a_prompt_renderer_contract() {
    let mut workspace = WorkspaceData::default();
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "broken-renderer-v4".into(),
        package_version: 1,
        prompt_renderer_version: 0,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: None,
    });

    assert_eq!(
        workspace.migrate_to_current_schema(),
        Err("当前工作区缺少 Prompt 渲染契约".into())
    );
    assert_eq!(workspace.schema_version, WORKSPACE_SCHEMA_VERSION);
}
