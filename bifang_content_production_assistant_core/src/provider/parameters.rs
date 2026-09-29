use super::*;

/// 文本阶段允许冻结和发送的有界生成参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGenerationParameters {
    /// 温度的千分值；`700` 表示 `0.7`，为空时使用 Provider 默认值。
    pub temperature_milli: Option<u16>,
    /// 最大输出 token 数；为空时使用 Provider 默认值。
    pub max_output_tokens: Option<u32>,
    /// 单次请求从发送到完整读取的超时毫秒数。
    pub timeout_ms: u64,
}

impl Default for TextGenerationParameters {
    fn default() -> Self {
        Self {
            temperature_milli: None,
            max_output_tokens: None,
            timeout_ms: 60_000,
        }
    }
}

impl TextGenerationParameters {
    /// 校验公开参数边界，避免任意值直接进入 Provider 请求。
    pub fn validate(&self) -> Result<(), ProviderParameterError> {
        if self.temperature_milli.is_some_and(|value| value > 2_000) {
            return Err(ProviderParameterError::TemperatureOutOfRange);
        }
        if self
            .max_output_tokens
            .is_some_and(|value| !(1..=128_000).contains(&value))
        {
            return Err(ProviderParameterError::MaxOutputTokensOutOfRange);
        }
        if !(1_000..=300_000).contains(&self.timeout_ms) {
            return Err(ProviderParameterError::TimeoutOutOfRange);
        }
        Ok(())
    }

    /// 返回稳定的参数材料，用于把参数变化纳入请求幂等边界。
    ///
    /// 旧版请求等价的默认参数返回空串，以保持升级前已发送请求的幂等键不变。
    pub fn idempotency_material(&self) -> String {
        if self == &Self::default() {
            return String::new();
        }
        format!(
            "temperatureMilli={};maxOutputTokens={};timeoutMs={}",
            self.temperature_milli
                .map_or_else(|| "default".into(), |value| value.to_string()),
            self.max_output_tokens
                .map_or_else(|| "default".into(), |value| value.to_string()),
            self.timeout_ms
        )
    }
}

/// 图片阶段允许冻结和发送的有界生成参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenerationParameters {
    /// 当前受支持图片协议允许的图片尺寸。
    pub size: String,
    /// 是否请求供应商在生成图片中加入 AI 水印；旧配置默认保持关闭。
    #[serde(default)]
    pub watermark: bool,
    /// 单次请求从发送到完整读取的超时毫秒数。
    pub timeout_ms: u64,
}

impl Default for ImageGenerationParameters {
    fn default() -> Self {
        Self {
            size: "1536x1024".into(),
            watermark: false,
            timeout_ms: 90_000,
        }
    }
}

impl ImageGenerationParameters {
    /// 建立火山方舟图片服务使用的首版显式默认参数。
    pub fn volcengine_ark_default() -> Self {
        Self {
            size: "2K".into(),
            watermark: true,
            timeout_ms: 90_000,
        }
    }

    /// 校验公开参数边界，拒绝任意图片尺寸或无界等待。
    pub fn validate(&self) -> Result<(), ProviderParameterError> {
        if !matches!(
            self.size.as_str(),
            "1024x1024" | "1536x1024" | "1024x1536" | "2K"
        ) {
            return Err(ProviderParameterError::UnsupportedImageSize);
        }
        if !(1_000..=300_000).contains(&self.timeout_ms) {
            return Err(ProviderParameterError::TimeoutOutOfRange);
        }
        Ok(())
    }

    /// 按服务协议进一步收窄图片尺寸，避免把供应商专属值发往错误端点。
    pub fn validate_for_provider_kind(
        &self,
        provider_kind: ProviderKind,
    ) -> Result<(), ProviderParameterError> {
        self.validate()?;
        let supported = match provider_kind {
            ProviderKind::OpenAiCompatible => {
                matches!(self.size.as_str(), "1024x1024" | "1536x1024" | "1024x1536")
            }
            ProviderKind::VolcengineArkImage => self.size == "2K",
            ProviderKind::Fixture => true,
        };
        if !supported {
            return Err(ProviderParameterError::UnsupportedImageSize);
        }
        if provider_kind == ProviderKind::OpenAiCompatible && self.watermark {
            return Err(ProviderParameterError::UnsupportedImageWatermark);
        }
        Ok(())
    }

    /// 返回稳定的参数材料，用于把参数变化纳入请求幂等边界。
    ///
    /// 旧版请求等价的默认参数返回空串，以保持升级前已发送请求的幂等键不变。
    pub fn idempotency_material(&self) -> String {
        if self == &Self::default() {
            return String::new();
        }
        let legacy_material = format!("size={};timeoutMs={}", self.size, self.timeout_ms);
        if self.watermark {
            format!("{legacy_material};watermark=true")
        } else {
            legacy_material
        }
    }
}
