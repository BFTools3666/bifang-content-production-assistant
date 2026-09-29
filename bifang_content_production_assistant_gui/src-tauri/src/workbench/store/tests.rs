use bifang_content_production_assistant_core::{
    ArticleTask, ArtifactLineage, ArtifactOutput, CURRENT_PROMPT_RENDERER_VERSION,
    ImageGenerationParameters, ReviewStatus, StageStatus, TextGenerationParameters, WorkflowStage,
};

use super::*;
use crate::workbench::models::{
    ImageRequestContract, PackageExecutionBinding, ProviderConfig, ProviderExecutionBinding,
    ProviderKind, RunRecord, WORKSPACE_SCHEMA_VERSION,
};

/// 小红书首次启动和重启均不注入SEO种子，笔记提示词只初始化一次。
#[cfg(feature = "xhs")]
#[tokio::test]
async fn note_edition_keeps_note_defaults_after_restart() {
    let directory = std::env::temp_dir().join(format!("bifang-note-restart-{}", uuid::Uuid::new_v4()));
    let first = WorkspaceRepository::open(&directory).await.unwrap();
    let original = first.snapshot().await;
    assert!(original.articles.is_empty());
    assert!(original.prompt_templates.iter().any(|template| template.name == "小红书原创笔记"));
    drop(first);
    let second = WorkspaceRepository::open(&directory).await.unwrap();
    let reopened = second.snapshot().await;
    assert!(reopened.articles.is_empty());
    assert_eq!(reopened.prompt_templates, original.prompt_templates);
    drop(second);
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

/// 通过公开领域 API 建立一篇可批准文章，避免测试绕过状态机直接伪造成功状态。
fn completed_article(id: &str, title: &str) -> ArticleTask {
    let mut article = ArticleTask::new(id, title, 1);
    article.queue_initial(2).unwrap();
    for (index, output) in [
        ArtifactOutput::Article("# 正文".into()),
        ArtifactOutput::Summary("文章简介".into()),
        ArtifactOutput::Image("approved-image.png".into()),
        ArtifactOutput::Format("<h1>正文</h1>".into()),
    ]
    .into_iter()
    .enumerate()
    {
        let stage = output.stage();
        let started_at_ms = 3 + i64::try_from(index).unwrap() * 2;
        let attempt_id = article.start_stage(stage, started_at_ms).unwrap();
        let lineage = ArtifactLineage {
            package_version: article.package_version,
            source_article_version: (stage != WorkflowStage::Article)
                .then_some(article.package_version),
            source_summary_version: (stage == WorkflowStage::Image)
                .then_some(article.package_version),
            template_id: (stage != WorkflowStage::Format).then(|| format!("{stage:?}-template")),
            template_version: (stage != WorkflowStage::Format).then_some(1),
            provider_id: (stage != WorkflowStage::Format).then(|| "provider-1".into()),
            provider_revision: (stage != WorkflowStage::Format).then_some(1),
            model: (stage != WorkflowStage::Format).then(|| "model-1".into()),
            attempt_id,
        };
        article
            .complete_stage(stage, output, lineage, started_at_ms + 1)
            .unwrap();
    }
    article.approve(20).unwrap();
    article
}

/// SQLite 快照在仓储重开后保留任务、Prompt 与文章包冻结执行绑定。
#[tokio::test]
async fn sqlite_snapshot_survives_repository_reopen() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-store-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());

    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let binding = first
        .update(|data| {
            data.articles.push(ArticleTask::new("task-1", "标题", 1));
            let provider = ProviderConfig {
                id: "provider-1".into(),
                name: "服务".into(),
                revision: 3,
                kind: ProviderKind::OpenAiCompatible,
                enabled: true,
                base_url: "https://example.com/v1".into(),
                text_model: "text-v3".into(),
                article_model: Some("article-v3".into()),
                summary_model: Some("summary-v3".into()),
                image_model: "image-v3".into(),
                article_parameters: Some(TextGenerationParameters::default()),
                summary_parameters: Some(TextGenerationParameters::default()),
                image_parameters: Some(ImageGenerationParameters::default()),
                default_for_article: true,
                default_for_summary: true,
                default_for_image: true,
            };
            data.provider_configs.push(provider.clone());
            data.provider_credential_refs
                .insert(provider.id.clone(), "credential-r3".into());
            let bound_provider = ProviderExecutionBinding {
                config: provider,
                credential_ref: Some("credential-r3".into()),
            };
            let binding = PackageExecutionBinding {
                task_id: "task-1".into(),
                package_version: 1,
                prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
                image_request_contract: Some(
                    crate::workbench::models::ImageRequestContract::CURRENT,
                ),
                article_template: data.prompt_templates[0].clone(),
                summary_template: data.prompt_templates[1].clone(),
                image_template: data.prompt_templates[2].clone(),
                article_provider: Some(bound_provider.clone()),
                summary_provider: Some(bound_provider.clone()),
                image_provider: Some(bound_provider),
            };
            data.execution_bindings.push(binding.clone());
            Ok(binding)
        })
        .await
        .unwrap();
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let data = second.snapshot().await;
    assert_eq!(data.articles[0].title, "标题");
    assert_eq!(data.prompt_templates.len(), 3);
    assert_eq!(data.execution_bindings, vec![binding]);
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 普通应用入口会为 v4 空内容库加法写入 14 条种子，并保留已有 Provider。
#[tokio::test]
async fn production_open_seeds_v4_empty_content_without_replacing_provider() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-seed-v4-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());
    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let mut provider = serde_json::to_value(ProviderConfig {
        id: "saved-openai".into(),
        name: "我已保存的 OpenAI".into(),
        revision: 2,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: "https://api.openai.com/v1".into(),
        text_model: "gpt-4o-mini".into(),
        article_model: Some("gpt-4o-mini".into()),
        summary_model: Some("gpt-4o-mini".into()),
        image_model: "gpt-image-2.5-flare".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: false,
        default_for_summary: false,
        default_for_image: false,
    })
    .unwrap();
    provider.as_object_mut().unwrap().remove("enabled");
    let payload = serde_json::json!({
        "schemaVersion": 4,
        "articles": [],
        "promptTemplates": WorkspaceData::default().prompt_templates,
        "providerConfigs": [provider],
        "providerConfigHistory": [],
        "providerCredentialRefs": {},
        "pendingCredentialDeletions": [],
        "executionBindings": [],
        "runHistory": []
    })
    .to_string();
    first
        .database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE workspace_snapshot SET schema_version = 4, payload = ? WHERE id = 1",
            [payload.into()],
        ))
        .await
        .unwrap();
    drop(first);

    let seeded = WorkspaceRepository::open_database_url_with_initial_content(&url, true)
        .await
        .unwrap();
    let data = seeded.snapshot().await;
    assert_eq!(data.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(data.initial_data_revision, 1);
    assert_eq!(data.articles.len(), 14);
    assert_eq!(data.provider_configs.len(), 1);
    assert_eq!(data.provider_configs[0].id, "saved-openai");
    assert_eq!(data.provider_configs[0].name, "我已保存的 OpenAI");
    assert!(data.provider_configs[0].enabled);
    drop(seeded);

    let reopened = WorkspaceRepository::open_database_url_with_initial_content(&url, true)
        .await
        .unwrap();
    assert_eq!(reopened.snapshot().await.articles.len(), 14);
    reopened
        .update(|data| {
            data.articles.clear();
            Ok(())
        })
        .await
        .unwrap();
    drop(reopened);

    let after_delete = WorkspaceRepository::open_database_url_with_initial_content(&url, true)
        .await
        .unwrap();
    let data = after_delete.snapshot().await;
    assert!(data.articles.is_empty());
    assert_eq!(data.initial_data_revision, 1);
    assert_eq!(data.provider_configs[0].id, "saved-openai");
    drop(after_delete);
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 已批准版本在重启和后续新包排队/再次重启后仍保持逐字段不变。
#[tokio::test]
async fn approved_package_survives_reopen_and_later_batch() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-approved-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());

    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let approved = completed_article("task-approved", "获批标题");
    let approved_package = approved.approved_history[0].clone();
    first
        .update(|data| {
            data.articles.push(approved);
            Ok(())
        })
        .await
        .unwrap();
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let reopened = second.snapshot().await;
    assert_eq!(reopened.articles[0].review_status, ReviewStatus::Approved);
    assert_eq!(
        reopened.articles[0].approved_history,
        vec![approved_package.clone()]
    );
    second
        .update(|data| {
            let article = data
                .articles
                .iter_mut()
                .find(|article| article.id == "task-approved")
                .unwrap();
            article
                .regenerate_stage(WorkflowStage::Article, 30)
                .unwrap();
            Ok(())
        })
        .await
        .unwrap();
    drop(second);

    let third = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let persisted = third.snapshot().await;
    let article = &persisted.articles[0];
    assert_eq!(article.package_version, 2);
    assert_eq!(article.review_status, ReviewStatus::Draft);
    assert_eq!(article.approved_history, vec![approved_package.clone()]);
    assert_eq!(article.package_history.len(), 1);
    assert_eq!(article.package_history[0].version, approved_package.version);
    assert_eq!(article.package_history[0].title, approved_package.title);
    assert_eq!(
        article.package_history[0].keywords,
        approved_package.keywords
    );
    assert_eq!(
        article.package_history[0].requirements,
        approved_package.requirements
    );
    assert_eq!(
        article.package_history[0].review_status,
        ReviewStatus::Approved
    );
    assert_eq!(
        article.package_history[0].artifacts,
        approved_package.artifacts
    );
    drop(third);
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 仓储重开会迁移并持久保存 v1 的文本默认位，第二次重开无需重复迁移。
#[tokio::test]
async fn repository_persists_v1_provider_default_migration() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-v1-migrate-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());

    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let legacy_payload = serde_json::json!({
        "schemaVersion": 1,
        "articles": [],
        "promptTemplates": [],
        "providerConfigs": [{
            "id": "legacy",
            "name": "旧配置",
            "kind": "openAiCompatible",
            "baseUrl": "https://example.com/v1",
            "textModel": "legacy-text",
            "imageModel": "legacy-image",
            "defaultForText": true,
            "defaultForImage": true
        }],
        "runHistory": []
    })
    .to_string();
    first
        .database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE workspace_snapshot SET schema_version = 1, payload = ? WHERE id = 1",
            [legacy_payload.into()],
        ))
        .await
        .unwrap();
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let migrated = second.snapshot().await;
    assert_eq!(migrated.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert!(migrated.provider_configs[0].default_for_article);
    assert!(migrated.provider_configs[0].default_for_summary);
    assert!(migrated.provider_configs[0].default_for_image);
    assert_eq!(migrated.provider_configs[0].revision, 1);
    assert_eq!(migrated.provider_configs[0].article_model(), "legacy-text");
    assert_eq!(migrated.provider_configs[0].summary_model(), "legacy-text");
    assert_eq!(
        migrated.provider_configs[0].image_parameters(),
        ImageGenerationParameters::default()
    );
    let stored = second
        .database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT payload FROM workspace_snapshot WHERE id = 1".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let stored_payload: String = stored.try_get("", "payload").unwrap();
    assert!(stored_payload.contains("\"articleModel\":\"legacy-text\""));
    assert!(stored_payload.contains("\"summaryModel\":\"legacy-text\""));
    assert!(!stored_payload.contains("defaultForText"));
    drop(second);

    let third = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let persisted = third.snapshot().await;
    assert_eq!(persisted.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert!(persisted.provider_configs[0].default_for_article);
    assert!(persisted.provider_configs[0].default_for_summary);
    assert!(persisted.provider_configs[0].default_for_image);
    assert_eq!(persisted.provider_configs[0].summary_model(), "legacy-text");
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// c60 的文本-only 绑定经 Task 5 初版落成 v3 后，仓储仍可迁移、持久化并重开。
#[tokio::test]
async fn repository_reopens_v3_legacy_text_only_binding() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-v3-text-only-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());

    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let defaults = WorkspaceData::default();
    let provider = ProviderConfig {
        id: "legacy-text-only".into(),
        name: "旧文本服务".into(),
        revision: 2,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: "https://example.com/v1".into(),
        text_model: "legacy-text".into(),
        article_model: Some("legacy-text".into()),
        summary_model: Some("legacy-text".into()),
        image_model: "legacy-image".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: false,
    };
    let legacy_payload = serde_json::json!({
        "schemaVersion": 3,
        "articles": [],
        "promptTemplates": defaults.prompt_templates.clone(),
        "providerConfigs": [provider.clone()],
        "providerCredentialRefs": {"legacy-text-only": "credential-v2-r2"},
        "executionBindings": [{
            "taskId": "task-v2-via-v3",
            "packageVersion": 1,
            "promptRendererVersion": 1,
            "articleTemplate": defaults.prompt_templates[0],
            "summaryTemplate": defaults.prompt_templates[1],
            "imageTemplate": defaults.prompt_templates[2],
            "articleProvider": {"config": provider.clone(), "credentialRef": "credential-v2-r2"},
            "summaryProvider": {"config": provider, "credentialRef": "credential-v2-r2"},
            "imageProvider": null
        }],
        "runHistory": []
    })
    .to_string();
    first
        .database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE workspace_snapshot SET schema_version = 3, payload = ? WHERE id = 1",
            [legacy_payload.into()],
        ))
        .await
        .unwrap();
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let migrated = second.snapshot().await;
    assert_eq!(migrated.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(
        migrated.execution_bindings[0].image_request_contract,
        Some(crate::workbench::models::ImageRequestContract::ExplicitBase64V1)
    );
    assert!(migrated.execution_bindings[0].image_provider.is_none());
    let stored: serde_json::Value =
        serde_json::from_str(&second.raw_snapshot_payload().await).unwrap();
    assert_eq!(stored["schemaVersion"], WORKSPACE_SCHEMA_VERSION);
    assert_eq!(
        stored["executionBindings"][0]["imageRequestContract"],
        "explicitBase64V1"
    );
    drop(second);

    let third = WorkspaceRepository::open_database_url(&url).await.unwrap();
    assert_eq!(
        third.snapshot().await.execution_bindings,
        migrated.execution_bindings
    );
    drop(third);
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 重开仓储会把遗留运行中步骤恢复为可重试的中断状态。
#[tokio::test]
async fn reopen_recovers_running_stage_as_interrupted() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-recover-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());

    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    first
        .update(|data| {
            let mut article = ArticleTask::new("task-1", "标题", 1);
            article.queue_initial(2).unwrap();
            article
                .start_stage(
                    bifang_content_production_assistant_core::WorkflowStage::Article,
                    3,
                )
                .unwrap();
            data.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    assert_eq!(
        second.snapshot().await.articles[0].stages.article.status,
        StageStatus::Interrupted
    );
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 重启遇到已运行的方舟配图时不得允许普通重试再次触发付费 POST。
#[tokio::test]
async fn reopen_marks_running_ark_image_as_non_retryable_unknown_outcome() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-ark-recover-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());

    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    first
        .update(|data| {
            let mut article = ArticleTask::new("task-ark", "标题", 1);
            article.queue_initial(2).unwrap();
            for (time, output) in [
                (3, ArtifactOutput::Article("# 正文".into())),
                (5, ArtifactOutput::Summary("简介".into())),
            ] {
                let stage = output.stage();
                let attempt_id = article.start_stage(stage, time).unwrap();
                article
                    .complete_stage(
                        stage,
                        output,
                        ArtifactLineage {
                            package_version: article.package_version,
                            source_article_version: (stage != WorkflowStage::Article)
                                .then_some(article.package_version),
                            source_summary_version: None,
                            template_id: Some(format!("{stage:?}-template")),
                            template_version: Some(1),
                            provider_id: Some("text-provider".into()),
                            provider_revision: Some(1),
                            model: Some("text-model".into()),
                            attempt_id,
                        },
                        time + 1,
                    )
                    .unwrap();
            }
            article.start_stage(WorkflowStage::Image, 7).unwrap();

            let mut ark = ProviderConfig::volcengine_ark_image_draft();
            ark.id = "ark-image".into();
            let image_provider = ProviderExecutionBinding {
                config: ark,
                credential_ref: Some("credential-ark".into()),
            };
            data.execution_bindings.push(PackageExecutionBinding {
                task_id: article.id.clone(),
                package_version: article.package_version,
                prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
                image_request_contract: Some(ImageRequestContract::VolcengineArkUrlV1),
                article_template: data.prompt_templates[0].clone(),
                summary_template: data.prompt_templates[1].clone(),
                image_template: data.prompt_templates[2].clone(),
                article_provider: None,
                summary_provider: None,
                image_provider: Some(image_provider),
            });
            data.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let mut data = second.snapshot().await;
    let article = &mut data.articles[0];
    assert_eq!(article.stages.image.status, StageStatus::Interrupted);
    let failure = article.stages.image.failure.as_ref().unwrap();
    assert_eq!(failure.code, "RUN_OUTCOME_UNKNOWN");
    assert!(!failure.retryable);
    assert!(article.retry_failed(8).is_err());
    drop(second);
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 排队中的行和非终态批次在重启后都必须收口为可恢复失败。
#[tokio::test]
async fn reopen_recovers_queued_tasks_and_finishes_run_record() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-queued-recover-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let database = directory.join("workspace.sqlite3");
    let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());
    let first = WorkspaceRepository::open_database_url(&url).await.unwrap();
    first
        .update(|data| {
            let mut article = ArticleTask::new("task-queued", "标题", 1);
            article.queue_initial(2).unwrap();
            data.articles.push(article);
            data.run_history.push(RunRecord {
                id: "run-queued".into(),
                task_ids: vec!["task-queued".into()],
                target_stage: None,
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
    drop(first);

    let second = WorkspaceRepository::open_database_url(&url).await.unwrap();
    let data = second.snapshot().await;
    assert_eq!(
        data.articles[0].stages.article.status,
        StageStatus::Interrupted
    );
    assert_eq!(data.run_history[0].status, RunStatus::PartialFailure);
    assert!(data.run_history[0].finished_at_ms.is_some());
    assert_eq!(data.run_history[0].failed_count, 1);
    tokio::fs::remove_dir_all(&directory).await.unwrap();
}

/// 全部已取消的遗留批次保持取消终态，取消行不得误计为失败。
#[test]
fn abandoned_cancelled_run_does_not_count_failures() {
    let mut data = WorkspaceData::default();
    let mut article = ArticleTask::new("task-cancelled", "标题", 1);
    article.queue_initial(2).unwrap();
    assert!(article.cancel_pending(3));
    data.articles.push(article);
    data.run_history.push(RunRecord {
        id: "run-cancelled".into(),
        task_ids: vec!["task-cancelled".into()],
        target_stage: None,
        status: RunStatus::Paused,
        started_at_ms: 2,
        finished_at_ms: None,
        completed_count: 0,
        failed_count: 0,
        is_test_fixture: false,
    });

    assert_eq!(recover_abandoned_runs(&mut data, 4), 1);
    assert_eq!(data.run_history[0].status, RunStatus::Cancelled);
    assert_eq!(data.run_history[0].completed_count, 0);
    assert_eq!(data.run_history[0].failed_count, 0);
    assert_eq!(data.run_history[0].finished_at_ms, Some(4));
}

/// 部分完成、部分取消的重启恢复与运行期共用取消终态，且不伪造失败数。
#[test]
fn abandoned_mixed_success_and_cancelled_run_stays_cancelled() {
    let mut data = WorkspaceData::default();
    let mut completed = ArticleTask::new("task-completed", "已完成", 1);
    completed.review_status = ReviewStatus::Pending;
    let mut cancelled = ArticleTask::new("task-cancelled", "已取消", 1);
    cancelled.queue_initial(2).unwrap();
    assert!(cancelled.cancel_pending(3));
    data.articles.extend([completed, cancelled]);
    data.run_history.push(RunRecord {
        id: "run-mixed".into(),
        task_ids: vec!["task-completed".into(), "task-cancelled".into()],
        target_stage: None,
        status: RunStatus::Running,
        started_at_ms: 2,
        finished_at_ms: None,
        completed_count: 0,
        failed_count: 0,
        is_test_fixture: false,
    });

    assert_eq!(recover_abandoned_runs(&mut data, 4), 1);
    assert_eq!(data.run_history[0].status, RunStatus::Cancelled);
    assert_eq!(data.run_history[0].completed_count, 1);
    assert_eq!(data.run_history[0].failed_count, 0);
}

/// 重启恢复单步运行时只中断目标阶段，并按目标状态收口运行记录。
#[test]
fn abandoned_single_stage_run_recovers_only_its_target() {
    let mut data = WorkspaceData::default();
    let mut article = ArticleTask::new("task-single", "标题", 1);
    article.stages.article.status = StageStatus::Queued;
    article.stages.summary.status = StageStatus::Queued;
    data.articles.push(article);
    data.run_history.push(RunRecord {
        id: "run-single".into(),
        task_ids: vec!["task-single".into()],
        target_stage: Some(WorkflowStage::Article),
        status: RunStatus::Running,
        started_at_ms: 2,
        finished_at_ms: None,
        completed_count: 0,
        failed_count: 0,
        is_test_fixture: false,
    });

    assert_eq!(recover_interrupted_articles(&mut data, 3), 1);
    assert_eq!(
        data.articles[0].stages.article.status,
        StageStatus::Interrupted
    );
    assert_eq!(data.articles[0].stages.summary.status, StageStatus::Queued);
    assert_eq!(recover_abandoned_runs(&mut data, 4), 1);
    assert_eq!(data.run_history[0].status, RunStatus::PartialFailure);
    assert_eq!(data.run_history[0].failed_count, 1);
    assert_eq!(data.run_history[0].finished_at_ms, Some(4));
}

/// 更新闭包失败时实时内存保持原样，后续写入不会夹带半成品。
#[tokio::test]
async fn failed_update_rolls_back_in_memory_candidate() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    let result = repository
        .update(|data| {
            data.articles
                .push(ArticleTask::new("partial", "不应保存", 1));
            Err::<(), _>("模拟后续字段失败".into())
        })
        .await;

    assert!(result.is_err());
    assert!(repository.snapshot().await.articles.is_empty());
}

/// SQLite 写入失败时也不能先行污染内存权威状态。
#[tokio::test]
async fn persistence_failure_rolls_back_in_memory_candidate() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .database
        .execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TRIGGER reject_workspace_update BEFORE UPDATE ON workspace_snapshot \
                 BEGIN SELECT RAISE(FAIL, 'simulated persistence failure'); END"
                .to_string(),
        ))
        .await
        .unwrap();

    let result = repository
        .update(|data| {
            data.articles
                .push(ArticleTask::new("not-persisted", "不应可见", 1));
            Ok(())
        })
        .await;

    assert!(result.is_err());
    assert!(repository.snapshot().await.articles.is_empty());
}

/// SQLite 只持久化 CLI 发布目标非秘密配置，不出现 CLI token 或遗留 App Secret。
#[tokio::test]
async fn connector_snapshot_never_contains_app_secret() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|data| {
            data.connector_configs.push(
                bifang_content_production_assistant_core::ConnectorConfig {
                    id: "connector".into(),
                    name: "飞书".into(),
                    revision: 1,
                    kind: bifang_content_production_assistant_core::ConnectorKind::FeishuDrive,
                    enabled: true,
                    cli_profile: "default".into(),
                    folder_token: "fld_target".into(),
                    default_format:
                        bifang_content_production_assistant_core::DocumentFormat::Markdown,
                    is_default: true,
                    timeout_ms: 30_000,
                },
            );
            Ok(())
        })
        .await
        .unwrap();

    let payload = repository.raw_snapshot_payload().await;
    assert!(payload.contains("\"cliProfile\":\"default\""));
    assert!(!payload.contains("connector:revision:opaque"));
    assert!(!payload.contains("appSecret"));
    assert!(!payload.contains("appId"));
    assert!(!payload.contains("accessToken"));
    assert!(!payload.contains("actual-secret-value"));
}
