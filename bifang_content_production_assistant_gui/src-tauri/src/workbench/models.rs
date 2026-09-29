//! Tauri 工作台适配器使用的 IPC 视图和显式环境开关。

use bifang_content_production_assistant_core::{
    ArticleTask, OverallStatus, PromptTemplate, SkippedTask,
};
pub use bifang_content_production_assistant_core::{
    ImageRequestContract, PackageExecutionBinding, ProviderConfig, ProviderExecutionBinding,
    ProviderKind, RunRecord, RunStatus, WORKSPACE_SCHEMA_VERSION, WorkspaceData,
};
use serde::{Deserialize, Serialize};

/// 供 GUI 展示的 Provider 状态，不返回密钥内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigView {
    /// 非秘密 Provider 配置。
    #[serde(flatten)]
    pub config: ProviderConfig,
    /// 系统凭据库中是否存在密钥。
    pub has_api_key: bool,
    /// 是否为显式 E2E 测试夹具。
    pub is_test_fixture: bool,
}

/// 保存一个 Provider 后返回的完整默认位视图，避免 GUI 暂时显示互斥默认项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigSaveResult {
    /// 已创建或修订的目标 Provider。
    pub saved: ProviderConfigView,
    /// 同一原子更新后的全部当前 Provider，包括被挤下默认位的新修订。
    pub providers: Vec<ProviderConfigView>,
}

/// 安全删除一个 Provider 后返回的当前集合和凭据清理状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigDeleteResult {
    /// 删除完成后的全部当前 Provider。
    pub providers: Vec<ProviderConfigView>,
    /// 系统凭据库清理是否仍在可重试队列中等待完成。
    pub credential_cleanup_pending: bool,
    /// 已保留非秘密快照并受本次删除影响的历史阶段绑定数量。
    pub historical_bindings_detached: u32,
}

/// 用户显式测试 Provider 后返回的安全结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnectionResult {
    /// 标识测试请求是否成功得到非空文本。
    pub ok: bool,
    /// 不含密钥和原始响应的说明。
    pub message: String,
}

/// 用户显式确认后生成、仅供当前前端内存展示的图片测试结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderImageTestResult {
    /// 测试所使用的稳定服务 ID。
    pub provider_id: String,
    /// 测试所使用的已保存配置修订。
    pub provider_revision: u32,
    /// 从图片魔数识别出的受支持 MIME。
    pub mime_type: String,
    /// Base64 解码后的真实字节数。
    pub byte_length: u64,
    /// 不含远端 URL 或本地路径的临时内存预览。
    pub image_data_url: String,
}

/// 返回当前进程是否明确启用了调试 E2E 测试夹具。
pub fn fixture_mode_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("BIFANG_CONTENT_PRODUCTION_ASSISTANT_E2E_FIXTURE")
            .is_ok_and(|value| value == "1")
}

/// 一行文章及其由 core 推导的总体状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleView {
    /// 完整文章任务。
    pub article: ArticleTask,
    /// 由 core 推导的总体状态。
    pub overall_status: OverallStatus,
    /// 当前文章是否已有受管配图；真实文件引用不会发送给 WebView。
    pub has_image: bool,
}

impl From<ArticleTask> for ArticleView {
    fn from(mut article: ArticleTask) -> Self {
        let overall_status = article.overall_status();
        let has_image = article.artifacts.image_ref.is_some();
        article.artifacts.image_ref = None;
        article.artifacts.original_image_ref = None;
        for approved in &mut article.approved_history {
            approved.artifacts.image_ref = None;
            approved.artifacts.original_image_ref = None;
        }
        for snapshot in &mut article.package_history {
            snapshot.artifacts.image_ref = None;
            snapshot.artifacts.original_image_ref = None;
        }
        Self {
            article,
            overall_status,
            has_image,
        }
    }
}

/// 工作台首页一次查询所需的完整视图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchView {
    /// Grid 行，按更新时间倒序。
    pub articles: Vec<ArticleView>,
    /// 最近运行记录。
    pub run_history: Vec<RunRecord>,
    /// 当前仍在执行或暂停的批次。
    pub active_run: Option<RunRecord>,
}

/// 批量添加标题后的可观察结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTitlesResult {
    /// 实际创建的行数。
    pub created_count: usize,
    /// 输入中被忽略的空白行数。
    pub ignored_blank_count: usize,
    /// 与现有或本次输入标题相同但仍被保留的行数。
    pub duplicate_count: usize,
    /// 新创建任务的稳定 ID。
    pub task_ids: Vec<String>,
}

/// 启动批次后的立即响应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchStartResult {
    /// 新批次 ID；没有可运行行时为空。
    pub batch_id: Option<String>,
    /// 实际进入队列的行数。
    pub queued_count: usize,
    /// 实际进入队列或预检为可运行的任务 ID。
    pub queued_task_ids: Vec<String>,
    /// 明确选中但被跳过的行数。
    pub skipped_count: usize,
    /// 被跳过任务及稳定原因，供 Grid 精确定位。
    pub skipped: Vec<SkippedTask>,
}

/// Prompt 本地渲染预览结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPreview {
    /// 不触发外部请求的完整渲染文本。
    pub rendered: String,
}

/// 供 GUI 展示的 Prompt 模板及由 core 推导的变量契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplateView {
    /// 可版本化的模板本体。
    #[serde(flatten)]
    pub template: PromptTemplate,
    /// 当前阶段允许引用的全部变量。
    pub allowed_variables: Vec<String>,
    /// 当前阶段必须引用的变量。
    pub required_variables: Vec<String>,
}

impl From<PromptTemplate> for PromptTemplateView {
    fn from(template: PromptTemplate) -> Self {
        let allowed_variables = template
            .allowed_variables()
            .iter()
            .map(|variable| (*variable).to_string())
            .collect();
        let required_variables = template
            .required_variables()
            .iter()
            .map(|variable| (*variable).to_string())
            .collect();
        Self {
            template,
            allowed_variables,
            required_variables,
        }
    }
}

/// Tauri command 向 GUI 返回的稳定安全错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// 供前端分支处理的稳定错误码。
    pub code: String,
    /// 不含密钥或原始响应的用户可见说明。
    pub message: String,
}

impl CommandError {
    /// 创建一条稳定 command 错误。
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifang_content_production_assistant_core::{
        ApprovedPackage, ArticleArtifacts, ArticlePackageSnapshot, ReviewStatus,
    };

    /// IPC 视图只暴露是否有图，当前与历史快照均不暴露本地路径。
    #[test]
    fn article_view_scrubs_all_image_paths() {
        let artifacts = ArticleArtifacts {
            image_ref: Some("/private/generated.png".into()),
            original_image_ref: Some("/private/original.png".into()),
            ..ArticleArtifacts::default()
        };
        let mut article = ArticleTask::new("task", "标题", 1);
        article.artifacts = artifacts.clone();
        article.approved_history.push(ApprovedPackage {
            version: 1,
            title: "标题".into(),
            keywords: Vec::new(),
            requirements: String::new(),
            artifacts: artifacts.clone(),
            approved_at_ms: 2,
        });
        article.package_history.push(ArticlePackageSnapshot {
            version: 1,
            title: "标题".into(),
            keywords: Vec::new(),
            requirements: String::new(),
            review_status: ReviewStatus::Draft,
            artifacts,
            captured_at_ms: 2,
        });

        let view = ArticleView::from(article);
        assert!(view.has_image);
        assert!(view.article.artifacts.image_ref.is_none());
        assert!(view.article.artifacts.original_image_ref.is_none());
        assert!(
            view.article.approved_history[0]
                .artifacts
                .image_ref
                .is_none()
        );
        assert!(
            view.article.package_history[0]
                .artifacts
                .image_ref
                .is_none()
        );
        assert!(
            view.article.approved_history[0]
                .artifacts
                .original_image_ref
                .is_none()
        );
        assert!(
            view.article.package_history[0]
                .artifacts
                .original_image_ref
                .is_none()
        );
    }
}
