use crate::{ArtifactLineage, ArtifactOutput};

use super::*;

fn connector(id: &str, revision: u32, is_default: bool) -> ConnectorConfig {
    ConnectorConfig {
        id: id.into(),
        name: id.into(),
        revision,
        kind: ConnectorKind::FeishuDrive,
        enabled: true,
        cli_profile: format!("profile-{id}"),
        folder_token: format!("fld_{id}"),
        default_format: DocumentFormat::Markdown,
        is_default,
        timeout_ms: 30_000,
    }
}

fn completed_article() -> ArticleTask {
    let mut article = ArticleTask::new("task", "标题/测试", 1);
    article.queue_initial(2).unwrap();
    for (index, output) in [
        ArtifactOutput::Article("# 标题\n\n正文".into()),
        ArtifactOutput::Summary("简介".into()),
        ArtifactOutput::Image("managed.png".into()),
        ArtifactOutput::Format("<h1>标题</h1><p>正文</p>".into()),
    ]
    .into_iter()
    .enumerate()
    {
        let stage = output.stage();
        let started = 3 + i64::try_from(index).unwrap() * 2;
        let attempt_id = article.start_stage(stage, started).unwrap();
        article
            .complete_stage(
                stage,
                output,
                ArtifactLineage {
                    package_version: 1,
                    source_article_version: (stage != WorkflowStage::Article).then_some(1),
                    source_summary_version: (stage == WorkflowStage::Image).then_some(1),
                    template_id: None,
                    template_version: None,
                    provider_id: None,
                    provider_revision: None,
                    model: None,
                    attempt_id,
                },
                started + 1,
            )
            .unwrap();
    }
    article
}

/// 保存默认连接器会创建不可变修订，并原子挤下旧默认项。
#[test]
fn revises_connector_and_keeps_single_default() {
    let mut current = vec![connector("old", 4, true), connector("target", 2, false)];
    let mut history = Vec::new();
    let mut requested = current[1].clone();
    requested.is_default = true;

    let update = apply_connector_config_update(&mut current, &mut history, requested).unwrap();

    assert_eq!(update.saved.revision, 3);
    assert!(update.saved.is_default);
    assert_eq!(current.iter().filter(|item| item.is_default).count(), 1);
    assert_eq!(current[0].revision, 5);
    assert_eq!(history.len(), 2);
}

/// 陈旧修订不能覆盖当前连接器，重复启停不会制造无意义修订。
#[test]
fn rejects_stale_revision_and_keeps_idempotent_enable() {
    let mut current = vec![connector("target", 3, false)];
    let mut history = Vec::new();
    let mut stale = current[0].clone();
    stale.revision = 2;
    assert_eq!(
        apply_connector_config_update(&mut current, &mut history, stale),
        Err(ConnectorConfigUpdateError::RevisionConflict)
    );
    let update =
        set_connector_config_enabled(&mut current, &mut history, "target", 3, true).unwrap();
    assert_eq!(update.saved.revision, 3);
    assert!(history.is_empty());
}

/// 持久化与 CLI 向导使用相同的 1..64 字符安全子集。
#[test]
fn validates_cli_profile_with_the_setup_safety_contract() {
    for valid in [
        "seo-publisher".to_string(),
        "内容发布".to_string(),
        "a".repeat(64),
    ] {
        let mut config = connector("target", 1, false);
        config.cli_profile = valid;
        assert_eq!(config.validate_for_persistence(), Ok(()));
    }

    for invalid in [
        " leading".to_string(),
        "trailing ".to_string(),
        "with space".to_string(),
        "prod;rm".to_string(),
        "../escape".to_string(),
        "name$HOME".to_string(),
        "a".repeat(65),
    ] {
        let mut config = connector("target", 1, false);
        config.cli_profile = invalid;
        assert_eq!(
            config.validate_for_persistence(),
            Err(ConnectorConfigError::CliProfileInvalid)
        );
    }
}

/// 旧直连目标缺少 CLI profile 时必须建立新修订并失败关闭。
#[test]
fn migrates_legacy_connector_to_disabled_cli_target() {
    let mut config = connector("legacy", 4, true);
    config.cli_profile.clear();
    let mut current = vec![config];

    let mut history = Vec::new();

    assert!(migrate_connector_configs_to_cli(&mut current, &mut history).unwrap());
    assert!(!current[0].enabled);
    assert!(!current[0].is_default);
    assert_eq!(current[0].revision, 5);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].revision, 4);
    assert_eq!(
        current[0].validate_for_persistence(),
        Err(ConnectorConfigError::CliProfileRequired)
    );
}

/// 已停用的旧目标仍需保留旧当前修订，不能因没有启用而跳过迁移。
#[test]
fn migrates_an_already_disabled_legacy_connector() {
    let mut config = connector("legacy-disabled", 9, false);
    config.cli_profile.clear();
    config.enabled = false;
    let mut current = vec![config];
    let mut history = Vec::new();

    assert!(migrate_connector_configs_to_cli(&mut current, &mut history).unwrap());
    assert_eq!(current[0].revision, 10);
    assert!(!current[0].enabled);
    assert_eq!(history[0].revision, 9);
}

/// 同步必须冻结文章与连接器修订，并只接受当前包成功的格式阶段。
#[test]
fn validates_sync_preconditions() {
    let article = completed_article();
    let config = connector("feishu", 7, true);
    let row_version = article.row_version;
    assert_eq!(
        validate_document_sync(
            &article,
            &config,
            DocumentFormat::Html,
            row_version,
            1,
            7,
            true,
        ),
        Ok(())
    );
    assert_eq!(
        validate_document_sync(
            &article,
            &config,
            DocumentFormat::Html,
            row_version.saturating_sub(1),
            1,
            7,
            true,
        ),
        Err(DocumentSyncError::ArticleRevisionConflict)
    );
    assert_eq!(
        validate_document_sync(
            &article,
            &config,
            DocumentFormat::Html,
            row_version,
            1,
            7,
            false,
        ),
        Err(DocumentSyncError::ConfirmationRequired)
    );
}

/// Markdown 与 HTML 投影都嵌入当前图片并转义标题结构字符。
#[test]
fn builds_self_contained_markdown_and_html() {
    let data = "data:image/png;base64,iVBORw0KGgo=";
    let markdown =
        build_self_contained_document(DocumentFormat::Markdown, "# 标题\n\n正文", data, "a[b]")
            .unwrap();
    assert!(markdown.starts_with("# 标题\n\n![a\\[b\\]](data:image/png;base64,"));
    let html = build_self_contained_document(
        DocumentFormat::Html,
        "<h1>标题</h1><p>正文</p>",
        data,
        "<标题>",
    )
    .unwrap();
    assert!(html.contains("</h1>\n<figure><img src=\"data:image/png;base64,"));
    assert!(html.contains("alt=\"&lt;标题&gt;\""));
}

/// 成功记录只复用完全相同的任务、包、连接器修订和格式。
#[test]
fn sync_record_idempotency_scope_is_exact() {
    let record = DocumentSyncRecord {
        task_id: "task".into(),
        package_version: 2,
        connector_id: "feishu".into(),
        connector_revision: 4,
        format: DocumentFormat::Markdown,
        file_token: "file".into(),
        file_name: "article-v2.md".into(),
        synced_at_ms: 10,
    };
    assert!(record.matches_request("task", 2, "feishu", 4, DocumentFormat::Markdown));
    assert!(!record.matches_request("task", 2, "feishu", 4, DocumentFormat::Html));
    assert_eq!(
        document_sync_file_name("标题/测试", "task", 2, DocumentFormat::Markdown),
        "标题_测试-v2.md"
    );
}
