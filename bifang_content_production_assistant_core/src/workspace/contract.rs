use super::*;

fn missing_prompt_renderer_version() -> u32 {
    0
}

pub(super) fn legacy_image_request_contract() -> ImageRequestContract {
    ImageRequestContract::ExplicitBase64V1
}

/// 图片请求体与响应解析的冻结 wire 契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageRequestContract {
    /// 首版显式请求 `b64_json`，供旧执行绑定保持同键同体。
    ExplicitBase64V1,
    /// Task 5 初版 v3 快照已经省略字段但尚未把契约纳入幂等材料。
    ProviderDefaultBase64Unversioned,
    /// GPT Image-compatible 请求省略 `response_format`，但仍只接收 Base64。
    ProviderDefaultBase64V2,
    /// 火山方舟显式请求 URL 响应的首版契约。
    VolcengineArkUrlV1,
}

impl ImageRequestContract {
    /// 新执行绑定使用的当前 wire 契约。
    pub const CURRENT: Self = Self::ProviderDefaultBase64V2;

    /// 返回与服务类型匹配的图片 wire 契约。
    pub const fn for_provider_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::OpenAiCompatible | ProviderKind::Fixture => Self::CURRENT,
            ProviderKind::VolcengineArkImage => Self::VolcengineArkUrlV1,
        }
    }

    /// 纳入新请求幂等身份；首版保持空串以复用升级前的键。
    pub const fn idempotency_material(self) -> &'static str {
        match self {
            Self::ExplicitBase64V1 | Self::ProviderDefaultBase64Unversioned => "",
            Self::ProviderDefaultBase64V2 => "imageRequestContract=providerDefaultBase64V2",
            Self::VolcengineArkUrlV1 => "imageRequestContract=volcengineArkUrlV1",
        }
    }
}

/// 一个阶段在文章包生命周期内冻结的 Provider 配置与凭据引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderExecutionBinding {
    /// 完整非秘密 Provider 修订，避免普通重试漂移到新地址或模型。
    pub config: ProviderConfig,
    /// 系统凭据库条目引用；夹具不需要凭据。
    pub credential_ref: Option<String>,
}

/// 一篇文章当前包版本冻结的三类 Prompt 与三阶段 Provider 绑定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageExecutionBinding {
    /// 所属文章稳定 ID。
    pub task_id: String,
    /// 所属文章包版本。
    pub package_version: u32,
    /// 此包冻结的 Prompt 渲染算法；缺失字段先以 0 保留给 schema 迁移判定。
    #[serde(default = "missing_prompt_renderer_version")]
    pub prompt_renderer_version: u32,
    /// 此包冻结的图片 wire 契约；v1-v3 缺失字段只在工作区迁移时补为首版。
    #[serde(default)]
    pub image_request_contract: Option<ImageRequestContract>,
    /// 正文 Prompt 的不可变版本。
    pub article_template: PromptTemplate,
    /// 简介 Prompt 的不可变版本。
    pub summary_template: PromptTemplate,
    /// 配图 Prompt 的不可变版本。
    pub image_template: PromptTemplate,
    /// 正文阶段 Provider；未配置时为空并保持失败关闭。
    pub article_provider: Option<ProviderExecutionBinding>,
    /// 简介阶段 Provider；未配置时为空并保持失败关闭。
    pub summary_provider: Option<ProviderExecutionBinding>,
    /// 配图阶段 Provider；未配置时为空并保持失败关闭。
    pub image_provider: Option<ProviderExecutionBinding>,
}
