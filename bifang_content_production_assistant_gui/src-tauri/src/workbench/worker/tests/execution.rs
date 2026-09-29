use super::*;

/// 仅执行本地 Format 时，缺少不相关 Provider 或凭据不得阻断快照与转换。
#[tokio::test]
async fn format_stage_does_not_require_ai_provider_or_credentials() {
    let workspace = WorkspaceData::default();
    let snapshot = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |_| panic!("没有 Provider 时不应读取凭据"),
    )
    .unwrap();
    assert!(snapshot.article_provider.is_none());
    assert!(snapshot.summary_provider.is_none());
    assert!(snapshot.image_provider.is_none());
    let mut task = ArticleTask::new("task", "标题", 0);
    task.artifacts.markdown = Some("# 正文".into());
    task.stages.article.artifact_version = Some(1);
    let client = OpenAiCompatibleClient::new().unwrap();
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);
    let result = execute_stage(
        &task,
        WorkflowStage::Format,
        &snapshot,
        &client,
        Path::new("unused"),
        &throttle,
        "format-attempt".into(),
        &paused,
        &cancelled,
    )
    .await
    .unwrap();

    assert!(matches!(result.0, ArtifactOutput::Format(ref html) if html.contains("<h1>")));
    assert_eq!(result.2, Some(TokenUsage::local_processing()));
}

/// 缺少阶段服务时在建立会话前失败，不可能进入 Provider 网络边界。
#[tokio::test]
async fn missing_stage_provider_fails_before_any_outbound_call() {
    let workspace = WorkspaceData::default();
    let snapshot = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |_| panic!("没有 Provider 时不应读取凭据"),
    )
    .unwrap();
    let task = ArticleTask::new("task", "标题", 0);
    let client = OpenAiCompatibleClient::new().unwrap();
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);

    let error = execute_stage(
        &task,
        WorkflowStage::Article,
        &snapshot,
        &client,
        Path::new("unused"),
        &throttle,
        "article-attempt".into(),
        &paused,
        &cancelled,
    )
    .await
    .unwrap_err();

    assert_eq!(error.error.kind, ProviderErrorKind::NotConfigured);
    assert_eq!(error.error.safe_message, "当前步骤尚未选择 AI 服务");
}

/// 合法但只含引用定义的 Markdown 不能以空 HTML 成功，必须成为可隔离的行级失败。
#[tokio::test]
async fn empty_rendered_format_returns_stage_error() {
    let workspace = WorkspaceData::default();
    let snapshot = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |_| panic!("本地格式转换不应读取凭据"),
    )
    .unwrap();
    let mut task = ArticleTask::new("task", "标题", 0);
    task.artifacts.markdown = Some("[ref]: https://example.com".into());
    task.stages.article.artifact_version = Some(1);
    let client = OpenAiCompatibleClient::new().unwrap();
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);

    let error = execute_stage(
        &task,
        WorkflowStage::Format,
        &snapshot,
        &client,
        Path::new("unused"),
        &throttle,
        "format-attempt".into(),
        &paused,
        &cancelled,
    )
    .await
    .unwrap_err();

    assert_eq!(error.error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.error.safe_message.is_empty());
}

/// Markdown 转换只产生字符串，不由后端执行其中 HTML 或脚本。
#[test]
fn markdown_renderer_produces_inert_html_string() {
    let mut output = String::new();
    html::push_html(
        &mut output,
        Parser::new_ext("# 标题\n\n正文", Options::all()),
    );
    assert!(output.contains("<h1>标题</h1>"));
    assert!(output.contains("<p>正文</p>"));
}

/// 测试配图明确标识夹具来源且不会产生网络 URL。
#[test]
fn fixture_image_is_inline_and_visibly_labelled() {
    let url = fixture_image_data_url("测试标题");
    assert!(url.starts_with("data:image/svg+xml;base64,"));
    let encoded = url.split_once(',').unwrap().1;
    let decoded = String::from_utf8(BASE64.decode(encoded).unwrap()).unwrap();
    assert!(decoded.contains("E2E 测试夹具 · 非真实 AI"));
    assert!(decoded.contains("测试标题"));
}

/// 首版与 v3 开发快照复用各自历史幂等身份，新 wire 契约必须得到新键。
#[test]
fn image_request_contract_partitions_idempotency_keys() {
    let task = ArticleTask::new("task-1", "标题", 0);
    let key_for = |contract| {
        task.stage_idempotency_key(
            WorkflowStage::Image,
            "provider-fixture",
            1,
            "",
            "template",
            1,
            "{{title}} {{summary}}",
            "model",
            "标题 简介",
            &image_request_idempotency_material("", contract),
        )
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

/// 从含真实密钥值的内存快照生成绑定后，SQLite 只持久化凭据引用。
#[tokio::test]
async fn sqlite_execution_binding_never_persists_api_key_values() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-secret-boundary-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());
    let repository = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let mut workspace = WorkspaceData::default();
    let mut provider = fixture_provider();
    provider.kind = ProviderKind::OpenAiCompatible;
    provider.base_url = "https://example.com/v1".into();
    provider.id = "provider-secret-boundary".into();
    workspace.provider_configs = vec![provider];
    workspace.provider_credential_refs.insert(
        "provider-secret-boundary".into(),
        "credential-secret-boundary".into(),
    );
    let snapshot = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |reference| {
            assert_eq!(reference, "credential-secret-boundary");
            Ok("sentinel-api-key-must-never-reach-sqlite".into())
        },
    )
    .unwrap();
    let binding = snapshot.to_binding("task-secret-boundary", 1);

    repository
        .update(|data| {
            data.execution_bindings.push(binding);
            Ok(())
        })
        .await
        .unwrap();
    let raw_payload = repository.raw_snapshot_payload().await;

    assert!(raw_payload.contains("credential-secret-boundary"));
    assert!(!raw_payload.contains("sentinel-api-key-must-never-reach-sqlite"));
    assert!(!raw_payload.contains("apiKey"));
    assert!(!raw_payload.contains("api_key"));
    drop(repository);
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

/// 真实 HTTP Provider 的部分 usage 经 worker 成功路径原样持久化到阶段尝试。
#[tokio::test]
async fn worker_persists_exact_partial_provider_usage() {
    const TASK_ID: &str = "task-provider-usage";
    const BATCH_ID: &str = "batch-provider-usage";
    let provider = local_open_ai_provider(serve_text_usage_response());
    let provider_id = provider.id.clone();
    let mut workspace = WorkspaceData::default();
    workspace
        .provider_credential_refs
        .insert(provider_id, "credential-provider-usage".into());
    workspace.provider_configs.push(provider);
    let execution = ExecutionSnapshot::from_workspace_for_stage_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        Some(WorkflowStage::Article),
        |reference| {
            assert_eq!(reference, "credential-provider-usage");
            Ok("sensitive-test-key".into())
        },
    )
    .unwrap();
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new(TASK_ID, "用量持久化", 1);
            article
                .queue_single_stage(WorkflowStage::Article, 2)
                .unwrap();
            workspace.articles.push(article);
            workspace.run_history.push(RunRecord {
                id: BATCH_ID.into(),
                task_ids: vec![TASK_ID.into()],
                target_stage: Some(WorkflowStage::Article),
                status: RunStatus::Queued,
                started_at_ms: 2,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: false,
            });
            Ok(())
        })
        .await
        .unwrap();
    let client = OpenAiCompatibleClient::new().unwrap();
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);

    run_batch_inner(
        &repository,
        BATCH_ID,
        &[TASK_ID.into()],
        &BTreeMap::from([(TASK_ID.into(), execution)]),
        &client,
        Path::new("unused"),
        &throttle,
        &paused,
        &cancelled,
        Some(WorkflowStage::Article),
    )
    .await
    .unwrap();

    let snapshot = repository.snapshot().await;
    let attempt = snapshot.articles[0]
        .attempt_history
        .iter()
        .find(|attempt| attempt.stage == WorkflowStage::Article)
        .unwrap();
    assert_eq!(
        attempt.status,
        bifang_content_production_assistant_core::StageAttemptStatus::Succeeded
    );
    assert_eq!(
        attempt.token_usage,
        Some(TokenUsage {
            input_tokens: Some(17),
            output_tokens: None,
            total_tokens: Some(31),
        })
    );
    assert_eq!(snapshot.run_history[0].status, RunStatus::Succeeded);
}

/// 非法图片的失败 attempt 保留 Provider usage，且不会创建任何资产文件。
#[tokio::test]
async fn worker_persists_usage_when_image_validation_fails() {
    const TASK_ID: &str = "task-invalid-image";
    const BATCH_ID: &str = "batch-invalid-image";
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-invalid-image-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();

    let provider = local_open_ai_provider(serve_invalid_image_response());
    let provider_id = provider.id.clone();
    let mut workspace = WorkspaceData::default();
    workspace
        .provider_credential_refs
        .insert(provider_id.clone(), "credential-invalid-image".into());
    workspace.provider_configs.push(provider);
    let execution = ExecutionSnapshot::from_workspace_for_stage_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        Some(WorkflowStage::Image),
        |reference| {
            assert_eq!(reference, "credential-invalid-image");
            Ok("sensitive-test-key".into())
        },
    )
    .unwrap();
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new(TASK_ID, "配图", 1);
            article.artifacts.markdown = Some("# 正文".into());
            article.artifacts.summary = Some("本地测试简介".into());
            article.stages.article.status = StageStatus::Succeeded;
            article.stages.article.artifact_version = Some(1);
            article.stages.summary.status = StageStatus::Succeeded;
            article.stages.summary.artifact_version = Some(1);
            article.queue_single_stage(WorkflowStage::Image, 2).unwrap();
            workspace.articles.push(article);
            workspace.run_history.push(RunRecord {
                id: BATCH_ID.into(),
                task_ids: vec![TASK_ID.into()],
                target_stage: Some(WorkflowStage::Image),
                status: RunStatus::Queued,
                started_at_ms: 2,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: false,
            });
            Ok(())
        })
        .await
        .unwrap();
    let client = OpenAiCompatibleClient::new().unwrap();
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);

    run_batch_inner(
        &repository,
        BATCH_ID,
        &[TASK_ID.into()],
        &BTreeMap::from([(TASK_ID.into(), execution)]),
        &client,
        &directory,
        &throttle,
        &paused,
        &cancelled,
        Some(WorkflowStage::Image),
    )
    .await
    .unwrap();

    let snapshot = repository.snapshot().await;
    let article = &snapshot.articles[0];
    let attempt = article
        .attempt_history
        .iter()
        .find(|attempt| attempt.stage == WorkflowStage::Image)
        .unwrap();
    assert_eq!(article.stages.image.status, StageStatus::Failed);
    assert_eq!(
        attempt.status,
        bifang_content_production_assistant_core::StageAttemptStatus::Failed
    );
    assert_eq!(
        attempt.token_usage,
        Some(TokenUsage {
            input_tokens: None,
            output_tokens: Some(23),
            total_tokens: Some(29),
        })
    );
    assert_eq!(snapshot.run_history[0].status, RunStatus::PartialFailure);
    let mut entries = tokio::fs::read_dir(&directory).await.unwrap();
    assert!(entries.next_entry().await.unwrap().is_none());
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

/// 多行恢复时按任务 ID 选择各自快照，不会把首行绑定复用于其他文章。
#[test]
fn task_execution_map_keeps_each_package_binding_separate() {
    let workspace = WorkspaceData::default();
    let mut first = ExecutionSnapshot::from_workspace_with_loader(
        &workspace,
        ProviderKind::OpenAiCompatible,
        |_| panic!("无 Provider 时不应读取凭据"),
    )
    .unwrap();
    let mut second = first.clone();
    first.article_template.content = "第一行 {{title}}".into();
    second.article_template.content = "第二行 {{title}}".into();
    let executions = BTreeMap::from([
        ("task-a".to_string(), first),
        ("task-b".to_string(), second),
    ]);

    assert_eq!(
        executions["task-a"].article_template.content,
        "第一行 {{title}}"
    );
    assert_eq!(
        executions["task-b"].article_template.content,
        "第二行 {{title}}"
    );
}

/// 单步 worker 成功后立即收口，不会领取同一行的下一个 SOP 阶段。
#[tokio::test]
async fn stage_worker_stops_after_exact_target() {
    let mut fixture_workspace = WorkspaceData::default();
    fixture_workspace.provider_configs.push(fixture_provider());
    let execution = ExecutionSnapshot::from_workspace_for_stage_with_loader(
        &fixture_workspace,
        ProviderKind::Fixture,
        Some(WorkflowStage::Article),
        |_| panic!("夹具不应读取凭据"),
    )
    .unwrap();
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("single", "单步正文", 1);
            article
                .queue_single_stage(WorkflowStage::Article, 2)
                .unwrap();
            workspace.articles.push(article);
            workspace.run_history.push(RunRecord {
                id: "batch-single".into(),
                task_ids: vec!["single".into()],
                target_stage: Some(WorkflowStage::Article),
                status: RunStatus::Queued,
                started_at_ms: 2,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: true,
            });
            Ok(())
        })
        .await
        .unwrap();

    run_fixture_stage_inner(
        &repository,
        "batch-single",
        "single",
        &execution,
        WorkflowStage::Article,
    )
    .await
    .unwrap();

    let snapshot = repository.snapshot().await;
    let article = &snapshot.articles[0];
    assert_eq!(article.stages.article.status, StageStatus::Succeeded);
    assert_eq!(article.stages.summary.status, StageStatus::NotStarted);
    assert_eq!(article.overall_status(), OverallStatus::Draft);
    assert_eq!(snapshot.run_history[0].status, RunStatus::Succeeded);
    assert_eq!(snapshot.run_history[0].completed_count, 1);
    assert_eq!(
        snapshot.run_history[0].target_stage,
        Some(WorkflowStage::Article)
    );
}

/// worker 隔离单行图片失败，并在精确重试时只增加失败阶段的尝试次数。
#[tokio::test]
async fn batch_worker_isolates_partial_failure_and_retries_only_failed_stage() {
    const FAILED_ID: &str = "task-failed";
    const SUCCEEDED_ID: &str = "task-succeeded";
    let task_ids = vec![FAILED_ID.to_string(), SUCCEEDED_ID.to_string()];
    let mut fixture_workspace = WorkspaceData::default();
    fixture_workspace.provider_configs.push(fixture_provider());
    let execution = ExecutionSnapshot::from_workspace_with_loader(
        &fixture_workspace,
        ProviderKind::Fixture,
        |_| panic!("夹具不应读取凭据"),
    )
    .unwrap();
    let executions = BTreeMap::from([
        (FAILED_ID.to_string(), execution.clone()),
        (SUCCEEDED_ID.to_string(), execution.clone()),
    ]);
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut failed = ArticleTask::new(FAILED_ID, "图片失败一次", 1);
            failed.queue_initial(2).unwrap();
            let mut succeeded = ArticleTask::new(SUCCEEDED_ID, "正常文章", 1);
            succeeded.queue_initial(2).unwrap();
            workspace.articles.extend([failed, succeeded]);
            workspace.run_history.push(RunRecord {
                id: "batch-initial".into(),
                task_ids: task_ids.clone(),
                target_stage: None,
                status: RunStatus::Queued,
                started_at_ms: 1,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: true,
            });
            Ok(())
        })
        .await
        .unwrap();

    run_fixture_batch_inner(&repository, "batch-initial", &task_ids, &executions)
        .await
        .unwrap();

    let snapshot = repository.snapshot().await;
    let failed = snapshot
        .articles
        .iter()
        .find(|article| article.id == FAILED_ID)
        .unwrap();
    let succeeded = snapshot
        .articles
        .iter()
        .find(|article| article.id == SUCCEEDED_ID)
        .unwrap();
    assert_eq!(failed.overall_status(), OverallStatus::PartialFailure);
    assert_eq!(failed.stages.article.status, StageStatus::Succeeded);
    assert_eq!(failed.stages.summary.status, StageStatus::Succeeded);
    assert_eq!(failed.stages.image.status, StageStatus::Failed);
    assert!(
        WorkflowStage::ordered()
            .into_iter()
            .all(|stage| succeeded.stages.get(stage).status == StageStatus::Succeeded)
    );
    assert_eq!(succeeded.overall_status(), OverallStatus::PendingReview);
    let format_attempt = succeeded
        .attempt_history
        .iter()
        .find(|attempt| attempt.stage == WorkflowStage::Format)
        .unwrap();
    assert_eq!(
        format_attempt.token_usage,
        Some(TokenUsage::local_processing())
    );
    let article_attempts = failed.stages.article.attempts;
    let summary_attempts = failed.stages.summary.attempts;
    assert_eq!(article_attempts, 1);
    assert_eq!(summary_attempts, 1);
    assert_eq!(failed.stages.image.attempts, 1);
    assert_eq!(
        failed
            .artifacts
            .summary_lineage
            .as_ref()
            .and_then(|lineage| lineage.source_article_version),
        Some(failed.package_version)
    );
    assert_eq!(
        succeeded
            .artifacts
            .image_lineage
            .as_ref()
            .and_then(|lineage| lineage.source_article_version),
        Some(succeeded.package_version)
    );
    assert_eq!(
        succeeded
            .artifacts
            .image_lineage
            .as_ref()
            .and_then(|lineage| lineage.source_summary_version),
        Some(succeeded.package_version)
    );
    assert_eq!(
        succeeded
            .artifacts
            .format_lineage
            .as_ref()
            .and_then(|lineage| lineage.source_article_version),
        Some(succeeded.package_version)
    );
    let initial_run = snapshot
        .run_history
        .iter()
        .find(|run| run.id == "batch-initial")
        .unwrap();
    assert_eq!(initial_run.status, RunStatus::PartialFailure);
    assert_eq!(initial_run.completed_count, 1);
    assert_eq!(initial_run.failed_count, 1);
    drop(snapshot);

    let retry_task_ids = repository
        .update(|workspace| {
            let preparation = prepare_retry_batch(
                &mut workspace.articles,
                [FAILED_ID.to_string()],
                current_time_ms(),
            );
            if preparation.queued_task_ids != [FAILED_ID.to_string()] {
                return Err("精确重试没有只选中失败行".into());
            }
            workspace.run_history.push(RunRecord {
                id: "batch-retry".into(),
                task_ids: preparation.queued_task_ids.clone(),
                target_stage: None,
                status: RunStatus::Queued,
                started_at_ms: current_time_ms(),
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: true,
            });
            Ok(preparation.queued_task_ids)
        })
        .await
        .unwrap();
    let retry_executions = BTreeMap::from([(FAILED_ID.to_string(), execution.clone())]);

    run_fixture_batch_inner(
        &repository,
        "batch-retry",
        &retry_task_ids,
        &retry_executions,
    )
    .await
    .unwrap();

    let snapshot = repository.snapshot().await;
    let retried = snapshot
        .articles
        .iter()
        .find(|article| article.id == FAILED_ID)
        .unwrap();
    assert_eq!(retried.stages.article.attempts, article_attempts);
    assert_eq!(retried.stages.summary.attempts, summary_attempts);
    assert_eq!(retried.stages.image.attempts, 2);
    assert_eq!(retried.stages.format.attempts, 1);
    assert_eq!(
        retried
            .artifacts
            .image_lineage
            .as_ref()
            .and_then(|lineage| lineage.source_article_version),
        retried.stages.article.artifact_version
    );
    assert_eq!(
        retried
            .artifacts
            .image_lineage
            .as_ref()
            .and_then(|lineage| lineage.source_summary_version),
        retried.stages.summary.artifact_version
    );
    assert_eq!(retried.overall_status(), OverallStatus::PendingReview);
    let retry_run = snapshot
        .run_history
        .iter()
        .find(|run| run.id == "batch-retry")
        .unwrap();
    assert_eq!(retry_run.status, RunStatus::Succeeded);
    assert_eq!(retry_run.completed_count, 1);
    assert_eq!(retry_run.failed_count, 0);
}
