use super::*;

#[test]
fn image_stage_requires_same_row_package_provider_and_confirmation() {
    let mut workspace = WorkspaceData::default();
    workspace.articles.push(ArticleTask::new("task", "标题", 1));
    let provider = fixture_provider("image");
    workspace.provider_configs.push(provider.clone());
    let article = &workspace.articles[0];
    let (row, package) = (article.row_version, article.package_version);
    let verify = |workspace: &WorkspaceData, confirmed, expected_revision| {
        validate_single_stage_request(
            workspace,
            "task",
            WorkflowStage::Image,
            row,
            package,
            confirmed,
            Some("image"),
            expected_revision,
        )
    };

    assert_eq!(
        verify(&workspace, false, Some(provider.revision)),
        Err(StageRequestError::ImageConfirmationRequired)
    );
    assert_eq!(
        verify(&workspace, true, Some(provider.revision + 1)),
        Err(StageRequestError::ProviderChanged)
    );
    assert_eq!(verify(&workspace, true, Some(provider.revision)), Ok(()));
    workspace.articles[0].row_version += 1;
    assert_eq!(
        verify(&workspace, true, Some(provider.revision)),
        Err(StageRequestError::RevisionConflict)
    );
}

#[test]
fn missing_article_and_non_image_stage_fail_without_side_effects() {
    let mut workspace = WorkspaceData::default();
    let before = workspace.clone();
    assert_eq!(
        validate_single_stage_request(
            &workspace,
            "missing",
            WorkflowStage::Article,
            1,
            1,
            false,
            None,
            None,
        ),
        Err(StageRequestError::ArticleNotFound)
    );
    workspace.articles.push(ArticleTask::new("task", "标题", 1));
    let article = &workspace.articles[0];
    assert_eq!(
        validate_single_stage_request(
            &workspace,
            "task",
            WorkflowStage::Article,
            article.row_version,
            article.package_version,
            false,
            None,
            None,
        ),
        Ok(())
    );
    assert_eq!(before.articles.len() + 1, workspace.articles.len());
}
