//! 云文档连接器命令层的持久化与幂等回归测试。

use bifang_content_production_assistant_core::{
    ArticleTask, ConnectorConfig, ConnectorKind, DocumentFormat, DocumentSyncRecord, StageStatus,
};

use super::commands::{
    MAX_FEISHU_FILE_BYTES, apply_connector_update_in_workspace, delete_connector_in_workspace,
    persist_uploaded_sync_record, recheck_document_sync_before_upload,
    reconcile_pending_connector_credential_deletions_inner, reusable_sync_record,
    validate_upload_size,
};
use crate::workbench::{models::WorkspaceData, store::WorkspaceRepository};

fn connector(id: &str, revision: u32) -> ConnectorConfig {
    ConnectorConfig {
        id: id.into(),
        name: "飞书".into(),
        revision,
        kind: ConnectorKind::FeishuDrive,
        enabled: true,
        cli_profile: "team".into(),
        folder_token: "fld_target".into(),
        default_format: DocumentFormat::Markdown,
        is_default: false,
        timeout_ms: 30_000,
    }
}

/// 保存 CLI 发布目标只更新非秘密配置，不建立或改动遗留凭据引用。
#[test]
fn connector_update_never_creates_a_credential_reference() {
    let mut workspace = WorkspaceData::default();
    let saved =
        apply_connector_update_in_workspace(&mut workspace, connector("connector", 1)).unwrap();
    let mut revised = saved;
    revised.folder_token = "fld_next".into();
    apply_connector_update_in_workspace(&mut workspace, revised).unwrap();

    assert!(workspace.connector_credential_refs.is_empty());
    assert!(workspace.pending_connector_credential_deletions.is_empty());
    let serialized = serde_json::to_string(&workspace).unwrap();
    assert!(serialized.contains("\"cliProfile\":\"team\""));
    assert!(!serialized.contains("appId"));
    assert!(!serialized.contains("appSecret"));
}

/// 删除不受历史同步记录阻挡；异常残留的旧秘密引用仍会排队清理。
#[test]
fn deletes_connector_without_erasing_sync_history() {
    let mut workspace = WorkspaceData::default();
    workspace.connector_configs.push(connector("connector", 3));
    workspace
        .connector_credential_refs
        .insert("connector".into(), "connector:revision:third".into());
    workspace.document_sync_records.push(DocumentSyncRecord {
        task_id: "task".into(),
        package_version: 1,
        connector_id: "connector".into(),
        connector_revision: 3,
        format: DocumentFormat::Markdown,
        file_token: "file".into(),
        file_name: "article-v1.md".into(),
        synced_at_ms: 1,
    });

    let removed = delete_connector_in_workspace(&mut workspace, "connector", 3).unwrap();

    assert!(workspace.connector_configs.is_empty());
    assert_eq!(workspace.connector_config_history.len(), 1);
    assert_eq!(workspace.document_sync_records.len(), 1);
    assert!(removed.contains("connector:revision:third"));
    assert!(
        workspace
            .pending_connector_credential_deletions
            .contains("connector:revision:third")
    );
}

/// 同任务、包、连接器修订和格式只复用成功记录，其他格式仍需真实上传。
#[test]
fn finds_only_exact_success_for_idempotent_sync() {
    let mut workspace = WorkspaceData::default();
    workspace.document_sync_records.push(DocumentSyncRecord {
        task_id: "task".into(),
        package_version: 2,
        connector_id: "connector".into(),
        connector_revision: 5,
        format: DocumentFormat::Markdown,
        file_token: "file".into(),
        file_name: "article-v2.md".into(),
        synced_at_ms: 8,
    });
    assert!(
        reusable_sync_record(
            &workspace,
            "task",
            2,
            "connector",
            5,
            DocumentFormat::Markdown,
        )
        .is_some()
    );
    assert!(
        reusable_sync_record(&workspace, "task", 2, "connector", 5, DocumentFormat::Html).is_none()
    );
}

/// 飞书上传边界接受恰好 20MB，并在任何出站前拒绝空文件和超限文件。
#[test]
fn validates_twenty_megabyte_upload_boundary() {
    assert!(validate_upload_size(MAX_FEISHU_FILE_BYTES).is_ok());
    assert_eq!(
        validate_upload_size(0).unwrap_err().code,
        "FORMAT_ARTIFACT_MISSING"
    );
    assert_eq!(
        validate_upload_size(MAX_FEISHU_FILE_BYTES + 1)
            .unwrap_err()
            .code,
        "CONNECTOR_FILE_TOO_LARGE"
    );
}

/// 清理失败保留队列供下次启动重试，成功后只移除对应引用。
#[tokio::test]
async fn connector_credential_cleanup_is_retryable() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            workspace
                .pending_connector_credential_deletions
                .insert("old-secret".into());
            Ok(())
        })
        .await
        .unwrap();
    reconcile_pending_connector_credential_deletions_inner(&repository, |_| Err("locked".into()))
        .await;
    assert!(
        repository
            .snapshot()
            .await
            .pending_connector_credential_deletions
            .contains("old-secret")
    );
    reconcile_pending_connector_credential_deletions_inner(&repository, |_| Ok(())).await;
    assert!(
        repository
            .snapshot()
            .await
            .pending_connector_credential_deletions
            .is_empty()
    );
}

/// 飞书已成功返回后若 SQLite 写入失败，命令必须阻止普通重试并提示先查远端。
#[tokio::test]
async fn post_upload_persistence_failure_is_outcome_unknown() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository.reject_snapshot_updates_for_test().await;
    let error = persist_uploaded_sync_record(
        &repository,
        DocumentSyncRecord {
            task_id: "task".into(),
            package_version: 1,
            connector_id: "connector".into(),
            connector_revision: 1,
            format: DocumentFormat::Markdown,
            file_token: "remote-file".into(),
            file_name: "article-v1.md".into(),
            synced_at_ms: 10,
        },
    )
    .await
    .unwrap_err();

    assert_eq!(error.code, "CONNECTOR_SYNC_OUTCOME_UNKNOWN");
    assert!(error.message.contains("先在飞书检查"));
    assert!(repository.snapshot().await.document_sync_records.is_empty());
}

/// 最后一道出站检查必须读取最新工作区，而不是复用最初构造文件时的快照。
#[tokio::test]
async fn final_upload_check_rejects_article_and_connector_changes() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.artifacts.markdown = Some("# 正文".into());
            article.artifacts.image_ref = Some("images/cover.png".into());
            article.stages.format.status = StageStatus::Succeeded;
            article.stages.format.artifact_version = Some(article.package_version);
            workspace.articles.push(article);
            workspace.connector_configs.push(connector("connector", 3));
            Ok(())
        })
        .await
        .unwrap();
    let row_version = repository.snapshot().await.articles[0].row_version;
    let check = || {
        recheck_document_sync_before_upload(
            &repository,
            "task",
            "connector",
            DocumentFormat::Markdown,
            row_version,
            1,
            3,
        )
    };
    assert!(check().await.is_ok());

    repository
        .update(|workspace| {
            workspace.articles[0].row_version += 1;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(check().await.unwrap_err().code, "ARTICLE_REVISION_CONFLICT");

    repository
        .update(|workspace| {
            workspace.articles[0].row_version = row_version;
            workspace.connector_configs[0].revision += 1;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        check().await.unwrap_err().code,
        "CONNECTOR_REVISION_CONFLICT"
    );
}
