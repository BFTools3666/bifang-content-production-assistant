//! 文章只读结果的人类文本渲染。

use bifang_content_production_assistant_core::{ArticleDetail, ArticleList};

/// 把文章列表渲染为紧凑人类文本，不改变 core 排序或状态语义。
pub(crate) fn render_human_article_list(list: &ArticleList) -> String {
    let mut lines = vec![format!("共 {} 篇文章", list.total)];
    lines.extend(list.items.iter().map(|article| {
        format!(
            "{} | {} | {:?} | 包版本 {} | 创建 {} | 更新 {}",
            article.id,
            article.title,
            article.overall_status,
            article.package_version,
            article.created_at_ms,
            article.updated_at_ms
        )
    }));
    lines.join("\n")
}

/// 把单篇安全投影渲染为人类文本；图片仅显示存在性，不显示本地引用。
pub(crate) fn render_human_article_detail(detail: &ArticleDetail) -> String {
    let article = &detail.article;
    let keywords = if article.keywords.is_empty() {
        "无".to_owned()
    } else {
        article.keywords.join("、")
    };
    [
        format!("{} | {}", article.id, article.title),
        format!(
            "状态：{:?} | 审核：{:?} | 包版本 {} | 行版本 {}",
            article.overall_status,
            article.review_status,
            article.package_version,
            article.row_version
        ),
        format!("关键词：{keywords}"),
        format!("内容要求：{}", article.requirements),
        format!(
            "配图：{} | AI 原图：{} | 编辑修订：{}",
            if detail.artifacts.has_image {
                "有"
            } else {
                "无"
            },
            if detail.artifacts.has_original_image {
                "有"
            } else {
                "无"
            },
            detail.artifacts.image_edit_revision
        ),
        format!(
            "历史：{} 次尝试 | {} 个批准包 | {} 个包快照",
            detail.history.attempt_count,
            detail.history.approved_package_count,
            detail.history.package_snapshot_count
        ),
        format!(
            "Markdown：{}",
            detail.artifacts.markdown.as_deref().unwrap_or("未生成")
        ),
        format!(
            "简介：{}",
            detail.artifacts.summary.as_deref().unwrap_or("未生成")
        ),
        format!(
            "HTML：{}",
            detail.artifacts.html.as_deref().unwrap_or("未生成")
        ),
    ]
    .join("\n")
}
