use super::*;

#[test]
fn provider_draft_normalization_keeps_core_defaults_and_injected_id() {
    let mut draft = ProviderConfig::open_ai_compatible_draft();
    draft.name = "  正文服务  ".into();
    draft.base_url = "  https://example.com/v1  ".into();
    draft.text_model = "  legacy  ".into();
    draft.article_model = Some("  article-model  ".into());
    draft.summary_model = Some("  summary-model  ".into());
    draft.image_model = "  image-model  ".into();

    let prepared = prepare_provider_config(draft, "provider-injected").unwrap();
    assert_eq!(prepared.id, "provider-injected");
    assert_eq!(prepared.name, "正文服务");
    assert_eq!(prepared.base_url, "https://example.com/v1");
    assert_eq!(prepared.article_model.as_deref(), Some("article-model"));
    assert_eq!(prepared.summary_model.as_deref(), Some("summary-model"));
    assert_eq!(prepared.text_model, "article-model");
    assert_eq!(prepared.image_model, "image-model");
}

#[test]
fn stale_provider_revision_never_changes_workspace_or_credential_reference() {
    let mut workspace = WorkspaceData::default();
    let current = real_provider("provider-one");
    workspace.provider_configs.push(current.clone());
    workspace
        .provider_credential_refs
        .insert(current.id.clone(), "provider-one:revision:current".into());
    let before = workspace.clone();
    let mut requested = current;
    requested.revision -= 1;

    assert_eq!(
        apply_workspace_provider_config_update(
            &mut workspace,
            requested,
            Some("provider-one:revision:new"),
        ),
        Err("PROVIDER_REVISION_CONFLICT".into())
    );
    assert_eq!(workspace, before);
}

#[test]
fn replacing_provider_queues_only_unreferenced_credential() {
    let mut workspace = WorkspaceData::default();
    let provider = real_provider("provider-one");
    workspace.provider_configs.push(provider.clone());
    workspace
        .provider_credential_refs
        .insert(provider.id.clone(), "provider-one:revision:old".into());

    let (_, active, superseded) = apply_workspace_provider_config_update(
        &mut workspace,
        provider,
        Some("provider-one:revision:new"),
    )
    .unwrap();

    assert_eq!(active, "provider-one:revision:new");
    assert_eq!(superseded.as_deref(), Some("provider-one:revision:old"));
    assert!(
        workspace
            .pending_credential_deletions
            .contains("provider-one:revision:old")
    );
    assert!(!workspace.pending_credential_deletions.contains(&active));
}

#[test]
fn key_rotation_rule_depends_on_existing_key_and_transport_change() {
    assert!(!endpoint_change_requires_key_rotation(false, true));
    assert!(!endpoint_change_requires_key_rotation(true, false));
    assert!(endpoint_change_requires_key_rotation(true, true));
}

#[test]
fn provider_lifecycle_rejects_stale_revision_without_mutation() {
    let mut workspace = WorkspaceData::default();
    let provider = real_provider("provider-one");
    workspace.provider_configs.push(provider.clone());
    let before = workspace.clone();

    assert_eq!(
        set_provider_enabled_in_workspace(
            &mut workspace,
            &provider.id,
            provider.revision - 1,
            false
        ),
        Err("PROVIDER_REVISION_CONFLICT".into())
    );
    assert_eq!(workspace, before);
}

#[test]
fn deleting_provider_detaches_historical_credential_references() {
    let mut workspace = WorkspaceData::default();
    let mut provider = real_provider("provider-one");
    provider.default_for_article = false;
    provider.default_for_summary = false;
    provider.default_for_image = false;
    workspace.provider_configs.push(provider.clone());
    workspace
        .provider_credential_refs
        .insert(provider.id.clone(), "provider-one:revision:current".into());
    let templates = workspace.prompt_templates.clone();
    workspace.execution_bindings.push(PackageExecutionBinding {
        task_id: "article-one".into(),
        package_version: 1,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: None,
        article_template: templates[0].clone(),
        summary_template: templates[1].clone(),
        image_template: templates[2].clone(),
        article_provider: Some(ProviderExecutionBinding {
            config: provider.clone(),
            credential_ref: Some("provider-one:revision:old".into()),
        }),
        summary_provider: None,
        image_provider: None,
    });

    let deleted =
        delete_provider_in_workspace(&mut workspace, &provider.id, provider.revision).unwrap();

    assert_eq!(deleted.historical_bindings_detached, 1);
    assert!(
        deleted
            .credential_refs
            .contains("provider-one:revision:old")
    );
    assert!(
        deleted
            .credential_refs
            .contains("provider-one:revision:current")
    );
    assert!(workspace.provider_configs.is_empty());
    assert!(
        workspace.execution_bindings[0]
            .article_provider
            .as_ref()
            .unwrap()
            .credential_ref
            .is_none()
    );
    assert_eq!(
        workspace.pending_credential_deletions,
        deleted.credential_refs
    );
}
