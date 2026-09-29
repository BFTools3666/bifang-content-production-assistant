use std::fs;

use bifang_content_production_assistant_core::ProviderConfig;

use super::*;

#[test]
fn preflight_never_accepts_paid_or_output_arguments() {
    let error = parse_arguments(vec![
        "preflight".into(),
        "--app-data-dir".into(),
        "/tmp/app".into(),
        "--confirm-paid-generation".into(),
    ])
    .unwrap_err();
    assert_eq!(error.code, "PROBE_PREFLIGHT_ARGUMENT_INVALID");
}

#[test]
fn execute_parses_explicit_charge_confirmation_and_paths() {
    let parsed = parse_arguments(vec![
        "execute".into(),
        "--app-data-dir".into(),
        "/tmp/app".into(),
        "--output-dir".into(),
        "/tmp/output".into(),
        "--confirm-paid-generation".into(),
    ])
    .unwrap();
    assert_eq!(parsed.mode, ProbeMode::Execute);
    assert!(parsed.confirmed_paid_generation);
    assert_eq!(parsed.output_dir, Some(PathBuf::from("/tmp/output")));
}

#[test]
fn provider_selection_uses_unique_enabled_default_ark_service() {
    let mut workspace = WorkspaceData::default();
    let mut non_default = ProviderConfig::volcengine_ark_image_draft();
    non_default.id = "other".into();
    non_default.default_for_image = false;
    let mut selected = ProviderConfig::volcengine_ark_image_draft();
    selected.id = "selected".into();
    selected.default_for_image = true;
    workspace.provider_configs = vec![non_default, selected];

    assert_eq!(
        select_ark_probe_provider(&workspace, None)
            .unwrap()
            .provider
            .id,
        "selected"
    );
}

#[test]
fn ark_probe_adapter_keeps_cli_specific_guidance_outside_core() {
    let ambiguous = ProbeFailure::from_core(ProviderProbeError::ArkProviderAmbiguous);
    assert_eq!(ambiguous.stage, "provider");
    assert_eq!(ambiguous.code, "PROBE_ARK_PROVIDER_AMBIGUOUS");
    assert!(ambiguous.message.contains("--provider-id"));
    let confirmation = ProbeFailure::from_core(ProviderProbeError::ArkPaidConfirmationRequired);
    assert_eq!(confirmation.stage, "confirmation");
    assert_eq!(confirmation.code, "PROBE_PAID_CONFIRMATION_REQUIRED");
    assert!(confirmation.message.contains("--confirm-paid-generation"));
    let default = ProbeFailure::from_core(ProviderProbeError::ArkDefaultAmbiguous);
    assert!(default.message.contains("在应用中修正配置"));
    let credential = ProbeFailure::from_core(ProviderProbeError::ArkCredentialPendingDeletion);
    assert_eq!(credential.stage, "credential");
    assert!(credential.message.contains("在应用中重新保存密钥"));
}

#[test]
fn image_inspection_requires_decodable_supported_bytes() {
    assert_eq!(
        inspect_image(b"\x89PNG\r\n\x1a\nnot-pixels")
            .unwrap_err()
            .code,
        "PROBE_IMAGE_DECODE_FAILED"
    );
    assert_eq!(
        inspect_image(b"plain text").unwrap_err().code,
        "PROBE_IMAGE_MAGIC_INVALID"
    );
}

#[test]
fn output_directory_must_be_outside_the_workspace() {
    let root = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-ark-probe-output-isolation-{}",
        uuid::Uuid::new_v4()
    ));
    let app_data_dir = root.join("app-data");
    let nested_output_dir = app_data_dir.join("probe-output");
    let isolated_output_dir = root.join("isolated-output");
    fs::create_dir_all(&nested_output_dir).unwrap();
    fs::create_dir_all(&isolated_output_dir).unwrap();
    let canonical_app_data_dir = app_data_dir.canonicalize().unwrap();

    assert_eq!(
        validate_output_dir(&app_data_dir, &canonical_app_data_dir)
            .unwrap_err()
            .code,
        "PROBE_OUTPUT_DIR_NOT_ISOLATED"
    );
    assert_eq!(
        validate_output_dir(&nested_output_dir, &canonical_app_data_dir)
            .unwrap_err()
            .code,
        "PROBE_OUTPUT_DIR_NOT_ISOLATED"
    );
    assert_eq!(
        validate_output_dir(&isolated_output_dir, &canonical_app_data_dir).unwrap(),
        isolated_output_dir.canonicalize().unwrap()
    );

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn read_only_workspace_uses_the_literal_special_character_path() {
    let root = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-ark-probe-db-%2F-{}",
        uuid::Uuid::new_v4()
    ));
    let app_data_dir = root.join("app%2edata");
    fs::create_dir_all(&app_data_dir).unwrap();
    let database_path = app_data_dir.join(DATABASE_FILE_NAME);
    let options = SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sea_orm::sqlx::query(
        "CREATE TABLE workspace_snapshot (id INTEGER PRIMARY KEY, schema_version INTEGER NOT NULL, payload TEXT NOT NULL, updated_at_ms INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let workspace = WorkspaceData::default();
    sea_orm::sqlx::query(
        "INSERT INTO workspace_snapshot (id, schema_version, payload, updated_at_ms) VALUES (1, ?, ?, 1)",
    )
    .bind(i64::from(WORKSPACE_SCHEMA_VERSION))
    .bind(serde_json::to_string(&workspace).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let (loaded, canonical_app_data_dir) = load_workspace_read_only(&app_data_dir).await.unwrap();
    assert_eq!(loaded.schema_version, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(canonical_app_data_dir, app_data_dir.canonicalize().unwrap());

    fs::remove_dir_all(root).unwrap();
}
