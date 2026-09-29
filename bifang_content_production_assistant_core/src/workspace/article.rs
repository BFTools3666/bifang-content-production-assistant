//! 工作区文章录入、人工血缘与删除后的凭据引用规则。

use std::collections::BTreeSet;

use crate::{ArticleTask, ArtifactLineage, OverallStatus, ReviewStatus};

use super::WorkspaceData;

const MAX_TITLES_PER_ADD: usize = 200;
const MAX_TITLE_CHARS: usize = 300;

/// 批量标题输入的稳定领域拒绝原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArticleTitleError {
    /// 输入没有非空标题。
    Empty,
    /// 单次标题数量超过上限。
    TooMany,
    /// 任一标题超过字符上限。
    TooLong,
}

impl ArticleTitleError {
    /// 返回协议适配器可映射的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::Empty => "EMPTY_TITLES",
            Self::TooMany => "TOO_MANY_TITLES",
            Self::TooLong => "TITLE_TOO_LONG",
        }
    }

    /// 返回不含用户输入的安全提示。
    pub fn message(self) -> String {
        match self {
            Self::Empty => "请至少输入一个文章标题".into(),
            Self::TooMany => format!("一次最多添加 {MAX_TITLES_PER_ADD} 个标题"),
            Self::TooLong => format!("单个标题最多 {MAX_TITLE_CHARS} 个字符"),
        }
    }
}

/// 已新增任务数量、重复提示及新任务身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedArticleTitles {
    /// 实际创建的任务数。
    pub created_count: usize,
    /// 输入中被忽略的空白行数。
    pub ignored_blank_count: usize,
    /// 与既有或同批标题相同但仍被保留的任务数。
    pub duplicate_count: usize,
    /// 新任务的稳定 ID，保留输入顺序。
    pub task_ids: Vec<String>,
}

/// 校验并标准化批量标题文本；只忽略空白行，重复标题仍保留。
pub fn parse_article_titles(titles: &str) -> Result<(Vec<String>, usize), ArticleTitleError> {
    let lines = titles.lines().map(str::trim).collect::<Vec<_>>();
    let ignored_blank_count = lines.iter().filter(|line| line.is_empty()).count();
    let non_blank = lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if non_blank.is_empty() {
        return Err(ArticleTitleError::Empty);
    }
    if non_blank.len() > MAX_TITLES_PER_ADD {
        return Err(ArticleTitleError::TooMany);
    }
    if non_blank
        .iter()
        .any(|title| title.chars().count() > MAX_TITLE_CHARS)
    {
        return Err(ArticleTitleError::TooLong);
    }
    Ok((non_blank, ignored_blank_count))
}

/// 将已经校验的标题追加为独立任务，由适配器只提供新 ID 和时间。
pub fn append_article_titles(
    workspace: &mut WorkspaceData,
    titles: Vec<String>,
    ignored_blank_count: usize,
    now_ms: i64,
    mut next_id: impl FnMut() -> String,
) -> AddedArticleTitles {
    let mut seen = workspace
        .articles
        .iter()
        .map(|article| article.title.trim().to_lowercase())
        .collect::<BTreeSet<_>>();
    let mut duplicate_count = 0;
    let mut task_ids = Vec::new();
    for title in titles {
        let normalized = title.to_lowercase();
        if !seen.insert(normalized) {
            duplicate_count += 1;
        }
        let task_id = next_id();
        workspace
            .articles
            .push(ArticleTask::new(&task_id, title, now_ms));
        task_ids.push(task_id);
    }
    AddedArticleTitles {
        created_count: task_ids.len(),
        ignored_blank_count,
        duplicate_count,
        task_ids,
    }
}

/// 为人工编辑构造安全血缘，调用方只提供新的尝试 ID。
pub fn manual_lineage(
    article: &ArticleTask,
    depends_on_article: bool,
    attempt_id: String,
) -> ArtifactLineage {
    let next_package_version = if article.review_status == ReviewStatus::Approved {
        article.package_version.saturating_add(1)
    } else {
        article.package_version
    };
    ArtifactLineage {
        package_version: next_package_version,
        source_article_version: depends_on_article
            .then_some(article.stages.article.artifact_version)
            .flatten(),
        source_summary_version: None,
        template_id: None,
        template_version: None,
        provider_id: None,
        provider_revision: None,
        model: None,
        attempt_id,
    }
}

/// 删除文章及其冻结绑定，并登记最后一个引用已经消失的凭据条目。
pub fn remove_article_and_collect_unused_credentials(
    workspace: &mut WorkspaceData,
    task_id: &str,
) -> Result<Vec<String>, String> {
    let position = workspace
        .articles
        .iter()
        .position(|article| article.id == task_id)
        .ok_or_else(|| "找不到指定文章".to_string())?;
    if matches!(
        workspace.articles[position].overall_status(),
        OverallStatus::Queued | OverallStatus::Running
    ) {
        return Err("运行中的文章不能删除".to_string());
    }
    let removed_refs = workspace
        .execution_bindings
        .iter()
        .filter(|binding| binding.task_id == task_id)
        .flat_map(|binding| {
            [
                binding.article_provider.as_ref(),
                binding.summary_provider.as_ref(),
                binding.image_provider.as_ref(),
            ]
            .into_iter()
            .flatten()
            .filter_map(|provider| provider.credential_ref.clone())
        })
        .collect::<BTreeSet<_>>();
    workspace.articles.remove(position);
    workspace
        .execution_bindings
        .retain(|binding| binding.task_id != task_id);
    let retained_refs = workspace.referenced_credential_refs();
    let unused = removed_refs
        .difference(&retained_refs)
        .cloned()
        .collect::<Vec<_>>();
    workspace
        .pending_credential_deletions
        .extend(unused.iter().cloned());
    Ok(unused)
}
