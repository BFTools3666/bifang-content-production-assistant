use super::*;

/// 最新模板查询只返回每个稳定 ID 的最高版本。
#[test]
fn latest_templates_hide_old_versions() {
    let mut templates = WorkspaceData::default().prompt_templates;
    let mut revised = templates[0].clone();
    revised.version = 2;
    revised.content = "新版 {{title}}".into();
    templates.push(revised.clone());

    let latest = latest_prompt_templates(&templates);

    assert_eq!(latest.len(), 3);
    assert_eq!(
        latest
            .iter()
            .find(|template| template.id == revised.id)
            .unwrap()
            .version,
        2
    );
}
