use super::*;

fn binding_for(
    workspace: &WorkspaceData,
    task_id: &str,
    package_version: u32,
) -> PackageExecutionBinding {
    PackageExecutionBinding {
        task_id: task_id.into(),
        package_version,
        prompt_renderer_version: CURRENT_PROMPT_RENDERER_VERSION,
        image_request_contract: Some(ImageRequestContract::CURRENT),
        article_template: workspace.prompt_templates[0].clone(),
        summary_template: workspace.prompt_templates[1].clone(),
        image_template: workspace.prompt_templates[2].clone(),
        article_provider: None,
        summary_provider: None,
        image_provider: None,
    }
}

#[test]
fn frozen_provider_authorization_uses_current_enabled_state() {
    let mut workspace = WorkspaceData::default();
    let provider = real_provider("real");
    workspace.provider_configs.push(provider.clone());
    let mut binding = binding_for(&workspace, "article", 1);
    binding.article_provider = Some(ProviderExecutionBinding {
        config: provider.clone(),
        credential_ref: Some("vault-reference".into()),
    });
    assert_eq!(
        ensure_binding_providers_enabled(&workspace, &binding),
        Ok(())
    );
    assert!(execution_binding_uses_credential(
        &binding,
        "vault-reference"
    ));

    workspace.provider_configs[0].enabled = false;
    let disabled = ensure_binding_providers_enabled(&workspace, &binding).unwrap_err();
    assert_eq!(disabled, BindingProviderError::Disabled);
    assert_eq!(disabled.code(), "PROVIDER_DISABLED");
    assert!(!disabled.message().contains("vault-reference"));

    workspace.provider_configs.clear();
    assert_eq!(
        ensure_binding_providers_enabled(&workspace, &binding),
        Err(BindingProviderError::NotConfigured)
    );
}

#[test]
fn upsert_and_lookup_keep_other_package_history() {
    let mut workspace = WorkspaceData::default();
    let old = binding_for(&workspace, "article", 1);
    let second = binding_for(&workspace, "article", 2);
    upsert_execution_binding(&mut workspace, old.clone());
    upsert_execution_binding(&mut workspace, second.clone());
    let mut replacement = old.clone();
    replacement.article_template.version = 9;
    upsert_execution_binding(&mut workspace, replacement.clone());

    assert_eq!(workspace.execution_bindings.len(), 2);
    assert_eq!(
        find_execution_binding(&workspace, "article", 1),
        Some(&replacement)
    );
    assert_eq!(
        find_execution_binding(&workspace, "article", 2),
        Some(&second)
    );
}

#[test]
fn stage_merge_changes_only_target_binding() {
    let workspace = WorkspaceData::default();
    let provider = real_provider("real");
    let mut current = binding_for(&workspace, "article", 1);
    current.article_provider = Some(ProviderExecutionBinding {
        config: provider.clone(),
        credential_ref: Some("article-ref".into()),
    });
    let mut proposed = current.clone();
    proposed.summary_template.version = 42;
    proposed.summary_provider = Some(ProviderExecutionBinding {
        config: provider,
        credential_ref: Some("summary-ref".into()),
    });
    proposed.image_request_contract = Some(ImageRequestContract::VolcengineArkUrlV1);

    let article_before = current.article_provider.clone();
    let image_before = current.image_request_contract;
    merge_stage_execution_binding(&mut current, &proposed, WorkflowStage::Summary);
    assert_eq!(current.summary_template.version, 42);
    assert_eq!(current.summary_provider, proposed.summary_provider);
    assert_eq!(current.article_provider, article_before);
    assert_eq!(current.image_request_contract, image_before);
}

#[test]
fn article_lookup_rejects_missing_identity_without_mutation() {
    let mut workspace = WorkspaceData::default();
    let before = workspace.clone();
    assert_eq!(
        find_article_mut(&mut workspace, "missing").unwrap_err(),
        "找不到指定文章"
    );
    assert_eq!(workspace, before);
}
