use super::*;

/// 图片单步生成必须绑定本次确认时看到的默认 Provider 修订。
#[test]
fn single_image_stage_requires_confirmation_and_matching_provider_revision() {
    let mut workspace = WorkspaceData::default();
    workspace
        .articles
        .push(bifang_content_production_assistant_core::ArticleTask::new(
            "task", "标题", 0,
        ));
    let mut provider = ProviderConfig::volcengine_ark_image_draft();
    provider.id = "image-provider".into();
    provider.revision = 7;
    workspace.provider_configs.push(provider);

    assert_eq!(
        validate_single_stage_request_inner(
            &workspace,
            "task",
            WorkflowStage::Image,
            1,
            1,
            false,
            Some("image-provider"),
            Some(7),
        )
        .unwrap_err()
        .code,
        "STAGE_IMAGE_CONFIRMATION_REQUIRED"
    );
    assert_eq!(
        validate_single_stage_request_inner(
            &workspace,
            "task",
            WorkflowStage::Image,
            1,
            1,
            true,
            Some("image-provider"),
            Some(6),
        )
        .unwrap_err()
        .code,
        "STAGE_PROVIDER_CHANGED"
    );
    assert!(
        validate_single_stage_request_inner(
            &workspace,
            "task",
            WorkflowStage::Image,
            1,
            1,
            true,
            Some("image-provider"),
            Some(7),
        )
        .is_ok()
    );
}

/// 行/包乐观锁与领域依赖分别保持稳定错误码。
#[test]
fn single_stage_preflight_maps_revision_and_dependency_errors() {
    let mut workspace = WorkspaceData::default();
    workspace
        .articles
        .push(bifang_content_production_assistant_core::ArticleTask::new(
            "task", "标题", 0,
        ));

    assert_eq!(
        validate_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Article,
            2,
            1,
            false,
            None,
            None,
        )
        .unwrap_err()
        .code,
        "ARTICLE_REVISION_CONFLICT"
    );
    assert_eq!(
        validate_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Summary,
            1,
            1,
            false,
            None,
            None,
        )
        .unwrap_err()
        .code,
        "DEPENDENCY_NOT_READY"
    );
    assert_eq!(
        validate_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Format,
            1,
            1,
            false,
            None,
            None,
        )
        .unwrap_err()
        .code,
        "DEPENDENCY_NOT_READY"
    );
}

/// 合并单步绑定只替换目标阶段，不覆盖同包其他冻结 Provider。
#[test]
fn single_stage_binding_merge_preserves_unrelated_stage_bindings() {
    let workspace = WorkspaceData::default();
    let template = |kind| {
        workspace
            .prompt_templates
            .iter()
            .find(|template| template.kind == kind)
            .unwrap()
            .clone()
    };
    let bound = |id: &str| ProviderExecutionBinding {
        config: ProviderConfig {
            id: id.into(),
            ..ProviderConfig::open_ai_compatible_draft()
        },
        credential_ref: Some(format!("credential-{id}")),
    };
    let mut current = PackageExecutionBinding {
        task_id: "task".into(),
        package_version: 1,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: template(
            bifang_content_production_assistant_core::PromptTemplateKind::Article,
        ),
        summary_template: template(
            bifang_content_production_assistant_core::PromptTemplateKind::Summary,
        ),
        image_template: template(
            bifang_content_production_assistant_core::PromptTemplateKind::Image,
        ),
        article_provider: Some(bound("article-old")),
        summary_provider: Some(bound("summary-old")),
        image_provider: Some(bound("image-old")),
    };
    current.prompt_renderer_version = LEGACY_PROMPT_RENDERER_VERSION;
    let mut proposed = current.clone();
    proposed.prompt_renderer_version = CURRENT_PROMPT_RENDERER_VERSION;
    proposed.article_provider = None;
    proposed.summary_provider = Some(bound("summary-new"));
    proposed.image_provider = None;

    merge_stage_execution_binding(&mut current, &proposed, WorkflowStage::Summary);

    assert_eq!(
        current.prompt_renderer_version,
        LEGACY_PROMPT_RENDERER_VERSION
    );
    assert_eq!(
        current.article_provider.as_ref().unwrap().config.id,
        "article-old"
    );
    assert_eq!(
        current.summary_provider.as_ref().unwrap().config.id,
        "summary-new"
    );
    assert_eq!(
        current.image_provider.as_ref().unwrap().config.id,
        "image-old"
    );

    let derived_summary = single_stage_execution_binding(
        Some(current.clone()),
        &proposed,
        WorkflowStage::Summary,
        false,
    );
    assert_eq!(
        derived_summary.article_provider.as_ref().unwrap().config.id,
        "article-old"
    );
    assert_eq!(
        derived_summary.summary_provider.as_ref().unwrap().config.id,
        "summary-new"
    );
    assert!(derived_summary.image_provider.is_none());

    let mut proposed_article = proposed.clone();
    proposed_article.article_provider = Some(bound("article-new"));
    proposed_article.summary_provider = None;
    let regenerated_article = single_stage_execution_binding(
        Some(current),
        &proposed_article,
        WorkflowStage::Article,
        false,
    );
    assert_eq!(
        regenerated_article.article_provider.unwrap().config.id,
        "article-new"
    );
    assert_eq!(
        regenerated_article.prompt_renderer_version,
        CURRENT_PROMPT_RENDERER_VERSION
    );
    assert!(regenerated_article.summary_provider.is_none());
    assert!(regenerated_article.image_provider.is_none());
}
