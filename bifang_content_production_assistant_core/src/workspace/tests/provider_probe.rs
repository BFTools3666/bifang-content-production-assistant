use super::*;

#[test]
fn text_probe_selects_the_saved_stage_and_minimal_request_without_a_key() {
    let mut workspace = WorkspaceData::default();
    let mut provider = real_provider("text-service");
    provider.article_parameters = Some(TextGenerationParameters {
        temperature_milli: Some(700),
        max_output_tokens: Some(800),
        timeout_ms: 70_000,
    });
    workspace.provider_configs.push(provider);
    workspace
        .provider_credential_refs
        .insert("text-service".into(), "keychain-reference".into());

    let TextProbeSelection::Request(article) =
        select_text_probe(&workspace, "text-service", WorkflowStage::Article, false).unwrap()
    else {
        panic!("expected a real text probe");
    };
    assert_eq!(article.model, "article");
    assert_eq!(article.credential_ref, "keychain-reference");
    assert_eq!(article.prompt, "请只回复：连接成功");
    assert_eq!(article.parameters.temperature_milli, None);
    assert_eq!(article.parameters.max_output_tokens, None);
    assert_eq!(article.parameters.timeout_ms, 30_000);
    assert_eq!(
        article.request_with_nonce("once").idempotency_key,
        "provider-test:text-service:once"
    );
    let TextProbeSelection::Request(summary) =
        select_text_probe(&workspace, "text-service", WorkflowStage::Summary, false).unwrap()
    else {
        panic!("expected a real summary probe");
    };
    assert_eq!(summary.model, "summary");
    assert_eq!(
        select_text_probe(&workspace, "text-service", WorkflowStage::Image, false),
        Err(ProviderProbeError::TextStageRequired)
    );
    workspace.provider_configs[0].enabled = false;
    assert_eq!(
        select_text_probe(&workspace, "text-service", WorkflowStage::Article, false),
        Err(ProviderProbeError::ProviderDisabled)
    );
    workspace.provider_configs[0].enabled = true;
    workspace.provider_configs[0].article_model = Some(String::new());
    assert_eq!(
        select_text_probe(&workspace, "text-service", WorkflowStage::Article, false),
        Err(ProviderProbeError::TextModelRequired)
    );
    workspace.provider_configs.push(fixture_provider("fixture"));
    assert_eq!(
        select_text_probe(&workspace, "fixture", WorkflowStage::Article, false),
        Err(ProviderProbeError::FixtureDisabled)
    );
    assert_eq!(
        select_text_probe(&workspace, "fixture", WorkflowStage::Article, true),
        Ok(TextProbeSelection::Fixture)
    );
}

#[test]
fn image_probe_requires_confirmation_and_current_saved_parameters() {
    let mut workspace = WorkspaceData::default();
    let mut provider = ProviderConfig::volcengine_ark_image_draft();
    provider.id = "ark".into();
    workspace.provider_configs.push(provider);
    assert_eq!(
        select_image_probe(&workspace, "ark", 1, false),
        Err(ProviderProbeError::ImageConfirmationRequired)
    );
    assert_eq!(
        select_image_probe(&workspace, "ark", 2, true),
        Err(ProviderProbeError::ProviderRevisionConflict)
    );
    let plan = select_image_probe(&workspace, "ark", 1, true).unwrap();
    assert_eq!(plan.credential_ref, "ark");
    assert_eq!(plan.parameters.size, "2K");
    assert!(!plan.prompt.is_empty());
    assert_eq!(
        plan.request_with_nonce("once").idempotency_key,
        "provider-image-test:ark:1:once"
    );
    workspace.provider_configs[0].enabled = false;
    assert_eq!(
        select_image_probe(&workspace, "ark", 1, true),
        Err(ProviderProbeError::ProviderDisabled)
    );
    workspace.provider_configs[0].enabled = true;
    workspace.provider_configs[0].image_model.clear();
    assert_eq!(
        select_image_probe(&workspace, "ark", 1, true),
        Err(ProviderProbeError::ImageModelRequired)
    );
    workspace.provider_configs[0].image_model = "ep-image".into();
    workspace.provider_configs[0].image_parameters = Some(ImageGenerationParameters {
        size: "1024x1024".into(),
        watermark: false,
        timeout_ms: 30_000,
    });
    assert_eq!(
        select_image_probe(&workspace, "ark", 1, true),
        Err(ProviderProbeError::ImageParametersInvalid)
    );
}

#[test]
fn ark_probe_uses_one_enabled_default_and_rejects_pending_credentials() {
    let mut workspace = WorkspaceData::default();
    let mut first = ProviderConfig::volcengine_ark_image_draft();
    first.id = "first".into();
    first.default_for_image = false;
    let mut second = ProviderConfig::volcengine_ark_image_draft();
    second.id = "second".into();
    second.default_for_image = false;
    workspace.provider_configs = vec![first, second];
    assert_eq!(
        select_ark_probe_provider(&workspace, None),
        Err(ProviderProbeError::ArkProviderAmbiguous)
    );
    workspace.provider_configs[1].default_for_image = true;
    let plan = select_ark_probe_provider(&workspace, None).unwrap();
    assert_eq!(plan.provider.id, "second");
    assert_eq!(plan.parameters.size, "2K");
    assert_eq!(
        plan.request_with_nonce("once").idempotency_key,
        "ark-image-probe:once"
    );
    assert_eq!(
        require_ark_probe_paid_confirmation(false),
        Err(ProviderProbeError::ArkPaidConfirmationRequired)
    );
    assert!(require_ark_probe_paid_confirmation(true).is_ok());
    assert_eq!(
        plan.prompt,
        "一张极简测试图片：白色背景中央只有一个蓝色圆形，画面清晰。"
    );
    workspace
        .provider_credential_refs
        .insert("second".into(), "shared-secret".into());
    workspace
        .pending_credential_deletions
        .insert("shared-secret".into());
    let plan = select_ark_probe_provider(&workspace, None).unwrap();
    assert_eq!(plan.credential_ref, "shared-secret");
    assert_eq!(
        ensure_ark_probe_credential_ready(&workspace, &plan),
        Err(ProviderProbeError::ArkCredentialPendingDeletion)
    );
    workspace.provider_configs[0].default_for_image = true;
    assert_eq!(
        select_ark_probe_provider(&workspace, None),
        Err(ProviderProbeError::ArkDefaultAmbiguous)
    );
    assert_eq!(
        select_ark_probe_provider(&workspace, Some("missing")),
        Err(ProviderProbeError::ArkProviderNotFound)
    );
}
