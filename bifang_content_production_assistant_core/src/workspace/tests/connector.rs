use super::*;
use crate::{ConnectorKind, DocumentFormat, DocumentSyncError};

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

fn ready_workspace() -> WorkspaceData {
    let mut workspace = WorkspaceData::default();
    let mut article = ArticleTask::new("task", "标题", 1);
    article.artifacts.markdown = Some("# 正文".into());
    article.artifacts.html = Some("<h1>正文</h1>".into());
    article.artifacts.image_ref = Some("images/cover.png".into());
    article.stages.format.status = StageStatus::Succeeded;
    article.stages.format.artifact_version = Some(article.package_version);
    workspace.articles.push(article);
    workspace.connector_configs.push(connector("connector", 3));
    workspace
}

fn sync_record(format: DocumentFormat) -> DocumentSyncRecord {
    DocumentSyncRecord {
        task_id: "task".into(),
        package_version: 1,
        connector_id: "connector".into(),
        connector_revision: 3,
        format,
        file_token: "file-token".into(),
        file_name: format!("article.{}", format.extension()),
        synced_at_ms: 10,
    }
}

#[test]
fn config_save_normalizes_fields_and_keeps_history_without_secrets() {
    let mut draft = connector("", 1);
    draft.name = "  飞书目标  ".into();
    draft.cli_profile = "  team  ".into();
    draft.folder_token = "  fld_target  ".into();
    let prepared = prepare_connector_config_for_save(draft, "connector-new".into()).unwrap();
    assert_eq!(prepared.id, "connector-new");
    assert_eq!(prepared.name, "飞书目标");
    assert_eq!(prepared.cli_profile, "team");
    assert_eq!(prepared.folder_token, "fld_target");

    let mut workspace = WorkspaceData::default();
    let saved = apply_connector_update_in_workspace(&mut workspace, prepared).unwrap();
    assert_eq!(saved.revision, 1);
    let mut revised = saved;
    revised.folder_token = "fld_next".into();
    let saved = apply_connector_update_in_workspace(&mut workspace, revised).unwrap();
    assert_eq!(saved.revision, 2);
    assert_eq!(workspace.connector_config_history.len(), 1);
    assert!(workspace.connector_credential_refs.is_empty());
}

#[test]
fn delete_requires_confirmation_and_preserves_history_with_shared_refs() {
    assert_eq!(
        validate_connector_delete_confirmation(false),
        Err("删除连接器前必须明确确认")
    );
    let mut workspace = WorkspaceData::default();
    workspace.connector_configs.push(connector("first", 3));
    workspace.connector_configs.push(connector("second", 1));
    workspace
        .connector_credential_refs
        .insert("first".into(), "shared-ref".into());
    workspace
        .connector_credential_refs
        .insert("second".into(), "shared-ref".into());
    workspace
        .document_sync_records
        .push(sync_record(DocumentFormat::Markdown));
    let before = workspace.clone();
    assert!(delete_connector_in_workspace(&mut workspace, "first", 2).is_err());
    assert_eq!(workspace, before);

    let removed = delete_connector_in_workspace(&mut workspace, "first", 3).unwrap();
    assert!(removed.contains("shared-ref"));
    assert!(
        !workspace
            .pending_connector_credential_deletions
            .contains("shared-ref")
    );
    assert_eq!(workspace.document_sync_records.len(), 1);
    assert_eq!(workspace.connector_config_history.len(), 1);
    delete_connector_in_workspace(&mut workspace, "second", 1).unwrap();
    assert!(
        workspace
            .pending_connector_credential_deletions
            .contains("shared-ref")
    );
    workspace
        .connector_credential_refs
        .insert("new".into(), "shared-ref".into());
    assert!(cancel_pending_connector_credential_deletion_if_referenced(
        &mut workspace,
        "shared-ref"
    ));
    assert!(
        !workspace
            .pending_connector_credential_deletions
            .contains("shared-ref")
    );
}

#[test]
fn sync_selection_is_fail_closed_and_reuses_only_exact_success() {
    let mut workspace = ready_workspace();
    let row = workspace.articles[0].row_version;
    let select = |workspace: &WorkspaceData, format, row, revision, confirmed| {
        select_document_sync(
            workspace,
            "task",
            "connector",
            format,
            row,
            1,
            revision,
            confirmed,
        )
    };
    assert_eq!(
        select(&workspace, DocumentFormat::Markdown, row, 3, false),
        Err(ConnectorSelectionError::DocumentSync(
            DocumentSyncError::ConfirmationRequired
        ))
    );
    assert_eq!(
        select(&workspace, DocumentFormat::Markdown, row + 1, 3, true),
        Err(ConnectorSelectionError::DocumentSync(
            DocumentSyncError::ArticleRevisionConflict
        ))
    );
    assert_eq!(
        select(&workspace, DocumentFormat::Markdown, row, 4, true),
        Err(ConnectorSelectionError::DocumentSync(
            DocumentSyncError::ConnectorRevisionConflict
        ))
    );
    assert!(matches!(
        select(&workspace, DocumentFormat::Markdown, row, 3, true),
        Ok(DocumentSyncSelection::Ready { .. })
    ));
    let saved =
        record_uploaded_document_sync(&mut workspace, sync_record(DocumentFormat::Markdown));
    assert_eq!(saved.file_token, "file-token");
    assert!(matches!(
        select(&workspace, DocumentFormat::Markdown, row, 3, true),
        Ok(DocumentSyncSelection::Reused(_))
    ));
    assert!(matches!(
        select(&workspace, DocumentFormat::Html, row, 3, true),
        Ok(DocumentSyncSelection::Ready { .. })
    ));
    let mut duplicate = sync_record(DocumentFormat::Markdown);
    duplicate.file_token = "duplicate-remote-file".into();
    let saved = record_uploaded_document_sync(&mut workspace, duplicate);
    assert_eq!(saved.file_token, "file-token");
    assert_eq!(workspace.document_sync_records.len(), 1);

    workspace.connector_configs[0].enabled = false;
    assert_eq!(
        select(&workspace, DocumentFormat::Markdown, row, 3, true),
        Err(ConnectorSelectionError::DocumentSync(
            DocumentSyncError::ConnectorDisabled
        ))
    );
}

#[test]
fn connection_test_and_upload_size_obey_core_boundaries() {
    let mut workspace = ready_workspace();
    assert_eq!(
        connector_for_connection_test(&workspace, "missing"),
        Err(ConnectorSelectionError::ConnectorNotFound)
    );
    assert!(connector_for_connection_test(&workspace, "connector").is_ok());
    workspace.connector_configs[0].enabled = false;
    assert_eq!(
        connector_for_connection_test(&workspace, "connector"),
        Err(ConnectorSelectionError::ConnectorDisabled)
    );
    assert_eq!(validate_upload_size(0), Err(DocumentUploadSizeError::Empty));
    assert_eq!(validate_upload_size(MAX_FEISHU_FILE_BYTES), Ok(()));
    assert_eq!(
        validate_upload_size(MAX_FEISHU_FILE_BYTES + 1),
        Err(DocumentUploadSizeError::TooLarge)
    );
}
