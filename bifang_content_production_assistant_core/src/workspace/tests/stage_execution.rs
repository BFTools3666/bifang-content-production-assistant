use super::*;
use crate::ProviderErrorKind;

#[test]
fn provider_plan_freezes_prompt_model_parameters_and_source_versions() {
    let mut workspace = WorkspaceData::default();
    let mut provider = real_provider("provider");
    provider.image_parameters = Some(ImageGenerationParameters {
        size: "1024x1024".into(),
        watermark: false,
        timeout_ms: 90_000,
    });
    workspace.provider_configs.push(provider);
    let mut plan =
        ExecutionPlan::from_workspace(&workspace, ProviderKind::OpenAiCompatible, None).unwrap();
    plan.image_template.content = "{{title}} {{summary}}".into();
    plan.article_template.content = "{{title}} {{keywords}} {{requirements}}".into();
    let mut task = ArticleTask::new("task", "标题", 1);
    task.keywords = vec!["关键词一".into(), "关键词二".into()];
    task.requirements = "内容要求".into();
    task.artifacts.markdown = Some("# 正文".into());
    task.artifacts.summary = Some("简介".into());
    task.stages.article.artifact_version = Some(2);
    task.stages.summary.artifact_version = Some(3);

    let StageExecutionPlan::ProviderRequest {
        provider,
        model,
        prompt,
        parameters,
        idempotency_key,
        lineage,
    } = StageExecutionPlan::from_task(&task, WorkflowStage::Image, &plan, "image-attempt".into())
        .unwrap()
    else {
        panic!("图片阶段必须建立 Provider 请求");
    };
    assert_eq!(provider.id, "provider");
    assert_eq!(model, "image");
    assert!(prompt.contains("标题"));
    assert!(prompt.contains("简介"));
    assert!(
        matches!(parameters, StageGenerationParameters::Image(ref value) if value.size == "1024x1024")
    );
    assert!(idempotency_key.starts_with("gfs-"));
    assert_eq!(lineage.source_article_version, Some(2));
    assert_eq!(lineage.source_summary_version, Some(3));
    assert_eq!(
        lineage.template_id.as_deref(),
        Some(plan.image_template.id.as_str())
    );
    assert_eq!(lineage.provider_revision, Some(3));
    assert_eq!(lineage.model.as_deref(), Some("image"));
    assert_eq!(lineage.attempt_id, "image-attempt");

    let StageExecutionPlan::ProviderRequest {
        prompt, parameters, ..
    } = StageExecutionPlan::from_task(
        &task,
        WorkflowStage::Article,
        &plan,
        "article-attempt".into(),
    )
    .unwrap()
    else {
        panic!("正文阶段必须建立 Provider 请求");
    };
    assert!(prompt.contains("关键词一、关键词二"));
    assert!(prompt.contains("内容要求"));
    assert!(matches!(parameters, StageGenerationParameters::Text(_)));
}

#[test]
fn image_wire_contract_keeps_legacy_keys_and_partitions_current_requests() {
    let mut workspace = WorkspaceData::default();
    let mut provider = fixture_provider("provider-fixture");
    provider.image_model = "model".into();
    workspace.provider_configs.push(provider);
    let mut plan = ExecutionPlan::from_workspace(&workspace, ProviderKind::Fixture, None).unwrap();
    plan.image_template.id = "template".into();
    plan.image_template.version = 1;
    plan.image_template.content = "{{title}} {{summary}}".into();
    let mut task = ArticleTask::new("task-1", "标题", 0);
    task.artifacts.summary = Some("简介".into());
    let key_for = |contract| {
        let mut plan = plan.clone();
        plan.image_request_contract = contract;
        let StageExecutionPlan::ProviderRequest {
            idempotency_key, ..
        } = StageExecutionPlan::from_task(&task, WorkflowStage::Image, &plan, "attempt".into())
            .unwrap()
        else {
            panic!("图片阶段必须建立 Provider 请求");
        };
        idempotency_key
    };

    assert_eq!(
        key_for(ImageRequestContract::ExplicitBase64V1),
        "gfs-83a53cbec3b83482d56687bd35fa5ec0"
    );
    assert_eq!(
        key_for(ImageRequestContract::ProviderDefaultBase64Unversioned),
        "gfs-83a53cbec3b83482d56687bd35fa5ec0"
    );
    assert_eq!(
        key_for(ImageRequestContract::ProviderDefaultBase64V2),
        "gfs-2954fe368a28e4e90777c56c1fac6481"
    );
    assert_eq!(
        image_request_idempotency_material(
            "size=1024x1024;timeoutMs=90000",
            ImageRequestContract::CURRENT,
        ),
        "size=1024x1024;timeoutMs=90000;imageRequestContract=providerDefaultBase64V2"
    );
}

#[test]
fn local_format_requires_nonempty_html_and_no_provider() {
    let workspace = WorkspaceData::default();
    let plan =
        ExecutionPlan::from_workspace(&workspace, ProviderKind::OpenAiCompatible, None).unwrap();
    let mut task = ArticleTask::new("task", "标题", 1);
    task.stages.article.artifact_version = Some(1);
    assert_eq!(
        StageExecutionPlan::from_task(&task, WorkflowStage::Format, &plan, "attempt".into())
            .unwrap_err()
            .kind,
        ProviderErrorKind::InvalidResponse
    );
    task.artifacts.markdown = Some("[ref]: https://example.com".into());
    assert_eq!(
        StageExecutionPlan::from_task(&task, WorkflowStage::Format, &plan, "attempt".into())
            .unwrap_err()
            .safe_message,
        "Markdown 无法生成非空 HTML"
    );
    task.artifacts.markdown = Some("# 正文".into());
    let StageExecutionPlan::LocalFormat { html, lineage } =
        StageExecutionPlan::from_task(&task, WorkflowStage::Format, &plan, "attempt".into())
            .unwrap()
    else {
        panic!("Format 必须在本地完成");
    };
    assert!(html.contains("<h1>正文</h1>"));
    assert_eq!(lineage.source_article_version, Some(1));
    assert_eq!(lineage.provider_id, None);
    assert_eq!(lineage.template_id, None);
}

#[test]
fn missing_provider_fails_before_any_outbound_request() {
    let workspace = WorkspaceData::default();
    let plan =
        ExecutionPlan::from_workspace(&workspace, ProviderKind::OpenAiCompatible, None).unwrap();
    let task = ArticleTask::new("task", "标题", 1);
    let error =
        StageExecutionPlan::from_task(&task, WorkflowStage::Article, &plan, "attempt".into())
            .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::NotConfigured);
    assert_eq!(error.safe_message, "当前步骤尚未选择 AI 服务");
}
