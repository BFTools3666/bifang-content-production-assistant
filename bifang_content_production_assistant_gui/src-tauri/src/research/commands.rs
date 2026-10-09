use std::sync::Arc;

use bifang_content_production_assistant_core::{
    TextGenerationRequest, TextProbeSelection, WorkflowStage, select_text_probe,
    research::*,
};
use tauri::{AppHandle, State};

use crate::workbench::{
    WorkspaceRepository,
    models::CommandError,
    provider::{CredentialVault, OpenAiCompatibleClient},
    store::current_time_ms,
};

use super::ResearchRuntime;

/// 读取原有SQLite中的对标资产，不访问凭据或外部网络。
#[tauri::command]
pub(crate) async fn get_research_library(repository: State<'_, Arc<WorkspaceRepository>>) -> Result<ResearchData, CommandError> {
    Ok(repository.snapshot().await.research)
}

/// 保存用户修改的内置提示词，业务校验由core拥有。
#[tauri::command]
pub(crate) async fn update_research_prompts(prompts: ResearchPrompts, repository: State<'_, Arc<WorkspaceRepository>>) -> Result<(), CommandError> {
    repository.update(|data| save_research_prompts(&mut data.research, prompts)).await.map_err(research_error)
}

/// 基于原有AI配置发起一次真实文本请求，不自动重试可能计费的调用。
async fn generate_text(repository: &WorkspaceRepository, provider_id: &str, prompt: &str) -> Result<(String, bifang_content_production_assistant_core::TextGenerationResult), CommandError> {
    let data = repository.snapshot().await;
    let plan = select_text_probe(&data, provider_id, WorkflowStage::Article, false).map_err(|error| CommandError::new(error.code(), error.message()))?;
    let TextProbeSelection::Request(plan) = plan else { return Err(research_error("请配置真实的文本AI服务".into())); };
    if data.pending_credential_deletions.contains(&plan.credential_ref) { return Err(research_error("当前AI密钥正在清理，请重新保存服务配置".into())); }
    let key = CredentialVault.get_api_key(&plan.credential_ref).map_err(|error| CommandError::new("PROVIDER_NOT_CONFIGURED", error.safe_message))?;
    let client = OpenAiCompatibleClient::new().map_err(research_error)?;
    let request = TextGenerationRequest {
        model: plan.model.clone(),
        prompt: prompt.into(),
        parameters: plan.provider.article_parameters(),
        idempotency_key: format!("research:{}:{}", plan.provider.id, uuid::Uuid::new_v4()),
    };
    let result = client.generate_text(&plan.provider, &key, &request).await.map_err(|error| CommandError::new("RESEARCH_AI_FAILED", error.safe_message))?;
    Ok((plan.model, result))
}

/// 仅生成标题候选，用户确认后才由既有添加标题用例建立文章任务。
#[tauri::command]
pub(crate) async fn generate_article_titles(input: TitleGenerationInput, provider_id: String, repository: State<'_, Arc<WorkspaceRepository>>, runtime: State<'_, ResearchRuntime>) -> Result<GeneratedTitles, CommandError> {
    let _guard = runtime.generation.try_lock().map_err(|_| research_error("已有生成正在进行，请等待结果".into()))?;
    let prompt = build_title_generation_prompt(&input).map_err(research_error)?;
    let (_, result) = generate_text(&repository, &provider_id, &prompt).await?;
    parse_generated_titles(&result.text, &input).map_err(research_error)
}

/// 生成二次标题和原创正文，结构通过校验后才原子保存草稿。
#[tauri::command]
pub(crate) async fn generate_research_content(input: ContentGenerationInput, provider_id: String, repository: State<'_, Arc<WorkspaceRepository>>, runtime: State<'_, ResearchRuntime>) -> Result<ResearchDraft, CommandError> {
    let _guard = runtime.generation.try_lock().map_err(|_| research_error("已有生成正在进行，请等待结果".into()))?;
    let data = repository.snapshot().await;
    let prompt = build_content_generation_prompt(&data.research, &input).map_err(research_error)?;
    let (model, result) = generate_text(&repository, &provider_id, &prompt).await?;
    let content = parse_generated_content(&result.text, input.platform).map_err(research_error)?;
    let draft = ResearchDraft {
        id: uuid::Uuid::new_v4().to_string(), platform: input.platform, keyword: input.keyword,
        source_ids: input.source_ids, style_id: input.style_id, title: content.titles[0].clone(),
        titles: content.titles, body: content.body, tags: content.tags, prompt, provider_id,
        model, token_usage: result.token_usage, created_at_ms: current_time_ms(),
    };
    repository.update(|data| save_research_draft(&mut data.research, draft.clone())).await.map_err(research_error)?;
    Ok(draft)
}

/// 从同一公开主页的完整作品分析风格，再保存可修改的风格指令。
#[tauri::command]
pub(crate) async fn analyze_research_style(platform: ContentPlatform, homepage_url: String, source_ids: Vec<String>, name: String, provider_id: String, repository: State<'_, Arc<WorkspaceRepository>>, runtime: State<'_, ResearchRuntime>) -> Result<StyleProfile, CommandError> {
    let _guard = runtime.generation.try_lock().map_err(|_| research_error("已有生成正在进行，请等待结果".into()))?;
    let data = repository.snapshot().await;
    if name.trim().is_empty() || name.chars().count() > 100 { return Err(research_error("请填写100字以内的风格名称".into())); }
    let prompt = build_style_analysis_prompt(&data.research, platform, &homepage_url, &source_ids).map_err(research_error)?;
    let samples = style_analysis_samples(&data.research, platform, &homepage_url, &source_ids).map_err(research_error)?.into_iter().cloned().collect::<Vec<_>>();
    let (_, result) = generate_text(&repository, &provider_id, &prompt).await?;
    if result.text.trim().is_empty() || result.text.chars().count() > 20_000 { return Err(research_error("AI未返回有效风格分析".into())); }
    let now = current_time_ms();
    let style = StyleProfile { id: uuid::Uuid::new_v4().to_string(), name, platform, author: samples[0].author.clone(), homepage_url, samples, analysis: result.text.clone(), instructions: result.text, revision: 1, created_at_ms: now, updated_at_ms: now };
    repository.update(|data| save_style_profile(&mut data.research, style)).await.map_err(research_error)
}

/// 保存用户编辑的风格指令，并保留原样本和AI分析。
#[tauri::command]
pub(crate) async fn update_research_style(mut style: StyleProfile, repository: State<'_, Arc<WorkspaceRepository>>) -> Result<StyleProfile, CommandError> {
    style.updated_at_ms = current_time_ms();
    repository.update(|data| {
        let old = data.research.styles.iter().find(|item| item.id == style.id).ok_or("风格不存在")?;
        style.samples = old.samples.clone(); style.analysis = old.analysis.clone();
        style.homepage_url = old.homepage_url.clone(); style.platform = old.platform; style.author = old.author.clone();
        save_style_profile(&mut data.research, style)
    }).await.map_err(research_error)
}

/// 保存人工修改的标题正文，保留不可变的生成血缘。
#[tauri::command]
pub(crate) async fn update_research_draft(id: String, title: String, body: String, repository: State<'_, Arc<WorkspaceRepository>>) -> Result<(), CommandError> {
    repository.update(|data| {
        let mut draft = data.research.drafts.iter().find(|draft| draft.id == id).cloned().ok_or("草稿不存在")?;
        draft.title = title; draft.body = body;
        save_research_draft(&mut data.research, draft)
    }).await.map_err(research_error)
}

/// 用用户选择的系统保存对话框导出原创草稿；只写用户指定文件。
#[tauri::command]
pub(crate) async fn export_research_draft(app: AppHandle, id: String, format: String, repository: State<'_, Arc<WorkspaceRepository>>) -> Result<bool, CommandError> {
    use tauri_plugin_dialog::DialogExt;
    let data = repository.snapshot().await;
    let (text, extension) = research_draft_document(&data.research, &id, &format).map_err(research_error)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().add_filter("内容", &[extension]).set_file_name(format!("毕方原创草稿.{extension}")).save_file(move |path| { let _ = sender.send(path); });
    let path = receiver.await.map_err(|_| research_error("保存对话框无法打开".into()))?;
    let Some(path) = path else { return Ok(false); };
    let path = path.into_path().map_err(|_| research_error("请选择本地保存路径".into()))?;
    tokio::fs::write(path, text).await.map_err(|_| research_error("导出失败，请检查文件权限".into()))?;
    Ok(true)
}

/// 把领域拒绝映射为可展示的稳定协议错误，不输出原始外部响应。
fn research_error(message: String) -> CommandError { CommandError::new("RESEARCH_VALIDATION", message) }

/// 将已保存草稿交给共享 core，继续原有配图、格式和连接器工作流。
#[tauri::command]
pub(crate) async fn prepare_research_article_task(id: String, repository: State<'_ , Arc<WorkspaceRepository>>) -> Result<String, CommandError> {
    repository.update(|data| prepare_research_article(data, &id, current_time_ms())).await.map_err(research_error)
}
