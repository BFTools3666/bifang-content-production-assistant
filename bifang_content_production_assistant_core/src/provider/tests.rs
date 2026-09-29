use super::*;

fn provider_config(id: &str, revision: u32) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        name: id.into(),
        revision,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: format!("https://{id}.example/v1"),
        text_model: "shared-text".into(),
        article_model: Some(format!("{id}-article")),
        summary_model: Some(format!("{id}-summary")),
        image_model: format!("{id}-image"),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: false,
        default_for_summary: false,
        default_for_image: false,
    }
}

/// 新表单的三个阶段默认位与生成参数只能来自 core 的同一份事实。
#[test]
fn creates_open_ai_compatible_draft_with_explicit_stage_defaults() {
    let draft = ProviderConfig::open_ai_compatible_draft();

    assert!(draft.id.is_empty());
    assert_eq!(draft.name, "OpenAI Compatible");
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.kind, ProviderKind::OpenAiCompatible);
    assert!(draft.enabled);
    assert_eq!(draft.base_url, "https://api.openai.com/v1");
    assert_eq!(draft.article_model(), "");
    assert_eq!(draft.summary_model(), "");
    assert!(draft.image_model.is_empty());
    assert_eq!(
        draft.article_parameters(),
        TextGenerationParameters::default()
    );
    assert_eq!(
        draft.summary_parameters(),
        TextGenerationParameters::default()
    );
    assert_eq!(
        draft.image_parameters(),
        ImageGenerationParameters::default()
    );
    assert!(draft.default_for_article);
    assert!(draft.default_for_summary);
    assert!(draft.default_for_image);
}

/// 火山方舟图片草稿只承担配图阶段，并预填已确认的端点、模型和请求参数。
#[test]
fn creates_volcengine_ark_image_draft_with_image_only_defaults() {
    let draft = ProviderConfig::volcengine_ark_image_draft();

    assert!(draft.id.is_empty());
    assert_eq!(draft.name, "火山方舟图片");
    assert_eq!(draft.revision, 1);
    assert_eq!(draft.kind, ProviderKind::VolcengineArkImage);
    assert!(draft.enabled);
    assert_eq!(draft.base_url, "https://ark.cn-beijing.volces.com/api/v3");
    assert_eq!(draft.article_model(), "");
    assert_eq!(draft.summary_model(), "");
    assert_eq!(draft.image_model, "ep-20260318093407-x5qm5");
    assert_eq!(
        draft.image_parameters(),
        ImageGenerationParameters {
            size: "2K".into(),
            watermark: true,
            timeout_ms: 90_000,
        }
    );
    assert!(!draft.default_for_article);
    assert!(!draft.default_for_summary);
    assert!(draft.default_for_image);
    assert_eq!(draft.validate_stage_configuration(), Ok(()));
}

/// 服务能力由 core 统一定义，图片专用服务不能被文本阶段误用。
#[test]
fn exposes_provider_kind_capabilities() {
    assert!(ProviderKind::OpenAiCompatible.supports(ProviderCapability::Text));
    assert!(ProviderKind::OpenAiCompatible.supports(ProviderCapability::Image));
    assert!(!ProviderKind::VolcengineArkImage.supports(ProviderCapability::Text));
    assert!(ProviderKind::VolcengineArkImage.supports(ProviderCapability::Image));
    assert!(ProviderKind::Fixture.supports(ProviderCapability::Text));
    assert!(ProviderKind::Fixture.supports(ProviderCapability::Image));
}

/// HTTP 鉴权、请求超时、限流和服务端错误必须得到不同稳定分类。
#[test]
fn maps_http_errors_without_response_payload() {
    let auth = ProviderError::from_http_status(401, None);
    let timeout = ProviderError::from_http_status(408, None);
    let rate = ProviderError::from_http_status(429, Some(2_000));
    let server = ProviderError::from_http_status(503, None);

    assert_eq!(auth.kind, ProviderErrorKind::Authentication);
    assert!(!auth.is_retryable());
    assert_eq!(timeout.kind, ProviderErrorKind::Timeout);
    assert!(timeout.is_retryable());
    assert_eq!(rate.kind, ProviderErrorKind::RateLimited);
    assert!(rate.is_retryable());
    assert_eq!(rate.retry_after_ms, Some(2_000));
    assert_eq!(server.kind, ProviderErrorKind::Server);
    assert!(server.is_retryable());
}

/// 稳定失败对象只包含安全文案和机器错误码。
#[test]
fn converts_provider_error_to_safe_workflow_failure() {
    let error = ProviderError {
        kind: ProviderErrorKind::RateLimited,
        safe_message: "AI 服务繁忙，请稍后重试".into(),
        retry_after_ms: Some(2_500),
    };
    let failure = error.as_failure();

    assert_eq!(failure.code, "PROVIDER_RATE_LIMITED");
    assert!(failure.retryable);
    assert_eq!(failure.message, "AI 服务繁忙，请稍后重试");
    assert_eq!(failure.retry_after_ms, Some(2_500));
}

/// 文本与图片参数都只接受首版公开的有界值域。
#[test]
fn validates_bounded_generation_parameters() {
    assert_eq!(TextGenerationParameters::default().validate(), Ok(()));
    assert_eq!(ImageGenerationParameters::default().validate(), Ok(()));
    for temperature_milli in [0, 2_000] {
        assert_eq!(
            TextGenerationParameters {
                temperature_milli: Some(temperature_milli),
                ..TextGenerationParameters::default()
            }
            .validate(),
            Ok(())
        );
    }
    for max_output_tokens in [1, 128_000] {
        assert_eq!(
            TextGenerationParameters {
                max_output_tokens: Some(max_output_tokens),
                ..TextGenerationParameters::default()
            }
            .validate(),
            Ok(())
        );
    }
    for timeout_ms in [1_000, 300_000] {
        assert_eq!(
            TextGenerationParameters {
                timeout_ms,
                ..TextGenerationParameters::default()
            }
            .validate(),
            Ok(())
        );
        assert_eq!(
            ImageGenerationParameters {
                timeout_ms,
                ..ImageGenerationParameters::default()
            }
            .validate(),
            Ok(())
        );
    }
    assert_eq!(
        TextGenerationParameters {
            temperature_milli: Some(2_001),
            ..TextGenerationParameters::default()
        }
        .validate(),
        Err(ProviderParameterError::TemperatureOutOfRange)
    );
    assert_eq!(
        TextGenerationParameters {
            max_output_tokens: Some(0),
            ..TextGenerationParameters::default()
        }
        .validate(),
        Err(ProviderParameterError::MaxOutputTokensOutOfRange)
    );
    assert_eq!(
        TextGenerationParameters {
            max_output_tokens: Some(128_001),
            ..TextGenerationParameters::default()
        }
        .validate(),
        Err(ProviderParameterError::MaxOutputTokensOutOfRange)
    );
    for timeout_ms in [999, 300_001] {
        assert_eq!(
            TextGenerationParameters {
                timeout_ms,
                ..TextGenerationParameters::default()
            }
            .validate(),
            Err(ProviderParameterError::TimeoutOutOfRange)
        );
        assert_eq!(
            ImageGenerationParameters {
                timeout_ms,
                ..ImageGenerationParameters::default()
            }
            .validate(),
            Err(ProviderParameterError::TimeoutOutOfRange)
        );
    }
    assert_eq!(
        ImageGenerationParameters {
            size: "4096x4096".into(),
            ..ImageGenerationParameters::default()
        }
        .validate(),
        Err(ProviderParameterError::UnsupportedImageSize)
    );
    assert_eq!(
        ImageGenerationParameters::volcengine_ark_default()
            .validate_for_provider_kind(ProviderKind::VolcengineArkImage),
        Ok(())
    );
    assert_eq!(
        ImageGenerationParameters::volcengine_ark_default()
            .validate_for_provider_kind(ProviderKind::OpenAiCompatible),
        Err(ProviderParameterError::UnsupportedImageSize)
    );
    assert_eq!(
        ImageGenerationParameters::default()
            .validate_for_provider_kind(ProviderKind::VolcengineArkImage),
        Err(ProviderParameterError::UnsupportedImageSize)
    );
    assert_eq!(
        ImageGenerationParameters {
            watermark: true,
            ..ImageGenerationParameters::default()
        }
        .validate_for_provider_kind(ProviderKind::OpenAiCompatible),
        Err(ProviderParameterError::UnsupportedImageWatermark)
    );
}

/// 物化旧配置默认值不会改变既有幂等键，显式参数变化才进入哈希材料。
#[test]
fn preserves_legacy_idempotency_material_for_default_parameters() {
    assert_eq!(
        TextGenerationParameters::default().idempotency_material(),
        ""
    );
    assert_eq!(
        ImageGenerationParameters::default().idempotency_material(),
        ""
    );
    assert_eq!(
        TextGenerationParameters {
            temperature_milli: Some(700),
            ..TextGenerationParameters::default()
        }
        .idempotency_material(),
        "temperatureMilli=700;maxOutputTokens=default;timeoutMs=60000"
    );
    assert_eq!(
        ImageGenerationParameters {
            size: "1024x1024".into(),
            ..ImageGenerationParameters::default()
        }
        .idempotency_material(),
        "size=1024x1024;timeoutMs=90000"
    );
    assert_eq!(
        ImageGenerationParameters::volcengine_ark_default().idempotency_material(),
        "size=2K;timeoutMs=90000;watermark=true"
    );
}

/// 迁移策略在 core 中判定 v1-v5，并幂等物化三阶段配置。
#[test]
fn migrates_known_provider_schemas_with_stable_stage_semantics() {
    let mut legacy = provider_config("legacy", 1);
    legacy.article_model = None;
    legacy.summary_model = None;
    legacy.article_parameters = None;
    legacy.summary_parameters = None;
    legacy.image_parameters = None;
    legacy.default_for_article = true;
    legacy.default_for_summary = false;

    let migration = ProviderConfigMigration::for_schema_version(1).unwrap();
    assert!(migration.migrate_config(&mut legacy));
    assert_eq!(migration.target_schema_version(), 5);
    assert!(legacy.enabled);
    assert_eq!(legacy.article_model(), "shared-text");
    assert_eq!(legacy.summary_model(), "shared-text");
    assert!(legacy.default_for_article);
    assert!(legacy.default_for_summary);
    assert_eq!(
        legacy.article_parameters(),
        TextGenerationParameters::default()
    );
    assert_eq!(
        legacy.image_parameters(),
        ImageGenerationParameters::default()
    );
    assert!(!migration.migrate_config(&mut legacy));

    assert_eq!(
        ProviderConfigMigration::for_schema_version(2),
        Ok(ProviderConfigMigration::FromSharedTextModel)
    );
    assert_eq!(
        ProviderConfigMigration::for_schema_version(3),
        Ok(ProviderConfigMigration::FromEnabledStatus)
    );
    assert_eq!(
        ProviderConfigMigration::for_schema_version(4),
        Ok(ProviderConfigMigration::FromImageProviderKind)
    );
    let mut explicitly_disabled = provider_config("disabled-v4", 1);
    explicitly_disabled.enabled = false;
    explicitly_disabled.default_for_article = false;
    explicitly_disabled.default_for_summary = false;
    explicitly_disabled.default_for_image = false;
    assert!(
        !ProviderConfigMigration::FromImageProviderKind.migrate_config(&mut explicitly_disabled)
    );
    assert!(!explicitly_disabled.enabled);
    assert_eq!(
        ProviderConfigMigration::for_schema_version(5),
        Ok(ProviderConfigMigration::Current)
    );
    assert_eq!(
        ProviderConfigMigration::for_schema_version(6),
        Err(ProviderConfigMigrationError::UnsupportedVersion(6))
    );
}

/// 模型、默认位与参数之间的跨字段约束由 core 统一拒绝。
#[test]
fn validates_provider_stage_cross_field_rules() {
    let mut config = provider_config("provider", 1);
    config.article_model = Some(String::new());
    config.summary_model = Some(String::new());
    config.image_model.clear();
    assert_eq!(
        config.validate_stage_configuration(),
        Err(ProviderConfigError::StageModelRequired)
    );

    config.default_for_article = true;
    config.image_model = "image".into();
    assert_eq!(
        config.validate_stage_configuration(),
        Err(ProviderConfigError::DefaultModelRequired)
    );

    config.default_for_article = false;
    config.article_parameters = Some(TextGenerationParameters {
        timeout_ms: 999,
        ..TextGenerationParameters::default()
    });
    assert_eq!(
        config.validate_stage_configuration(),
        Err(ProviderConfigError::InvalidParameter(
            ProviderParameterError::TimeoutOutOfRange
        ))
    );
    config.article_parameters = Some(TextGenerationParameters::default());
    assert_eq!(config.validate_stage_configuration(), Ok(()));

    let mut ark = ProviderConfig::volcengine_ark_image_draft();
    ark.article_model = Some("ep-text".into());
    assert_eq!(
        ark.validate_stage_configuration(),
        Err(ProviderConfigError::UnsupportedStage(
            ProviderCapability::Text
        ))
    );
    ark.article_model = Some(String::new());
    ark.image_parameters = Some(ImageGenerationParameters::default());
    assert_eq!(
        ark.validate_stage_configuration(),
        Err(ProviderConfigError::InvalidParameter(
            ProviderParameterError::UnsupportedImageSize
        ))
    );

    config.enabled = false;
    config.default_for_image = true;
    assert_eq!(
        config.validate_stage_configuration(),
        Err(ProviderConfigError::DisabledDefault)
    );
}

/// 公共更新用例自身拒绝无稳定身份或跨字段非法的配置，并保持集合不变。
#[test]
fn provider_update_validates_before_mutating_collections() {
    let original = provider_config("current", 2);
    let mut current = vec![original.clone()];
    let mut history = vec![provider_config("history", 1)];
    let current_before = current.clone();
    let history_before = history.clone();

    for invalid in [
        ProviderConfig {
            id: " ".into(),
            ..original.clone()
        },
        ProviderConfig {
            name: " ".into(),
            ..original.clone()
        },
        ProviderConfig {
            article_model: Some(String::new()),
            default_for_article: true,
            ..original.clone()
        },
    ] {
        assert!(matches!(
            apply_provider_config_update(&mut current, &mut history, invalid),
            Err(ProviderConfigUpdateError::InvalidConfig(_))
        ));
        assert_eq!(current, current_before);
        assert_eq!(history, history_before);
    }
}

/// 默认位切换必须同时修订目标与被挤下的配置，并把全部旧修订写入历史。
#[test]
fn atomically_revises_every_provider_changed_by_default_rebinding() {
    let mut first = provider_config("first", 3);
    first.default_for_article = true;
    first.default_for_summary = true;
    first.default_for_image = true;
    let second = provider_config("second", 5);
    let mut current = vec![first, second.clone()];
    let mut history = Vec::new();
    let requested = ProviderConfig {
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
        ..second
    };

    let update = apply_provider_config_update(&mut current, &mut history, requested).unwrap();

    assert!(update.replaced_existing);
    assert_eq!(update.saved.revision, 6);
    assert_eq!(current[0].revision, 4);
    assert!(!current[0].default_for_article);
    assert!(!current[0].default_for_summary);
    assert!(!current[0].default_for_image);
    assert_eq!(current[1], update.saved);
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].id, "first");
    assert_eq!(history[0].revision, 3);
    assert_eq!(history[1].id, "second");
    assert_eq!(history[1].revision, 5);
}

/// 抢占单一阶段默认位不能清除原配置仍持有的其他阶段默认位。
#[test]
fn revises_default_slots_independently_per_stage() {
    let mut first = provider_config("first", 1);
    first.default_for_article = true;
    first.default_for_summary = true;
    first.default_for_image = true;
    let second = provider_config("second", 1);
    let mut current = vec![first, second.clone()];
    let mut history = Vec::new();
    let requested = ProviderConfig {
        default_for_summary: true,
        ..second
    };

    apply_provider_config_update(&mut current, &mut history, requested).unwrap();

    assert!(current[0].default_for_article);
    assert!(!current[0].default_for_summary);
    assert!(current[0].default_for_image);
    assert!(!current[1].default_for_article);
    assert!(current[1].default_for_summary);
    assert!(!current[1].default_for_image);
}

/// 任一受影响修订达到上限时，当前配置和历史必须都保持不变。
#[test]
fn revision_exhaustion_rejects_default_rebinding_without_partial_mutation() {
    let mut first = provider_config("first", u32::MAX);
    first.default_for_summary = true;
    let second = provider_config("second", 7);
    let mut current = vec![first, second.clone()];
    let mut history = vec![provider_config("historical", 1)];
    let current_before = current.clone();
    let history_before = history.clone();
    let requested = ProviderConfig {
        default_for_summary: true,
        ..second
    };

    assert_eq!(
        apply_provider_config_update(&mut current, &mut history, requested),
        Err(ProviderConfigUpdateError::RevisionExhausted)
    );
    assert_eq!(current, current_before);
    assert_eq!(history, history_before);
}

/// 停用会建立修订并清空默认位；启用不恢复默认位，重复提交不再加修订。
#[test]
fn toggles_provider_enabled_state_with_idempotent_revisions() {
    let mut provider = provider_config("provider", 4);
    provider.default_for_article = true;
    provider.default_for_summary = true;
    provider.default_for_image = true;
    let mut current = vec![provider];
    let mut history = Vec::new();

    let disabled =
        set_provider_config_enabled(&mut current, &mut history, "provider", false).unwrap();
    assert_eq!(disabled.saved.revision, 5);
    assert!(!disabled.saved.enabled);
    assert!(!disabled.saved.default_for_article);
    assert!(!disabled.saved.default_for_summary);
    assert!(!disabled.saved.default_for_image);
    assert_eq!(history.len(), 1);

    let unchanged =
        set_provider_config_enabled(&mut current, &mut history, "provider", false).unwrap();
    assert_eq!(unchanged.saved.revision, 5);
    assert_eq!(history.len(), 1);

    let enabled =
        set_provider_config_enabled(&mut current, &mut history, "provider", true).unwrap();
    assert_eq!(enabled.saved.revision, 6);
    assert!(enabled.saved.enabled);
    assert!(!enabled.saved.default_for_article);
    assert!(!enabled.saved.default_for_summary);
    assert!(!enabled.saved.default_for_image);
    assert_eq!(history.len(), 2);

    assert_eq!(
        set_provider_config_enabled(&mut current, &mut history, "missing", true),
        Err(ProviderConfigUpdateError::NotFound)
    );
}

/// 步骤分配只替换目标步骤；移除后参数仍保留，重复移除不产生修订。
#[test]
fn assigns_and_removes_one_provider_stage_without_losing_configuration() {
    let mut first = provider_config("first", 3);
    first.default_for_article = true;
    first.default_for_summary = true;
    first.default_for_image = true;
    let second = provider_config("second", 7);
    let second_summary_model = second.summary_model().to_string();
    let second_summary_parameters = second.summary_parameters();
    let mut current = vec![first, second];
    let mut history = Vec::new();

    let assigned = set_provider_stage_assignment(
        &mut current,
        &mut history,
        "second",
        PromptTemplateKind::Summary,
        true,
    )
    .unwrap();
    assert_eq!(assigned.saved.revision, 8);
    assert!(current[0].default_for_article);
    assert!(!current[0].default_for_summary);
    assert!(current[0].default_for_image);
    assert!(!assigned.saved.default_for_article);
    assert!(assigned.saved.default_for_summary);
    assert!(!assigned.saved.default_for_image);

    let removed = set_provider_stage_assignment(
        &mut current,
        &mut history,
        "second",
        PromptTemplateKind::Summary,
        false,
    )
    .unwrap();
    assert_eq!(removed.saved.revision, 9);
    assert!(!removed.saved.default_for_summary);
    assert_eq!(removed.saved.summary_model(), second_summary_model);
    assert_eq!(
        removed.saved.summary_parameters(),
        second_summary_parameters
    );
    let history_len = history.len();

    let unchanged = set_provider_stage_assignment(
        &mut current,
        &mut history,
        "second",
        PromptTemplateKind::Summary,
        false,
    )
    .unwrap();
    assert_eq!(unchanged.saved.revision, 9);
    assert_eq!(history.len(), history_len);
}

/// 不可用、能力不符或缺少模型的服务不能分配，且失败不改变集合。
#[test]
fn rejects_invalid_stage_assignments_without_mutating_collections() {
    let mut disabled = provider_config("disabled", 2);
    disabled.enabled = false;
    let mut missing_model = provider_config("missing-model", 4);
    missing_model.article_model = Some(String::new());
    missing_model.text_model.clear();
    let mut ark = ProviderConfig::volcengine_ark_image_draft();
    ark.id = "ark".into();
    ark.default_for_image = false;
    let mut current = vec![disabled, missing_model, ark];
    let mut history = vec![provider_config("historical", 1)];
    let current_before = current.clone();
    let history_before = history.clone();

    assert!(matches!(
        set_provider_stage_assignment(
            &mut current,
            &mut history,
            "disabled",
            PromptTemplateKind::Article,
            true,
        ),
        Err(ProviderConfigUpdateError::InvalidConfig(
            ProviderConfigError::DisabledDefault
        ))
    ));
    assert!(matches!(
        set_provider_stage_assignment(
            &mut current,
            &mut history,
            "missing-model",
            PromptTemplateKind::Article,
            true,
        ),
        Err(ProviderConfigUpdateError::InvalidConfig(
            ProviderConfigError::DefaultModelRequired
        ))
    ));
    assert!(matches!(
        set_provider_stage_assignment(
            &mut current,
            &mut history,
            "ark",
            PromptTemplateKind::Article,
            true,
        ),
        Err(ProviderConfigUpdateError::InvalidConfig(
            ProviderConfigError::UnsupportedStage(ProviderCapability::Text)
        ))
    ));
    assert_eq!(current, current_before);
    assert_eq!(history, history_before);
}

/// 删除必须失败关闭；安全删除从当前集合移除并保留最终修订。
#[test]
fn deletes_only_unbound_non_default_provider() {
    let mut default_provider = provider_config("default", 2);
    default_provider.default_for_article = true;
    let removable = provider_config("removable", 7);
    let mut current = vec![default_provider, removable.clone()];
    let mut history = vec![provider_config("historical", 1)];
    let current_before = current.clone();
    let history_before = history.clone();

    assert_eq!(
        delete_provider_config(&mut current, &mut history, "default"),
        Err(ProviderConfigDeleteError::DefaultInUse)
    );
    assert_eq!(current, current_before);
    assert_eq!(history, history_before);

    let removed = delete_provider_config(&mut current, &mut history, "removable").unwrap();
    assert_eq!(removed, removable);
    assert_eq!(current.len(), 1);
    assert_eq!(history.last(), Some(&removed));
    assert_eq!(
        delete_provider_config(&mut current, &mut history, "removable"),
        Err(ProviderConfigDeleteError::NotFound)
    );
}
