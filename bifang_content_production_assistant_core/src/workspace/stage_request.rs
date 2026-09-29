//! 单阶段执行的乐观锁、付费确认与默认服务快照校验。

use super::{WorkflowStage, WorkspaceData, default_provider_for_stage};

/// 单阶段请求的稳定领域拒绝原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageRequestError {
    /// 指定文章不存在。
    ArticleNotFound,
    /// 行或文章包已变化。
    RevisionConflict,
    /// 图片生成缺少本次显式付费确认。
    ImageConfirmationRequired,
    /// 本次确认所见图片 Provider 与当前默认项不一致。
    ProviderChanged,
}

impl StageRequestError {
    /// 稳定机器错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::ArticleNotFound => "STAGE_GENERATE_FAILED",
            Self::RevisionConflict => "ARTICLE_REVISION_CONFLICT",
            Self::ImageConfirmationRequired => "STAGE_IMAGE_CONFIRMATION_REQUIRED",
            Self::ProviderChanged => "STAGE_PROVIDER_CHANGED",
        }
    }

    /// 不含用户内容或凭据的安全提示。
    pub const fn message(self) -> &'static str {
        match self {
            Self::ArticleNotFound => "找不到指定文章",
            Self::RevisionConflict => "文章状态已变化，请刷新后重新确认",
            Self::ImageConfirmationRequired => "生成配图可能产生服务费用，请确认后再运行",
            Self::ProviderChanged => "配图 AI 服务已变化，请重新查看并确认当前服务",
        }
    }
}

/// 在加载凭据或修改工作区前核对 Grid 快照与当前权威状态。
#[allow(clippy::too_many_arguments)]
pub fn validate_single_stage_request(
    workspace: &WorkspaceData,
    task_id: &str,
    stage: WorkflowStage,
    expected_row_version: u64,
    expected_package_version: u32,
    confirmed: bool,
    expected_provider_id: Option<&str>,
    expected_provider_revision: Option<u32>,
) -> Result<(), StageRequestError> {
    let article = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .ok_or(StageRequestError::ArticleNotFound)?;
    if article.row_version != expected_row_version
        || article.package_version != expected_package_version
    {
        return Err(StageRequestError::RevisionConflict);
    }
    if stage == WorkflowStage::Image {
        if !confirmed {
            return Err(StageRequestError::ImageConfirmationRequired);
        }
        let current_provider = default_provider_for_stage(workspace, stage);
        let provider_matches = current_provider.is_some_and(|provider| {
            expected_provider_id == Some(provider.id.as_str())
                && expected_provider_revision == Some(provider.revision)
        });
        if !provider_matches {
            return Err(StageRequestError::ProviderChanged);
        }
    }
    Ok(())
}
