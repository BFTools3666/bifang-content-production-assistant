use super::*;

/// 正式环境逻辑始终拒绝未显式启用的 Fixture 配置。
#[test]
fn fixture_mode_defaults_to_disabled() {
    if std::env::var("BIFANG_CONTENT_PRODUCTION_ASSISTANT_E2E_FIXTURE").is_err() {
        assert!(!fixture_mode_enabled());
    }
}

/// 图片测试没有本次付费确认时必须在任何凭据或网络边界前拒绝。
#[test]
fn image_test_requires_explicit_charge_confirmation() {
    assert_eq!(
        require_image_probe_confirmation(false).unwrap_err().code(),
        "PROVIDER_IMAGE_TEST_CONFIRMATION_REQUIRED"
    );
    assert!(require_image_probe_confirmation(true).is_ok());
}

/// 图片测试预检接受已启用方舟配置，并拒绝停用或缺少图片模型的配置。
#[test]
fn image_test_preflight_uses_one_saved_image_configuration() {
    let mut provider = ProviderConfig::volcengine_ark_image_draft();
    provider.id = "provider-ark".into();
    provider.name = "方舟图片".into();
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(provider.clone());
    let plan = select_image_probe(&workspace, &provider.id, 1, true).unwrap();
    assert!(validate_image_test_endpoint(&plan.provider).is_ok());
    assert_eq!(
        ImageRequestContract::for_provider_kind(provider.kind),
        ImageRequestContract::VolcengineArkUrlV1
    );
    let request = plan.request_with_nonce("test-nonce");
    assert_eq!(request.model, provider.image_model);
    assert_eq!(request.prompt, plan.prompt);
    assert_eq!(request.parameters, provider.image_parameters());
    assert_eq!(
        request.idempotency_key,
        "provider-image-test:provider-ark:1:test-nonce"
    );
    assert_eq!(
        select_image_probe(&workspace, &provider.id, 2, true)
            .unwrap_err()
            .code(),
        "PROVIDER_REVISION_CONFLICT"
    );

    provider.enabled = false;
    workspace.provider_configs[0] = provider.clone();
    assert_eq!(
        select_image_probe(&workspace, &provider.id, 1, true)
            .unwrap_err()
            .code(),
        "PROVIDER_DISABLED"
    );
    provider.enabled = true;
    provider.image_model.clear();
    workspace.provider_configs[0] = provider.clone();
    assert_eq!(
        select_image_probe(&workspace, &provider.id, 1, true)
            .unwrap_err()
            .code(),
        "PROVIDER_IMAGE_MODEL_REQUIRED"
    );
}

/// 图片结果不确定时不得沿用暗示可安全重试的通用错误。
#[test]
fn image_test_uncertain_failures_use_a_charge_safe_error() {
    for kind in [
        ProviderErrorKind::Timeout,
        ProviderErrorKind::Transport,
        ProviderErrorKind::Server,
        ProviderErrorKind::InvalidResponse,
        ProviderErrorKind::UnknownOutcome,
    ] {
        let error = bifang_content_production_assistant_core::ProviderError {
            kind,
            safe_message: "外部原始错误不得显示".into(),
            retry_after_ms: None,
        };
        assert_eq!(
            image_test_command_error(error).code,
            "PROVIDER_IMAGE_TEST_RESULT_UNCERTAIN"
        );
    }
}
