use super::*;

#[test]
fn latest_prompt_selection_keeps_highest_version_per_id() {
    let mut first = initial_prompt_templates().remove(0);
    let latest = first.revised("请为 {{title}} 写正文").unwrap();
    let original = first.clone();
    first.version = 1;
    let selected = latest_prompt_templates(&[latest.clone(), first]);

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0], latest);
    assert_eq!(original.version, 1);
}

#[test]
fn prompt_revision_appends_without_mutating_history() {
    let mut workspace = WorkspaceData::default();
    let original = workspace.prompt_templates[0].clone();
    let requested = PromptTemplate {
        name: "新版正文模板".into(),
        content: "请为 {{title}} 写正文".into(),
        ..original.clone()
    };
    let revised = append_prompt_template_revision(&mut workspace, &requested).unwrap();

    assert_eq!(revised.id, original.id);
    assert_eq!(revised.kind, original.kind);
    assert_eq!(revised.version, original.version + 1);
    assert_eq!(workspace.prompt_templates[0], original);
    assert_eq!(workspace.prompt_templates.last(), Some(&revised));
}

#[test]
fn prompt_revision_rejects_kind_change_and_exhaustion_atomically() {
    let mut workspace = WorkspaceData::default();
    let initial = workspace.prompt_templates[0].clone();
    let mut wrong_kind = initial.clone();
    wrong_kind.kind = crate::PromptTemplateKind::Summary;
    let before = workspace.clone();
    assert_eq!(
        append_prompt_template_revision(&mut workspace, &wrong_kind),
        Err(PromptWorkspaceError::KindMismatch)
    );
    assert_eq!(workspace, before);

    workspace.prompt_templates[0].version = u32::MAX;
    let before = workspace.clone();
    assert_eq!(
        append_prompt_template_revision(&mut workspace, &initial),
        Err(PromptWorkspaceError::Template(
            crate::TemplateError::VersionExhausted
        ))
    );
    assert_eq!(workspace, before);
}
