use super::*;

/// 被动列表只读取原子持久化的凭据引用，不解密系统凭据库。
#[test]
fn provider_views_report_persisted_credential_registration() {
    let mut registered = ProviderConfig::open_ai_compatible_draft();
    registered.id = "provider-registered".into();
    registered.name = "已登记".into();
    let mut missing = ProviderConfig::open_ai_compatible_draft();
    missing.id = "provider-missing".into();
    missing.name = "未登记".into();
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs = vec![registered, missing];
    workspace.provider_credential_refs.insert(
        "provider-registered".into(),
        "provider-registered:revision:1".into(),
    );

    let views = provider_config_views(&workspace);

    assert_eq!(views.len(), 2);
    assert!(
        views
            .iter()
            .find(|provider| provider.config.id == "provider-registered")
            .unwrap()
            .has_api_key
    );
    assert!(
        !views
            .iter()
            .find(|provider| provider.config.id == "provider-missing")
            .unwrap()
            .has_api_key
    );
}

/// 切换默认 Provider 时，目标和被挤下默认位的配置都必须保留旧快照并提升修订。
#[test]
fn provider_default_rebinding_versions_every_changed_config() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs = vec![
        ProviderConfig {
            id: "first".into(),
            name: "first".into(),
            revision: 3,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://first.example/v1".into(),
            text_model: "text".into(),
            article_model: Some("first-article".into()),
            summary_model: Some("first-summary".into()),
            image_model: "image".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: true,
            default_for_summary: true,
            default_for_image: true,
        },
        ProviderConfig {
            id: "second".into(),
            name: "second".into(),
            revision: 5,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://second.example/v1".into(),
            text_model: "text".into(),
            article_model: Some("second-article".into()),
            summary_model: Some("second-summary".into()),
            image_model: "image".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: false,
            default_for_summary: false,
            default_for_image: false,
        },
    ];
    let requested = ProviderConfig {
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
        ..workspace.provider_configs[1].clone()
    };

    let (saved, credential_ref, _) =
        apply_provider_config_update(&mut workspace, requested, None).unwrap();

    assert_eq!(saved.revision, 6);
    assert_eq!(credential_ref, "second");
    assert_eq!(workspace.provider_configs[0].revision, 4);
    assert!(!workspace.provider_configs[0].default_for_article);
    assert!(!workspace.provider_configs[0].default_for_summary);
    assert!(!workspace.provider_configs[0].default_for_image);
    assert_eq!(workspace.provider_configs[1].revision, 6);
    assert!(workspace.provider_configs[1].default_for_article);
    assert!(workspace.provider_configs[1].default_for_summary);
    assert!(workspace.provider_configs[1].default_for_image);
    assert_eq!(workspace.provider_config_history.len(), 2);
    assert!(workspace.provider_config_history.iter().any(|provider| {
        provider.id == "first"
            && provider.revision == 3
            && provider.default_for_article
            && provider.default_for_summary
            && provider.default_for_image
    }));
    assert!(workspace.provider_config_history.iter().any(|provider| {
        provider.id == "second"
            && provider.revision == 5
            && !provider.default_for_article
            && !provider.default_for_summary
            && !provider.default_for_image
    }));
}

/// 已有服务保存必须匹配当前修订，旧步骤草稿不能覆盖并发更新。
#[test]
fn provider_update_rejects_a_stale_target_revision_without_mutation() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs = vec![ProviderConfig {
        id: "provider".into(),
        name: "current".into(),
        revision: 5,
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
        default_for_article: false,
        default_for_summary: false,
        default_for_image: false,
    }];
    let before = workspace.clone();
    let requested = ProviderConfig {
        name: "stale edit".into(),
        revision: 4,
        ..workspace.provider_configs[0].clone()
    };

    assert_eq!(
        apply_provider_config_update(&mut workspace, requested, None),
        Err("PROVIDER_REVISION_CONFLICT".into())
    );
    assert_eq!(workspace, before);
}

/// 抢占某一个步骤的默认位不会误清同一配置的其他步骤能力。
#[test]
fn provider_default_rebinding_is_independent_per_stage() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs = vec![
        ProviderConfig {
            id: "first".into(),
            name: "first".into(),
            revision: 1,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://first.example/v1".into(),
            text_model: "text".into(),
            article_model: Some("first-article".into()),
            summary_model: Some("first-summary".into()),
            image_model: "image".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: true,
            default_for_summary: true,
            default_for_image: true,
        },
        ProviderConfig {
            id: "second".into(),
            name: "second".into(),
            revision: 1,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://second.example/v1".into(),
            text_model: "text".into(),
            article_model: Some("second-article".into()),
            summary_model: Some("second-summary".into()),
            image_model: "image".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: false,
            default_for_summary: false,
            default_for_image: false,
        },
    ];
    let requested = ProviderConfig {
        default_for_summary: true,
        ..workspace.provider_configs[1].clone()
    };

    apply_provider_config_update(&mut workspace, requested, None).unwrap();

    let first = &workspace.provider_configs[0];
    assert!(first.default_for_article);
    assert!(!first.default_for_summary);
    assert!(first.default_for_image);
    let second = &workspace.provider_configs[1];
    assert!(!second.default_for_article);
    assert!(second.default_for_summary);
    assert!(!second.default_for_image);
}

/// 任一受影响配置达到修订上限时必须原子拒绝，不能覆盖同 revision 的冻结语义。
#[test]
fn provider_revision_exhaustion_leaves_workspace_unchanged() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs = vec![
        ProviderConfig {
            id: "current-default".into(),
            name: "current-default".into(),
            revision: u32::MAX,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://first.example/v1".into(),
            text_model: "text".into(),
            article_model: Some("article".into()),
            summary_model: Some("summary".into()),
            image_model: "image".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: true,
            default_for_summary: false,
            default_for_image: false,
        },
        ProviderConfig {
            id: "requested".into(),
            name: "requested".into(),
            revision: 7,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://second.example/v1".into(),
            text_model: "text".into(),
            article_model: Some("article".into()),
            summary_model: Some("summary".into()),
            image_model: "image".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: false,
            default_for_summary: false,
            default_for_image: false,
        },
    ];
    let before = workspace.clone();
    let requested = ProviderConfig {
        default_for_article: true,
        ..workspace.provider_configs[1].clone()
    };

    assert_eq!(
        apply_provider_config_update(&mut workspace, requested, None),
        Err("PROVIDER_REVISION_EXHAUSTED".into())
    );
    assert_eq!(workspace, before);

    let mut target_workspace = WorkspaceData {
        provider_configs: vec![ProviderConfig {
            revision: u32::MAX,
            default_for_article: false,
            ..before.provider_configs[1].clone()
        }],
        ..WorkspaceData::default()
    };
    let target_before = target_workspace.clone();
    let target_request = ProviderConfig {
        name: "changed".into(),
        ..target_workspace.provider_configs[0].clone()
    };
    assert_eq!(
        apply_provider_config_update(&mut target_workspace, target_request, None),
        Err("PROVIDER_REVISION_EXHAUSTED".into())
    );
    assert_eq!(target_workspace, target_before);
}

/// 文章包仍引用旧凭据时必须阻止配置更新后的凭据回收。
#[test]
fn historical_execution_binding_retains_referenced_credential() {
    let workspace = WorkspaceData::default();
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
    let binding = PackageExecutionBinding {
        task_id: "task".into(),
        package_version: 1,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: Some(ProviderExecutionBinding {
            config: provider,
            credential_ref: Some("credential-old".into()),
        }),
        summary_provider: None,
        image_provider: None,
    };

    assert!(execution_binding_uses_credential(
        &binding,
        "credential-old"
    ));
    assert!(!execution_binding_uses_credential(
        &binding,
        "credential-new"
    ));
}

/// 密钥轮换原子激活新引用，并只把已经没有冻结绑定的旧引用放入清理队列。
#[test]
fn provider_rotation_queues_only_an_unreferenced_superseded_credential() {
    let mut workspace = WorkspaceData::default();
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
    workspace.provider_configs.push(provider.clone());
    workspace
        .provider_credential_refs
        .insert(provider.id.clone(), "credential-old".into());
    workspace
        .pending_credential_deletions
        .insert("credential-new".into());
    let requested = ProviderConfig {
        name: "provider revised".into(),
        ..provider.clone()
    };

    let (_, active, superseded) =
        apply_provider_config_update(&mut workspace, requested, Some("credential-new")).unwrap();

    assert_eq!(active, "credential-new");
    assert_eq!(superseded.as_deref(), Some("credential-old"));
    assert!(
        !workspace
            .pending_credential_deletions
            .contains("credential-new")
    );
    assert!(
        workspace
            .pending_credential_deletions
            .contains("credential-old")
    );

    let mut bound_workspace = WorkspaceData::default();
    bound_workspace.provider_configs.push(provider.clone());
    bound_workspace
        .provider_credential_refs
        .insert(provider.id.clone(), "credential-old".into());
    bound_workspace
        .pending_credential_deletions
        .insert("credential-new".into());
    let templates = bound_workspace.prompt_templates.clone();
    bound_workspace
        .execution_bindings
        .push(PackageExecutionBinding {
            task_id: "task".into(),
            package_version: 1,
            prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
            image_request_contract: Some(ImageRequestContract::CURRENT),
            article_template: templates[0].clone(),
            summary_template: templates[1].clone(),
            image_template: templates[2].clone(),
            article_provider: Some(ProviderExecutionBinding {
                config: provider.clone(),
                credential_ref: Some("credential-old".into()),
            }),
            summary_provider: None,
            image_provider: None,
        });
    apply_provider_config_update(
        &mut bound_workspace,
        ProviderConfig {
            name: "provider revised".into(),
            ..provider
        },
        Some("credential-new"),
    )
    .unwrap();
    assert!(
        !bound_workspace
            .pending_credential_deletions
            .contains("credential-old")
    );
}

/// 地址变化只对确实持有旧密钥的配置强制轮换。
#[test]
fn provider_transport_change_never_reuses_an_old_host_secret() {
    let previous = ProviderConfig {
        id: "provider".into(),
        name: "provider".into(),
        revision: 1,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: "https://old.example/v1".into(),
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
    let equivalent = provider_endpoint("https://old.example/v1/", "chat/completions").unwrap();
    let new_host = provider_endpoint("https://new.example/v1", "chat/completions").unwrap();
    let new_path = provider_endpoint("https://old.example/v2", "chat/completions").unwrap();
    let equivalent_config = previous.clone();
    let new_host_config = ProviderConfig {
        base_url: "https://new.example/v1".into(),
        ..previous.clone()
    };
    let new_path_config = ProviderConfig {
        base_url: "https://old.example/v2".into(),
        ..previous.clone()
    };

    assert!(!provider_transport_changed(
        &previous,
        &equivalent_config,
        Some(&equivalent)
    ));
    assert!(provider_transport_changed(
        &previous,
        &new_host_config,
        Some(&new_host)
    ));
    assert!(provider_transport_changed(
        &previous,
        &new_path_config,
        Some(&new_path)
    ));
    assert!(!endpoint_change_requires_key_rotation(
        &previous,
        &new_host_config,
        Some(&new_host),
        false
    ));
    assert!(endpoint_change_requires_key_rotation(
        &previous,
        &new_host_config,
        Some(&new_host),
        true
    ));
}

/// 方舟配置只接受已确认的北京 API v3 根地址，并以图片端点作为密钥轮换边界。
#[test]
fn validates_volcengine_ark_primary_endpoint_without_accepting_full_path() {
    let ark = ProviderConfig::volcengine_ark_image_draft();
    let endpoint = provider_primary_endpoint(&ark).unwrap().unwrap();
    assert_eq!(
        endpoint.as_str(),
        "https://ark.cn-beijing.volces.com/api/v3/images/generations"
    );

    let full_endpoint = ProviderConfig {
        base_url: "https://ark.cn-beijing.volces.com/api/v3/images/generations".into(),
        ..ark.clone()
    };
    assert!(provider_primary_endpoint(&full_endpoint).is_err());
    let wrong_host = ProviderConfig {
        base_url: "https://example.com/api/v3".into(),
        ..ark.clone()
    };
    assert!(provider_primary_endpoint(&wrong_host).is_err());

    let open_ai = ProviderConfig {
        id: "provider-existing".into(),
        ..ProviderConfig::open_ai_compatible_draft()
    };
    assert!(provider_transport_changed(&open_ai, &ark, Some(&endpoint)));
    assert!(endpoint_change_requires_key_rotation(
        &open_ai,
        &ark,
        Some(&endpoint),
        true
    ));
}

/// 钥匙串删除失败后引用跨仓储重开仍在队列，后续成功会幂等清除。
#[tokio::test]
async fn credential_cleanup_failure_is_retried_after_repository_reopen() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-credential-outbox-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());
    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    first
        .update(|workspace| {
            workspace
                .pending_credential_deletions
                .insert("credential-orphan".into());
            Ok(())
        })
        .await
        .unwrap();
    let mut failed_attempts = 0;
    reconcile_pending_credential_deletions_with(&first, |credential_ref| {
        assert_eq!(credential_ref, "credential-orphan");
        failed_attempts += 1;
        Err("simulated keychain outage".into())
    })
    .await;
    assert_eq!(failed_attempts, 1);
    assert!(
        first
            .snapshot()
            .await
            .pending_credential_deletions
            .contains("credential-orphan")
    );
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let mut successful_attempts = 0;
    reconcile_pending_credential_deletions_with(&second, |credential_ref| {
        assert_eq!(credential_ref, "credential-orphan");
        successful_attempts += 1;
        Ok(())
    })
    .await;
    assert_eq!(successful_attempts, 1);
    assert!(
        second
            .snapshot()
            .await
            .pending_credential_deletions
            .is_empty()
    );
    drop(second);
    tokio::fs::remove_dir_all(directory).await.unwrap();
}
