use super::*;

/// 旧运行记录缺少 targetStage 时保持完整批次语义，新记录按目标阶段收口。
#[test]
fn run_record_target_stage_is_backward_compatible() {
    let legacy = serde_json::json!({
        "id": "legacy",
        "taskIds": ["task"],
        "status": "succeeded",
        "startedAtMs": 1,
        "finishedAtMs": 2,
        "completedCount": 1,
        "failedCount": 0,
        "isTestFixture": false
    });
    let run: RunRecord = serde_json::from_value(legacy).unwrap();

    assert_eq!(run.target_stage, None);
    assert_eq!(
        terminal_stage_run_status(StageStatus::Succeeded).unwrap(),
        RunStatus::Succeeded
    );
    assert!(terminal_stage_run_status(StageStatus::Queued).is_err());
}

/// v9 快照缺少全部连接器字段时以空集合升级到当前版本，第二次迁移保持幂等。
#[test]
fn connector_schema_migration_accepts_v9_json_and_is_idempotent() {
    let mut legacy = serde_json::to_value(WorkspaceData::default()).unwrap();
    legacy["schemaVersion"] = serde_json::json!(PRE_CONNECTOR_SCHEMA_VERSION);
    let object = legacy.as_object_mut().unwrap();
    for field in [
        "connectorConfigs",
        "connectorConfigHistory",
        "connectorCredentialRefs",
        "pendingConnectorCredentialDeletions",
        "documentSyncRecords",
    ] {
        object.remove(field);
    }
    let mut reopened: WorkspaceData = serde_json::from_value(legacy).unwrap();

    assert!(reopened.connector_configs.is_empty());
    assert!(reopened.connector_config_history.is_empty());
    assert!(reopened.connector_credential_refs.is_empty());
    assert!(reopened.pending_connector_credential_deletions.is_empty());
    assert!(reopened.document_sync_records.is_empty());
    assert!(reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert!(!reopened.migrate_to_current_schema().unwrap());
}

/// v10 直连配置保留非秘密身份与历史，但停用当前目标并把旧秘密排队清理。
#[test]
fn connector_cli_migration_disables_legacy_target_and_queues_secret_cleanup() {
    let mut legacy = serde_json::to_value(WorkspaceData::default()).unwrap();
    legacy["schemaVersion"] = serde_json::json!(PRE_LARK_CLI_CONNECTOR_SCHEMA_VERSION);
    legacy["connectorConfigs"] = serde_json::json!([
        {
            "id": "connector-legacy",
            "name": "旧飞书",
            "revision": 7,
            "kind": "feishuDrive",
            "enabled": true,
            "appId": "cli_legacy",
            "folderToken": "fld_target",
            "defaultFormat": "markdown",
            "isDefault": true,
            "timeoutMs": 30000
        },
        {
            "id": "connector-disabled",
            "name": "旧停用目标",
            "revision": 3,
            "kind": "feishuDrive",
            "enabled": false,
            "appId": "cli_disabled",
            "folderToken": "fld_disabled",
            "defaultFormat": "html",
            "isDefault": false,
            "timeoutMs": 45000
        }
    ]);
    legacy["connectorConfigHistory"] = serde_json::json!([{
        "id": "connector-old-history",
        "name": "既有历史",
        "revision": 2,
        "kind": "feishuDrive",
        "enabled": false,
        "appId": "cli_history",
        "folderToken": "fld_history",
        "defaultFormat": "markdown",
        "isDefault": false,
        "timeoutMs": 30000
    }]);
    legacy["connectorCredentialRefs"] = serde_json::json!({
        "connector-legacy": "connector:revision:legacy"
    });
    legacy["documentSyncRecords"] = serde_json::json!([{
        "taskId": "task-legacy",
        "packageVersion": 6,
        "connectorId": "connector-legacy",
        "connectorRevision": 7,
        "format": "markdown",
        "fileToken": "file-legacy",
        "fileName": "legacy-v6.md",
        "syncedAtMs": 1234
    }]);
    let mut reopened: WorkspaceData = serde_json::from_value(legacy).unwrap();

    assert!(reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened.schema_version, WORKSPACE_SCHEMA_VERSION);
    let active = reopened
        .connector_configs
        .iter()
        .find(|connector| connector.id == "connector-legacy")
        .unwrap();
    assert_eq!(active.revision, 8);
    assert_eq!(active.folder_token, "fld_target");
    assert!(active.cli_profile.is_empty());
    assert!(!active.enabled);
    assert!(!active.is_default);
    let disabled = reopened
        .connector_configs
        .iter()
        .find(|connector| connector.id == "connector-disabled")
        .unwrap();
    assert_eq!(disabled.revision, 4);
    assert_eq!(disabled.default_format, DocumentFormat::Html);
    assert!(!disabled.enabled);
    assert_eq!(reopened.connector_config_history.len(), 3);
    assert!(
        reopened.connector_config_history.iter().any(|connector| {
            connector.id == "connector-old-history" && connector.revision == 2
        })
    );
    assert!(
        reopened
            .connector_config_history
            .iter()
            .any(|connector| { connector.id == "connector-legacy" && connector.revision == 7 })
    );
    assert!(
        reopened
            .connector_config_history
            .iter()
            .any(|connector| { connector.id == "connector-disabled" && connector.revision == 3 })
    );
    assert!(reopened.connector_credential_refs.is_empty());
    assert!(
        reopened
            .pending_connector_credential_deletions
            .contains("connector:revision:legacy")
    );
    assert_eq!(reopened.document_sync_records.len(), 1);
    assert_eq!(reopened.document_sync_records[0].file_token, "file-legacy");
    assert_eq!(reopened.document_sync_records[0].connector_revision, 7);
    let first_migration = reopened.clone();
    assert!(!reopened.migrate_to_current_schema().unwrap());
    assert_eq!(reopened, first_migration);
}
