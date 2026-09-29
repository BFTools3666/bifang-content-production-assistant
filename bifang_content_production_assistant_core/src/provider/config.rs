use super::*;

/// 不包含 API 密钥的 AI Provider 配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// 本地稳定配置 ID。
    pub id: String,
    /// 用户可识别的名称。
    pub name: String,
    /// 同一稳定 ID 下不可变配置的修订号。
    #[serde(default = "default_provider_revision")]
    pub revision: u32,
    /// Provider 配置类型。
    pub kind: ProviderKind,
    /// 是否允许该配置参与新的连接测试和工作流执行。
    #[serde(default)]
    pub enabled: bool,
    /// 由宿主适配器校验和使用的 API 根地址。
    pub base_url: String,
    /// v1/v2 正文和简介共享的兼容模型镜像；新执行不得直接读取。
    #[serde(default)]
    pub text_model: String,
    /// 正文阶段使用的模型；缺失仅表示尚未迁移的 v1/v2 数据。
    #[serde(default)]
    pub article_model: Option<String>,
    /// 简介阶段使用的模型；缺失仅表示尚未迁移的 v1/v2 数据。
    #[serde(default)]
    pub summary_model: Option<String>,
    /// 配图阶段使用的模型。
    pub image_model: String,
    /// 正文阶段冻结的有界生成参数。
    #[serde(default)]
    pub article_parameters: Option<TextGenerationParameters>,
    /// 简介阶段冻结的有界生成参数。
    #[serde(default)]
    pub summary_parameters: Option<TextGenerationParameters>,
    /// 配图阶段冻结的有界生成参数。
    #[serde(default)]
    pub image_parameters: Option<ImageGenerationParameters>,
    /// 是否明确分配为正文阶段唯一服务；`defaultForText` 仅用于读取 v1 快照。
    #[serde(default, alias = "defaultForText")]
    pub default_for_article: bool,
    /// 是否明确分配为简介阶段唯一服务。
    #[serde(default)]
    pub default_for_summary: bool,
    /// 是否明确分配为配图阶段唯一服务。
    pub default_for_image: bool,
}

impl ProviderConfig {
    /// 建立新表单使用的 OpenAI-compatible 领域默认配置，不包含稳定 ID 或密钥。
    pub fn open_ai_compatible_draft() -> Self {
        Self {
            id: String::new(),
            name: "OpenAI Compatible".into(),
            revision: 1,
            kind: ProviderKind::OpenAiCompatible,
            enabled: true,
            base_url: "https://api.openai.com/v1".into(),
            text_model: String::new(),
            article_model: Some(String::new()),
            summary_model: Some(String::new()),
            image_model: String::new(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::default()),
            default_for_article: true,
            default_for_summary: true,
            default_for_image: true,
        }
    }

    /// 建立新表单使用的火山方舟图片服务默认配置，不包含稳定 ID 或密钥。
    pub fn volcengine_ark_image_draft() -> Self {
        Self {
            id: String::new(),
            name: "火山方舟图片".into(),
            revision: 1,
            kind: ProviderKind::VolcengineArkImage,
            enabled: true,
            base_url: "https://ark.cn-beijing.volces.com/api/v3".into(),
            text_model: String::new(),
            article_model: Some(String::new()),
            summary_model: Some(String::new()),
            image_model: "ep-20260318093407-x5qm5".into(),
            article_parameters: Some(TextGenerationParameters::default()),
            summary_parameters: Some(TextGenerationParameters::default()),
            image_parameters: Some(ImageGenerationParameters::volcengine_ark_default()),
            default_for_article: false,
            default_for_summary: false,
            default_for_image: true,
        }
    }

    /// 把旧版共享模型和硬编码参数物化成三阶段显式配置。
    pub fn materialize_stage_configuration(&mut self) -> bool {
        let before = (
            self.text_model.clone(),
            self.article_model.clone(),
            self.summary_model.clone(),
            self.article_parameters.clone(),
            self.summary_parameters.clone(),
            self.image_parameters.clone(),
        );
        if self.article_model.is_none() {
            self.article_model = Some(self.text_model.clone());
        }
        if self.summary_model.is_none() {
            self.summary_model = Some(self.text_model.clone());
        }
        if self.article_parameters.is_none() {
            self.article_parameters = Some(TextGenerationParameters::default());
        }
        if self.summary_parameters.is_none() {
            self.summary_parameters = Some(TextGenerationParameters::default());
        }
        if self.image_parameters.is_none() {
            self.image_parameters = Some(self.kind.default_image_parameters());
        }
        if self.text_model.trim().is_empty() {
            self.text_model = self
                .article_model()
                .trim()
                .is_empty()
                .then(|| self.summary_model().to_string())
                .unwrap_or_else(|| self.article_model().to_string());
        }
        before
            != (
                self.text_model.clone(),
                self.article_model.clone(),
                self.summary_model.clone(),
                self.article_parameters.clone(),
                self.summary_parameters.clone(),
                self.image_parameters.clone(),
            )
    }

    /// 返回正文阶段的显式模型，并对尚未物化的旧数据安全回退。
    pub fn article_model(&self) -> &str {
        self.article_model.as_deref().unwrap_or(&self.text_model)
    }

    /// 返回简介阶段的显式模型，并对尚未物化的旧数据安全回退。
    pub fn summary_model(&self) -> &str {
        self.summary_model.as_deref().unwrap_or(&self.text_model)
    }

    /// 返回正文阶段参数，并保持旧数据的历史运行默认值。
    pub fn article_parameters(&self) -> TextGenerationParameters {
        self.article_parameters.clone().unwrap_or_default()
    }

    /// 返回简介阶段参数，并保持旧数据的历史运行默认值。
    pub fn summary_parameters(&self) -> TextGenerationParameters {
        self.summary_parameters.clone().unwrap_or_default()
    }

    /// 返回配图阶段参数，并保持旧数据的历史运行默认值。
    pub fn image_parameters(&self) -> ImageGenerationParameters {
        self.image_parameters.clone().unwrap_or_default()
    }

    /// 校验三个阶段的模型、默认位和有界参数之间的领域关系。
    pub fn validate_stage_configuration(&self) -> Result<(), ProviderConfigError> {
        if !self.enabled
            && (self.default_for_article || self.default_for_summary || self.default_for_image)
        {
            return Err(ProviderConfigError::DisabledDefault);
        }
        if self.kind == ProviderKind::Fixture {
            return Ok(());
        }
        let configures_text = !self.article_model().trim().is_empty()
            || !self.summary_model().trim().is_empty()
            || self.default_for_article
            || self.default_for_summary;
        let configures_image = !self.image_model.trim().is_empty() || self.default_for_image;
        if configures_text && !self.kind.supports(ProviderCapability::Text) {
            return Err(ProviderConfigError::UnsupportedStage(
                ProviderCapability::Text,
            ));
        }
        if configures_image && !self.kind.supports(ProviderCapability::Image) {
            return Err(ProviderConfigError::UnsupportedStage(
                ProviderCapability::Image,
            ));
        }
        if self.article_model().trim().is_empty()
            && self.summary_model().trim().is_empty()
            && self.image_model.trim().is_empty()
        {
            return Err(ProviderConfigError::StageModelRequired);
        }
        if (self.default_for_article && self.article_model().trim().is_empty())
            || (self.default_for_summary && self.summary_model().trim().is_empty())
            || (self.default_for_image && self.image_model.trim().is_empty())
        {
            return Err(ProviderConfigError::DefaultModelRequired);
        }
        self.article_parameters().validate()?;
        self.summary_parameters().validate()?;
        self.image_parameters()
            .validate_for_provider_kind(self.kind)?;
        Ok(())
    }

    /// 校验一个配置能否进入当前集合；端点 URL 等宿主协议规则仍由适配器负责。
    pub fn validate_for_persistence(&self) -> Result<(), ProviderConfigError> {
        if self.id.trim().is_empty() {
            return Err(ProviderConfigError::IdRequired);
        }
        if self.name.trim().is_empty() {
            return Err(ProviderConfigError::NameRequired);
        }
        self.validate_stage_configuration()
    }
}

/// Provider 三阶段配置违反稳定业务约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderConfigError {
    /// 持久配置必须拥有稳定 ID。
    IdRequired,
    /// 持久配置必须拥有用户可识别的名称。
    NameRequired,
    /// 三个阶段均未填写模型。
    StageModelRequired,
    /// 被设为默认的阶段没有填写对应模型。
    DefaultModelRequired,
    /// 已停用配置不能继续承担任何阶段默认位。
    DisabledDefault,
    /// 当前服务类型不支持被配置的生成阶段。
    UnsupportedStage(ProviderCapability),
    /// 某阶段生成参数超出允许值域。
    InvalidParameter(ProviderParameterError),
}

impl ProviderConfigError {
    /// 返回供适配器和前端使用的稳定错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdRequired => "PROVIDER_ID_REQUIRED",
            Self::NameRequired => "PROVIDER_NAME_REQUIRED",
            Self::StageModelRequired => "PROVIDER_STAGE_MODEL_REQUIRED",
            Self::DefaultModelRequired => "PROVIDER_DEFAULT_MODEL_REQUIRED",
            Self::DisabledDefault => "PROVIDER_DISABLED_DEFAULT",
            Self::UnsupportedStage(_) => "PROVIDER_STAGE_UNSUPPORTED",
            Self::InvalidParameter(_) => "PROVIDER_PARAMETER_INVALID",
        }
    }
}

impl From<ProviderParameterError> for ProviderConfigError {
    fn from(error: ProviderParameterError) -> Self {
        Self::InvalidParameter(error)
    }
}

impl std::fmt::Display for ProviderConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdRequired => formatter.write_str("AI 服务稳定 ID 不能为空"),
            Self::NameRequired => formatter.write_str("AI 服务名称不能为空"),
            Self::StageModelRequired => formatter.write_str("至少为一个阶段填写模型"),
            Self::DefaultModelRequired => formatter.write_str("设为默认的阶段必须填写对应模型"),
            Self::DisabledDefault => formatter.write_str("已停用的 AI 服务不能设为默认服务"),
            Self::UnsupportedStage(ProviderCapability::Text) => {
                formatter.write_str("当前 AI 服务类型不支持正文或简介生成")
            }
            Self::UnsupportedStage(ProviderCapability::Image) => {
                formatter.write_str("当前 AI 服务类型不支持配图生成")
            }
            Self::InvalidParameter(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProviderConfigError {}
