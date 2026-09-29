use super::*;

/// 运行标记要求显式夹具模式且正文、简介、图片均由夹具提供。
#[test]
fn fixture_run_marker_requires_mode_and_both_default_capabilities() {
    let mut workspace = WorkspaceData::default();
    workspace.provider_configs.push(ProviderConfig {
        id: "fixture".into(),
        name: "fixture".into(),
        revision: 1,
        kind: ProviderKind::Fixture,
        enabled: true,
        base_url: String::new(),
        text_model: "text".into(),
        article_model: Some("article".into()),
        summary_model: Some("summary".into()),
        image_model: "image".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: false,
        default_for_image: false,
    });

    assert!(!run_uses_fixture_for_mode(&workspace, false));
    assert!(!run_uses_fixture_for_mode(&workspace, true));
    workspace.provider_configs[0].default_for_summary = true;
    assert!(!run_uses_fixture_for_mode(&workspace, true));
    workspace.provider_configs[0].default_for_image = true;
    assert!(run_uses_fixture_for_mode(&workspace, true));
}

/// 所有启动入口共用同一个历史写入函数，并把持久记录限制为最近一百次。
#[test]
fn queued_run_history_is_bounded() {
    let mut workspace = WorkspaceData::default();
    for index in 0..101 {
        record_queued_run(
            &mut workspace,
            format!("batch-{index}"),
            vec![format!("article-{index}")],
        );
    }

    assert_eq!(workspace.run_history.len(), 100);
    assert_eq!(workspace.run_history[0].id, "batch-100");
    assert_eq!(workspace.run_history[99].id, "batch-1");
}
