//! 文章只读查询的接口无关用例、投影与稳定错误。

use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};

use crate::{ArticleTask, OverallStatus, ReviewStatus, StageStates};

/// 当前可被产品适配器读取的工作区快照模式版本。
pub const CURRENT_WORKSPACE_SCHEMA_VERSION: u32 = 12;

/// CLI 等只读适配器从持久层载入的最窄工作区快照。
///
/// Serde 会忽略 GUI 工作区中的其它字段，避免 core 反向依赖桌面适配器模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleWorkspaceSnapshot {
    /// 持久化工作区的模式版本。
    pub schema_version: u32,
    /// 工作区中的全部文章任务。
    pub articles: Vec<ArticleTask>,
}

/// 文章只读仓储一次载入操作的异步返回值。
pub type ArticleReadFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<ArticleTask>, ArticleReadError>> + Send + 'a>>;

/// 为文章只读用例提供持久层数据的最窄端口。
pub trait ArticleReadRepository: Sync {
    /// 只读载入全部文章，不迁移、恢复、规范化或保存工作区。
    fn load_articles(&self) -> ArticleReadFuture<'_>;
}

/// 文章只读查询可以稳定映射到协议与进程退出码的失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticleReadError {
    /// 应用数据目录或数据库路径不满足只读边界。
    WorkspacePathInvalid,
    /// 指定的工作区数据库或快照不存在。
    WorkspaceNotFound,
    /// 工作区无法安全打开或读取。
    WorkspaceUnavailable,
    /// 工作区快照结构或内容无效。
    WorkspaceInvalid,
    /// 工作区模式版本不是当前产品版本。
    WorkspaceSchemaUnsupported,
    /// 文章稳定 ID 为空。
    ArticleIdInvalid,
    /// 工作区中不存在精确匹配的文章稳定 ID。
    ArticleNotFound,
}

impl ArticleReadError {
    /// 返回供 Agent 分支处理的稳定机器错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::WorkspacePathInvalid => "WORKSPACE_PATH_INVALID",
            Self::WorkspaceNotFound => "WORKSPACE_NOT_FOUND",
            Self::WorkspaceUnavailable => "WORKSPACE_UNAVAILABLE",
            Self::WorkspaceInvalid => "WORKSPACE_INVALID",
            Self::WorkspaceSchemaUnsupported => "WORKSPACE_SCHEMA_UNSUPPORTED",
            Self::ArticleIdInvalid => "ARTICLE_ID_INVALID",
            Self::ArticleNotFound => "ARTICLE_NOT_FOUND",
        }
    }

    /// 返回不包含本机路径、数据库正文或其它敏感材料的诊断说明。
    pub const fn message(self) -> &'static str {
        match self {
            Self::WorkspacePathInvalid => {
                "应用数据目录必须是现有的绝对非符号链接目录，且工作区数据库必须是其中的普通文件"
            }
            Self::WorkspaceNotFound => "找不到已保存的工作区数据库或快照",
            Self::WorkspaceUnavailable => "无法以只读方式打开或读取工作区数据库",
            Self::WorkspaceInvalid => "工作区快照格式无效",
            Self::WorkspaceSchemaUnsupported => {
                "工作区版本与当前 CLI 不一致，请先启动一次当前桌面应用"
            }
            Self::ArticleIdInvalid => "文章 ID 不能为空",
            Self::ArticleNotFound => "找不到指定文章",
        }
    }
}

/// `article list` 返回的完整分页前只读结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleList {
    /// 按产品默认顺序排列的文章摘要。
    pub items: Vec<ArticleListItem>,
    /// 当前工作区中的文章总数。
    pub total: usize,
}

/// 一篇文章在只读列表中的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleListItem {
    /// 文章稳定 ID。
    pub id: String,
    /// 用户输入的文章标题。
    pub title: String,
    /// 按录入顺序保留的关键词。
    pub keywords: Vec<String>,
    /// 行级内容要求。
    pub requirements: String,
    /// 当前文章包版本。
    pub package_version: u32,
    /// 当前乐观锁行版本。
    pub row_version: u64,
    /// 由 core 推导的总体状态。
    pub overall_status: OverallStatus,
    /// 当前人工审核状态。
    pub review_status: ReviewStatus,
    /// 四个固定 SOP 阶段的当前状态。
    pub stages: StageStates,
    /// 文章首次创建时间，Unix 毫秒。
    pub created_at_ms: i64,
    /// 最近一次领域变化时间，Unix 毫秒。
    pub updated_at_ms: i64,
}

/// `article get` 返回的一篇文章安全详情。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleDetail {
    /// 与列表相同的身份、输入、版本和状态摘要。
    pub article: ArticleListItem,
    /// 当前文章包中不含本地引用的可读产物。
    pub artifacts: ArticleReadArtifacts,
    /// 只暴露数量、不返回历史正文或本地引用的历史摘要。
    pub history: ArticleHistoryCounts,
}

/// 当前文章包中可以安全离开持久层的文本产物与图片存在性。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleReadArtifacts {
    /// 当前 Markdown 正文。
    pub markdown: Option<String>,
    /// 当前文章简介。
    pub summary: Option<String>,
    /// 当前派生 HTML。
    pub html: Option<String>,
    /// 当前文章包是否有配图；不会返回配图路径。
    pub has_image: bool,
    /// 当前文章包是否保留 AI 原图；不会返回原图路径。
    pub has_original_image: bool,
    /// 当前人工配图编辑修订号。
    pub image_edit_revision: u64,
}

/// 不暴露历史正文、图片引用或 Provider 细节的历史数量摘要。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleHistoryCounts {
    /// 已持久化的阶段尝试数量。
    pub attempt_count: usize,
    /// 不可变已批准文章包数量。
    pub approved_package_count: usize,
    /// 被新版本替换过的文章包快照数量。
    pub package_snapshot_count: usize,
}

/// 按 GUI 相同的默认顺序列出全部文章，并投影为安全摘要。
pub async fn list_articles(
    repository: &impl ArticleReadRepository,
) -> Result<ArticleList, ArticleReadError> {
    let mut articles = repository.load_articles().await?;
    articles.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    let total = articles.len();
    Ok(ArticleList {
        items: articles.into_iter().map(ArticleListItem::from).collect(),
        total,
    })
}

/// 按大小写敏感的稳定 ID 精确查找一篇文章，并返回脱敏详情。
pub async fn get_article(
    repository: &impl ArticleReadRepository,
    article_id: &str,
) -> Result<ArticleDetail, ArticleReadError> {
    validate_article_id(article_id)?;
    let article = repository
        .load_articles()
        .await?
        .into_iter()
        .find(|article| article.id == article_id)
        .ok_or(ArticleReadError::ArticleNotFound)?;
    Ok(ArticleDetail::from(article))
}

/// 在任何持久层访问前校验文章稳定 ID 的最小值域。
pub fn validate_article_id(article_id: &str) -> Result<(), ArticleReadError> {
    if article_id.trim().is_empty() {
        Err(ArticleReadError::ArticleIdInvalid)
    } else {
        Ok(())
    }
}

impl From<ArticleTask> for ArticleListItem {
    fn from(article: ArticleTask) -> Self {
        let overall_status = article.overall_status();
        Self {
            id: article.id,
            title: article.title,
            keywords: article.keywords,
            requirements: article.requirements,
            package_version: article.package_version,
            row_version: article.row_version,
            overall_status,
            review_status: article.review_status,
            stages: article.stages,
            created_at_ms: article.created_at_ms,
            updated_at_ms: article.updated_at_ms,
        }
    }
}

impl From<ArticleTask> for ArticleDetail {
    fn from(article: ArticleTask) -> Self {
        let artifacts = ArticleReadArtifacts {
            markdown: article.artifacts.markdown.clone(),
            summary: article.artifacts.summary.clone(),
            html: article.artifacts.html.clone(),
            has_image: article.artifacts.image_ref.is_some(),
            has_original_image: article.artifacts.original_image_ref.is_some(),
            image_edit_revision: article.artifacts.image_edit_revision,
        };
        let history = ArticleHistoryCounts {
            attempt_count: article.attempt_history.len(),
            approved_package_count: article.approved_history.len(),
            package_snapshot_count: article.package_history.len(),
        };
        Self {
            article: ArticleListItem::from(article),
            artifacts,
            history,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApprovedPackage, ArticleArtifacts, ArticlePackageSnapshot};

    struct FakeRepository {
        articles: Vec<ArticleTask>,
    }

    impl ArticleReadRepository for FakeRepository {
        fn load_articles(&self) -> ArticleReadFuture<'_> {
            Box::pin(async { Ok(self.articles.clone()) })
        }
    }

    #[tokio::test]
    async fn lists_articles_in_created_time_then_id_descending_order() {
        let mut oldest = ArticleTask::new("article-z", "旧文章", 10);
        oldest.created_at_ms = 10;
        let mut same_time_a = ArticleTask::new("article-a", "同刻 A", 20);
        same_time_a.created_at_ms = 20;
        let mut same_time_z = ArticleTask::new("article-z2", "同刻 Z", 20);
        same_time_z.created_at_ms = 20;
        let repository = FakeRepository {
            articles: vec![oldest, same_time_a, same_time_z],
        };

        let result = list_articles(&repository).await.unwrap();

        assert_eq!(result.total, 3);
        assert_eq!(
            result
                .items
                .iter()
                .map(|article| article.id.as_str())
                .collect::<Vec<_>>(),
            vec!["article-z2", "article-a", "article-z"]
        );
    }

    #[tokio::test]
    async fn gets_text_artifacts_without_serializing_any_local_image_reference() {
        let mut article = ArticleTask::new("article-1", "安全详情", 20);
        article.artifacts.markdown = Some("正文".into());
        article.artifacts.summary = Some("简介".into());
        article.artifacts.html = Some("<p>正文</p>".into());
        article.artifacts.image_ref = Some("/private/current.png".into());
        article.artifacts.original_image_ref = Some("/private/original.png".into());
        article.approved_history.push(ApprovedPackage {
            version: 1,
            title: "旧标题".into(),
            keywords: Vec::new(),
            requirements: String::new(),
            artifacts: ArticleArtifacts {
                image_ref: Some("/private/approved.png".into()),
                ..ArticleArtifacts::default()
            },
            approved_at_ms: 10,
        });
        article.package_history.push(ArticlePackageSnapshot {
            version: 1,
            title: "旧标题".into(),
            keywords: Vec::new(),
            requirements: String::new(),
            review_status: ReviewStatus::Draft,
            artifacts: ArticleArtifacts {
                original_image_ref: Some("/private/history.png".into()),
                ..ArticleArtifacts::default()
            },
            captured_at_ms: 10,
        });
        let repository = FakeRepository {
            articles: vec![article],
        };

        let detail = get_article(&repository, "article-1").await.unwrap();
        let encoded = serde_json::to_string(&detail).unwrap();

        assert_eq!(detail.artifacts.markdown.as_deref(), Some("正文"));
        assert!(detail.artifacts.has_image);
        assert!(detail.artifacts.has_original_image);
        assert_eq!(detail.history.approved_package_count, 1);
        assert_eq!(detail.history.package_snapshot_count, 1);
        assert!(!encoded.contains("/private/"));
        assert!(!encoded.contains("imageRef"));
        assert!(!encoded.contains("originalImageRef"));
    }

    #[tokio::test]
    async fn rejects_blank_and_missing_article_ids_with_stable_errors() {
        let repository = FakeRepository { articles: vec![] };

        assert_eq!(
            get_article(&repository, "  ").await,
            Err(ArticleReadError::ArticleIdInvalid)
        );
        assert_eq!(
            get_article(&repository, "missing").await,
            Err(ArticleReadError::ArticleNotFound)
        );
    }
}
