//! 原创草稿的本地输出与既有完整文章工作流衔接。

use super::*;
use crate::{ArticleTask, ArtifactLineage, WorkspaceData};

/// 从已保存草稿构造本地导出内容；格式规则由 core 拥有。
pub fn research_draft_document(data: &ResearchData, id: &str, format: &str) -> Result<(String, &'static str), String> {
    let draft = data.drafts.iter().find(|draft| draft.id == id).ok_or("草稿不存在")?;
    let tags = draft.tags.iter().map(|tag| format!("#{tag}")).collect::<Vec<_>>().join(" ");
    match format {
        "markdown" => Ok((format!("# {}\n\n{}\n\n{tags}\n", draft.title, draft.body), "md")),
        "text" => Ok((format!("{}\n\n{}\n\n{tags}\n", draft.title, draft.body), "txt")),
        "html" => {
            // 原始正文以文本输出，避免可分享文件执行模型输出中的脚本或远程资源。
            let escape = |text: &str| text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
            Ok((format!("<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><body><article><h1>{}</h1><div style=\"white-space:pre-wrap;line-height:1.8\">{}</div><p>{}</p></article></body></html>", escape(&draft.title), escape(&draft.title), escape(&draft.body), escape(&tags)), "html"))
        }
        _ => Err("导出格式不支持".into()),
    }
}

/// 将真实草稿正文送入原文章任务继续简介、配图、排版与飞书同步，不重跑正文 AI。
pub fn prepare_research_article(data: &mut WorkspaceData, id: &str, now_ms: i64) -> Result<String, String> {
    let draft = data.research.drafts.iter().find(|draft| draft.id == id).ok_or("草稿不存在")?;
    let task_id = format!("research-draft-{}", draft.id);
    if let Some(existing) = data.articles.iter().find(|article| article.id == task_id) {
        if existing.title != draft.title || existing.artifacts.markdown.as_deref() != Some(draft.body.as_str()) {
            return Err("该草稿已转入批量写作并有不同内容，请在已有文章中继续编辑".into());
        }
        return Ok(task_id);
    }
    let mut article = ArticleTask::new(task_id.clone(), draft.title.clone(), now_ms);
    article.keywords = draft.tags.clone();
    article.requirements = draft.keyword.clone();
    article.edit_article(draft.body.clone(), ArtifactLineage {
        package_version: 1, source_article_version: None, source_summary_version: None,
        template_id: None, template_version: None,
        provider_id: Some(draft.provider_id.clone()), provider_revision: None,
        model: Some(draft.model.clone()), attempt_id: format!("research-draft:{}", draft.id),
    }, now_ms).map_err(|_| "草稿无法转入文章工作台")?;
    data.articles.insert(0, article);
    Ok(task_id)
}
