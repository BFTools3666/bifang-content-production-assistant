use super::*;

/// 新批次选择最新 Prompt、已启用默认服务和明确的凭据引用。
#[test]
fn execution_plan_selects_current_non_secret_inputs() {
    let mut workspace = WorkspaceData::default();
    let mut revised = workspace.prompt_templates[0].clone();
    revised.version = 7;
    revised.content = "新版 {{title}}".into();
    workspace.prompt_templates.push(revised);
    let mut provider = real_provider("provider-current");
    provider.revision = 4;
    workspace.provider_configs.push(provider);
    workspace
        .provider_credential_refs
        .insert("provider-current".into(), "credential-current-v4".into());

    let plan = ExecutionPlan::from_workspace(&workspace, ProviderKind::OpenAiCompatible, None)
        .expect("当前配置应可冻结");
    assert_eq!(plan.article_template.version, 7);
    assert_eq!(plan.article_template.content, "新版 {{title}}");
    assert_eq!(plan.article_provider.as_ref().unwrap().config.revision, 4);
    assert_eq!(
        plan.article_provider
            .as_ref()
            .unwrap()
            .credential_ref
            .as_deref(),
        Some("credential-current-v4")
    );
    assert_eq!(plan.validate_for_new_run(), Ok(()));
}

/// 单阶段不读取其他阶段 Provider；格式阶段不要求任何 AI 配置。
#[test]
fn execution_plan_keeps_stage_and_fixture_boundaries() {
    let mut workspace = WorkspaceData::default();
    let mut disabled = fixture_provider("disabled");
    disabled.enabled = false;
    workspace.provider_configs.push(disabled);
    let missing = ExecutionPlan::from_workspace(&workspace, ProviderKind::Fixture, None).unwrap();
    assert!(missing.validate_for_new_run().is_err());
    assert_eq!(missing.validate_for_stage(WorkflowStage::Format), Ok(()));
    workspace.provider_configs.push(fixture_provider("fixture"));

    let summary = ExecutionPlan::from_workspace(
        &workspace,
        ProviderKind::Fixture,
        Some(WorkflowStage::Summary),
    )
    .unwrap();
    assert!(summary.article_provider.is_none());
    assert!(summary.image_provider.is_none());
    assert_eq!(summary.summary_provider.unwrap().credential_ref, None);
    let real_mode = ExecutionPlan::from_workspace(
        &workspace,
        ProviderKind::OpenAiCompatible,
        Some(WorkflowStage::Summary),
    )
    .unwrap();
    assert!(real_mode.summary_provider.is_none());
}

/// 历史绑定不漂移到新配置，并且拒绝跨运行模式或缺失 wire 契约。
#[test]
fn execution_plan_restores_historical_binding_without_drift() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(real_provider("historic"));
    let frozen =
        ExecutionPlan::from_workspace(&workspace, ProviderKind::OpenAiCompatible, None).unwrap();
    let binding = PackageExecutionBinding {
        task_id: "task".into(),
        package_version: 2,
        prompt_renderer_version: frozen.prompt_renderer_version,
        image_request_contract: Some(frozen.image_request_contract),
        article_template: frozen.article_template.clone(),
        summary_template: frozen.summary_template.clone(),
        image_template: frozen.image_template.clone(),
        article_provider: frozen.article_provider.clone(),
        summary_provider: frozen.summary_provider.clone(),
        image_provider: frozen.image_provider.clone(),
    };
    workspace.provider_configs[0].revision = 9;
    let restored = ExecutionPlan::from_binding(&binding, ProviderKind::OpenAiCompatible).unwrap();
    assert_eq!(restored.article_provider.unwrap().config.revision, 3);
    assert!(ExecutionPlan::from_binding(&binding, ProviderKind::Fixture).is_err());
    let mut incomplete = binding;
    incomplete.image_request_contract = None;
    assert!(ExecutionPlan::from_binding(&incomplete, ProviderKind::OpenAiCompatible).is_err());
}

/// 图片 wire 契约和 Prompt renderer 的旧版兼容范围由 Core 固定。
#[test]
fn execution_plan_preserves_protocol_and_renderer_contract() {
    let mut workspace = WorkspaceData::default();
    let mut ark = real_provider("ark");
    ark.kind = ProviderKind::VolcengineArkImage;
    ark.default_for_article = false;
    ark.default_for_summary = false;
    ark.default_for_image = true;
    workspace.provider_configs.push(ark);
    let mut plan = ExecutionPlan::from_workspace(
        &workspace,
        ProviderKind::OpenAiCompatible,
        Some(WorkflowStage::Image),
    )
    .unwrap();
    assert_eq!(
        plan.image_request_contract,
        ImageRequestContract::VolcengineArkUrlV1
    );
    assert_eq!(plan.validate_for_stage(WorkflowStage::Image), Ok(()));
    assert_eq!(
        plan.use_prompt_renderer_version(LEGACY_PROMPT_RENDERER_VERSION),
        Ok(())
    );
    assert!(plan.use_prompt_renderer_version(999).is_err());
    assert_eq!(plan.prompt_renderer_version, LEGACY_PROMPT_RENDERER_VERSION);
}

#[test]
fn new_run_preflight_rejects_template_provider_mode_and_missing_reference() {
    let mut workspace = WorkspaceData::default();
    workspace.prompt_templates.clear();
    assert!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible)
            .unwrap_err()
            .contains("缺少正文 Prompt 模板")
    );

    workspace.prompt_templates = crate::initial_prompt_templates();
    assert!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible)
            .unwrap_err()
            .contains("正文阶段尚未选择默认 AI 服务")
    );

    let mut provider = real_provider("real");
    provider.enabled = false;
    workspace.provider_configs.push(provider);
    assert!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible)
            .unwrap_err()
            .contains("已停用")
    );

    workspace.provider_configs[0].enabled = true;
    assert!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::Fixture)
            .unwrap_err()
            .contains("不支持当前阶段或运行模式")
    );
    assert!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible)
            .unwrap_err()
            .contains("尚未登记 API 密钥")
    );
    workspace
        .provider_credential_refs
        .insert("real".into(), " ".into());
    assert!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible)
            .unwrap_err()
            .contains("尚未登记 API 密钥")
    );
    workspace
        .provider_credential_refs
        .insert("real".into(), "vault-ref".into());
    assert_eq!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::OpenAiCompatible),
        Ok(())
    );
}

#[test]
fn fixture_preflight_never_requires_keychain_reference() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(fixture_provider("fixture"));
    assert_eq!(
        validate_execution_configuration_for_kind(&workspace, ProviderKind::Fixture),
        Ok(())
    );
}
