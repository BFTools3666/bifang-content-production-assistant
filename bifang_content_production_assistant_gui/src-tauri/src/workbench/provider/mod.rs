//! OpenAI-compatible、火山方舟图片 Provider 与系统凭据库边界。

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bifang_content_production_assistant_core::{
    ImageGenerationFuture, ImageGenerationRequest, ImageGenerationResult, ImageGenerator,
    ProviderError, ProviderErrorKind, TextGenerationFuture, TextGenerationRequest,
    TextGenerationResult, TextGenerator, TokenUsage,
};
use reqwest::{Client, Url, header};
use serde::{Deserialize, Serialize};

use super::models::{ImageRequestContract, ProviderConfig, ProviderKind};

#[cfg(not(feature = "xhs"))]
const KEYRING_SERVICE: &str = "com.arden.bifang-content-production-assistant.ai-provider";
#[cfg(feature = "xhs")]
const KEYRING_SERVICE: &str = "com.arden.bifang-xiaohongshu-assistant.ai-provider";
const MAX_TEXT_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_IMAGE_RESPONSE_BYTES: usize = 36 * 1024 * 1024;
const MAX_ARK_IMAGE_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_DOWNLOADED_IMAGE_BYTES: usize = 25 * 1024 * 1024;
const ARK_IMAGE_HOST_SUFFIX: &str = ".tos-cn-beijing.volces.com";

/// Provider 已返回计费用量后仍可能在产物校验或下载阶段失败。
#[derive(Debug)]
pub(crate) struct ProviderCallError {
    /// 可安全持久化和展示的稳定错误。
    pub error: ProviderError,
    /// 失败前 Provider 已明确返回的逐字段用量；完全缺失时为空。
    pub token_usage: Option<TokenUsage>,
}

impl ProviderCallError {
    /// 把已解析的真实用量附到后续处理错误，不推算缺失字段。
    fn with_token_usage(error: ProviderError, token_usage: Option<TokenUsage>) -> Self {
        Self { error, token_usage }
    }
}

impl From<ProviderError> for ProviderCallError {
    fn from(error: ProviderError) -> Self {
        Self {
            error,
            token_usage: None,
        }
    }
}

mod ark_download;
mod credential_vault;

pub(crate) use ark_download::validate_ark_generation_endpoint;
use ark_download::*;
pub use credential_vault::CredentialVault;

/// 不跟随重定向的受管 AI HTTPS 客户端。
#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    client: Client,
}

impl OpenAiCompatibleClient {
    /// 建立禁用重定向的客户端，防止认证头被带到意外主机。
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| "无法初始化 AI 网络客户端".to_string())?;
        Ok(Self { client })
    }

    /// 执行一次 OpenAI-compatible 文本生成请求。
    pub async fn generate_text(
        &self,
        config: &ProviderConfig,
        api_key: &str,
        request: &TextGenerationRequest,
    ) -> Result<TextGenerationResult, ProviderError> {
        self.generate_text_with_usage_error(config, api_key, request)
            .await
            .map_err(|error| error.error)
    }

    /// 执行文本请求，并在成功响应的产物校验失败时保留 Provider usage。
    pub(crate) async fn generate_text_with_usage_error(
        &self,
        config: &ProviderConfig,
        api_key: &str,
        request: &TextGenerationRequest,
    ) -> Result<TextGenerationResult, ProviderCallError> {
        validate_real_config(
            config,
            api_key,
            &request.model,
            ProviderKind::OpenAiCompatible,
        )?;
        request
            .parameters
            .validate()
            .map_err(|_| invalid_request("AI 文本生成参数超出允许范围"))?;
        let endpoint = provider_endpoint(&config.base_url, "chat/completions")?;
        let response = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                "Idempotency-Key",
                idempotency_header(&request.idempotency_key)?,
            )
            .timeout(Duration::from_millis(request.parameters.timeout_ms))
            .json(&ChatRequest {
                model: &request.model,
                messages: [ChatMessage {
                    role: "user",
                    content: &request.prompt,
                }],
                temperature: request
                    .parameters
                    .temperature_milli
                    .map(|value| f64::from(value) / 1_000.0),
                max_completion_tokens: request.parameters.max_output_tokens,
            })
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if !status.is_success() {
            return Err(ProviderError::from_http_status(
                status.as_u16(),
                parse_retry_after_ms(response.headers()),
            )
            .into());
        }
        let body = read_limited_body(
            response,
            MAX_TEXT_RESPONSE_BYTES,
            "AI 文本响应超过安全大小上限",
        )
        .await?;
        let payload = serde_json::from_slice::<ChatResponse>(&body)
            .map_err(|_| invalid_response("AI 文本响应格式无效"))?;
        let token_usage = payload.usage.and_then(ChatUsage::into_token_usage);
        let text = payload
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ProviderCallError::with_token_usage(
                    invalid_response("AI 文本响应为空"),
                    token_usage.clone(),
                )
            })?;
        Ok(TextGenerationResult {
            text,
            provider_request_id: request_id,
            token_usage,
        })
    }

    /// 按冻结契约发送图片请求；旧绑定必须保留首版请求体。
    pub(crate) async fn generate_image_with_contract(
        &self,
        config: &ProviderConfig,
        api_key: &str,
        request: &ImageGenerationRequest,
        contract: ImageRequestContract,
    ) -> Result<ImageGenerationResult, ProviderError> {
        self.generate_image_with_usage_error(config, api_key, request, contract)
            .await
            .map_err(|error| error.error)
    }

    /// 执行图片请求，并在成功响应的产物校验或下载失败时保留 Provider usage。
    pub(crate) async fn generate_image_with_usage_error(
        &self,
        config: &ProviderConfig,
        api_key: &str,
        request: &ImageGenerationRequest,
        contract: ImageRequestContract,
    ) -> Result<ImageGenerationResult, ProviderCallError> {
        let expected_kind = match contract {
            ImageRequestContract::VolcengineArkUrlV1 => ProviderKind::VolcengineArkImage,
            ImageRequestContract::ExplicitBase64V1
            | ImageRequestContract::ProviderDefaultBase64Unversioned
            | ImageRequestContract::ProviderDefaultBase64V2 => ProviderKind::OpenAiCompatible,
        };
        validate_real_config(config, api_key, &request.model, expected_kind)?;
        request
            .parameters
            .validate_for_provider_kind(expected_kind)
            .map_err(|_| invalid_request("AI 图片生成参数超出允许范围"))?;
        let endpoint = provider_endpoint(&config.base_url, "images/generations")?;
        if contract == ImageRequestContract::VolcengineArkUrlV1 {
            validate_ark_generation_endpoint(&endpoint)?;
        }
        let request_timeout = Duration::from_millis(request.parameters.timeout_ms);
        let request_builder = self
            .client
            .post(endpoint)
            .bearer_auth(api_key)
            .header(header::CONTENT_TYPE, "application/json")
            .timeout(request_timeout);
        let response = match contract {
            ImageRequestContract::VolcengineArkUrlV1 => {
                request_builder
                    .json(&VolcengineArkImageRequest {
                        model: &request.model,
                        prompt: &request.prompt,
                        sequential_image_generation: "disabled",
                        response_format: "url",
                        size: &request.parameters.size,
                        stream: false,
                        watermark: request.parameters.watermark,
                    })
                    .send()
                    .await
            }
            ImageRequestContract::ExplicitBase64V1
            | ImageRequestContract::ProviderDefaultBase64Unversioned
            | ImageRequestContract::ProviderDefaultBase64V2 => {
                request_builder
                    .header(
                        "Idempotency-Key",
                        idempotency_header(&request.idempotency_key)?,
                    )
                    .json(&ImageRequest {
                        model: &request.model,
                        prompt: &request.prompt,
                        size: &request.parameters.size,
                        response_format: match contract {
                            ImageRequestContract::ExplicitBase64V1 => Some("b64_json"),
                            ImageRequestContract::ProviderDefaultBase64Unversioned
                            | ImageRequestContract::ProviderDefaultBase64V2 => None,
                            ImageRequestContract::VolcengineArkUrlV1 => unreachable!(),
                        },
                    })
                    .send()
                    .await
            }
        }
        .map_err(|error| {
            if contract == ImageRequestContract::VolcengineArkUrlV1 && !error.is_connect() {
                ark_generation_outcome_unknown()
            } else {
                map_reqwest_error(error)
            }
        })?;
        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .or_else(|| response.headers().get("x-tt-logid"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if !status.is_success() {
            return Err(ProviderError::from_http_status(
                status.as_u16(),
                parse_retry_after_ms(response.headers()),
            )
            .into());
        }
        let maximum = if contract == ImageRequestContract::VolcengineArkUrlV1 {
            MAX_ARK_IMAGE_RESPONSE_BYTES
        } else {
            MAX_IMAGE_RESPONSE_BYTES
        };
        let body = read_limited_body(response, maximum, "AI 图片响应超过安全大小上限")
            .await
            .map_err(|error| {
                if contract == ImageRequestContract::VolcengineArkUrlV1
                    && matches!(
                        error.kind,
                        ProviderErrorKind::Timeout
                            | ProviderErrorKind::Transport
                            | ProviderErrorKind::UnknownOutcome
                    )
                {
                    ark_generation_outcome_unknown()
                } else {
                    error
                }
            })?;
        let payload = serde_json::from_slice::<ImageResponse>(&body)
            .map_err(|_| invalid_response("AI 图片响应格式无效"))?;
        let token_usage = payload.usage.and_then(ImageUsage::into_token_usage);
        let image = payload.data.into_iter().next().ok_or_else(|| {
            ProviderCallError::with_token_usage(
                invalid_response("AI 图片响应为空"),
                token_usage.clone(),
            )
        })?;
        if contract == ImageRequestContract::VolcengineArkUrlV1 {
            let image_url = image
                .url
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ProviderCallError::with_token_usage(
                        invalid_response("火山方舟图片响应缺少下载地址"),
                        token_usage.clone(),
                    )
                })?;
            let bytes = download_ark_image(&image_url, request_timeout)
                .await
                .map_err(ark_post_generation_download_failure)
                .map_err(|error| ProviderCallError::with_token_usage(error, token_usage.clone()))?;
            return Ok(ImageGenerationResult {
                image_url: None,
                image_base64: Some(BASE64.encode(bytes)),
                provider_request_id: request_id,
                token_usage,
            });
        }
        let image_base64 = image
            .b64_json
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ProviderCallError::with_token_usage(
                    invalid_response("AI 图片服务必须返回 Base64 图片数据"),
                    token_usage.clone(),
                )
            })?;
        BASE64.decode(image_base64.as_bytes()).map_err(|_| {
            ProviderCallError::with_token_usage(
                invalid_response("AI 图片服务返回的 Base64 数据无效"),
                token_usage.clone(),
            )
        })?;
        Ok(ImageGenerationResult {
            image_url: None,
            image_base64: Some(image_base64),
            provider_request_id: request_id,
            token_usage,
        })
    }
}

/// 把冻结配置和密钥绑定为 core 定义的厂商无关生成端口。
#[derive(Clone)]
pub struct OpenAiProviderSession {
    client: OpenAiCompatibleClient,
    config: ProviderConfig,
    api_key: String,
    image_request_contract: ImageRequestContract,
}

impl OpenAiProviderSession {
    /// 创建只在当前批次内存中存在的不可变 Provider 会话。
    pub fn new(
        client: OpenAiCompatibleClient,
        config: ProviderConfig,
        api_key: String,
        image_request_contract: ImageRequestContract,
    ) -> Self {
        Self {
            client,
            config,
            api_key,
            image_request_contract,
        }
    }

    /// worker 使用的文本调用，保留成功响应后处理失败前已返回的 usage。
    pub(crate) async fn generate_text_with_usage_error(
        &self,
        request: &TextGenerationRequest,
    ) -> Result<TextGenerationResult, ProviderCallError> {
        self.client
            .generate_text_with_usage_error(&self.config, &self.api_key, request)
            .await
    }

    /// worker 使用的图片调用，保留成功响应后处理失败前已返回的 usage。
    pub(crate) async fn generate_image_with_usage_error(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<ImageGenerationResult, ProviderCallError> {
        self.client
            .generate_image_with_usage_error(
                &self.config,
                &self.api_key,
                request,
                self.image_request_contract,
            )
            .await
    }
}

impl TextGenerator for OpenAiProviderSession {
    fn generate_text<'a>(&'a self, request: &'a TextGenerationRequest) -> TextGenerationFuture<'a> {
        Box::pin(async move {
            self.client
                .generate_text(&self.config, &self.api_key, request)
                .await
        })
    }
}

impl ImageGenerator for OpenAiProviderSession {
    fn generate_image<'a>(
        &'a self,
        request: &'a ImageGenerationRequest,
    ) -> ImageGenerationFuture<'a> {
        Box::pin(async move {
            self.client
                .generate_image_with_contract(
                    &self.config,
                    &self.api_key,
                    request,
                    self.image_request_contract,
                )
                .await
        })
    }
}

/// 在累积前检查 Content-Length，并逐块执行硬上限。
async fn read_limited_body(
    mut response: reqwest::Response,
    maximum: usize,
    oversize_message: &str,
) -> Result<Vec<u8>, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(invalid_response(oversize_message));
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(maximum),
    );
    while let Some(chunk) = response.chunk().await.map_err(map_response_body_error)? {
        if body.len().saturating_add(chunk.len()) > maximum {
            return Err(invalid_response(oversize_message));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// 校验幂等键可安全进入 HTTP header，且不会无界增长。
fn idempotency_header(value: &str) -> Result<header::HeaderValue, ProviderError> {
    if value.is_empty() || value.len() > 200 {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidRequest,
            safe_message: "AI 请求幂等键无效".into(),
            retry_after_ms: None,
        });
    }
    header::HeaderValue::from_str(value).map_err(|_| ProviderError {
        kind: ProviderErrorKind::InvalidRequest,
        safe_message: "AI 请求幂等键无效".into(),
        retry_after_ms: None,
    })
}

/// 创建不含外部值的有界请求参数错误。
fn invalid_request(message: &str) -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::InvalidRequest,
        safe_message: message.into(),
        retry_after_ms: None,
    }
}

/// 创建一个不包含下载 URL、签名参数或响应正文的传输错误。
fn transport_error(message: &str) -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::Transport,
        safe_message: message.into(),
        retry_after_ms: None,
    }
}

/// 校验正式 Provider 配置、模型和密钥均完整。
fn validate_real_config(
    config: &ProviderConfig,
    api_key: &str,
    model: &str,
    expected_kind: ProviderKind,
) -> Result<(), ProviderError> {
    if !config.enabled
        || config.kind != expected_kind
        || config.base_url.trim().is_empty()
        || model.trim().is_empty()
        || api_key.trim().is_empty()
    {
        return Err(ProviderError {
            kind: ProviderErrorKind::NotConfigured,
            safe_message: "AI 服务地址、模型或密钥尚未配置完整".into(),
            retry_after_ms: None,
        });
    }
    Ok(())
}

/// 解析并收窄 Provider 端点；生产只允许无凭据、无查询的 HTTPS 根地址。
pub fn provider_endpoint(base_url: &str, path: &str) -> Result<Url, ProviderError> {
    if !url_text_has_canonical_authority(base_url) || url_text_has_credentials(base_url) {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidRequest,
            safe_message: "AI 服务地址不能包含非标准 authority 或凭据".into(),
            retry_after_ms: None,
        });
    }
    let mut base = Url::parse(base_url).map_err(|_| ProviderError {
        kind: ProviderErrorKind::InvalidRequest,
        safe_message: "AI 服务地址无效".into(),
        retry_after_ms: None,
    })?;
    let loopback_test = cfg!(test)
        && base.scheme() == "http"
        && base
            .host_str()
            .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "::1"));
    if base.scheme() != "https" && !loopback_test {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidRequest,
            safe_message: "AI 服务地址必须使用 HTTPS".into(),
            retry_after_ms: None,
        });
    }
    if !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || base.host_str().is_none()
    {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidRequest,
            safe_message: "AI 服务地址不能包含凭据、查询参数或片段".into(),
            retry_after_ms: None,
        });
    }
    if !base.path().ends_with('/') {
        let path = format!("{}/", base.path());
        base.set_path(&path);
    }
    base.join(path).map_err(|_| ProviderError {
        kind: ProviderErrorKind::InvalidRequest,
        safe_message: "无法构造 AI 服务请求地址".into(),
        retry_after_ms: None,
    })
}

/// 把传输错误映射为不含 URL、密钥或底层正文的稳定错误。
fn map_reqwest_error(error: reqwest::Error) -> ProviderError {
    let kind = if error.is_timeout() {
        ProviderErrorKind::Timeout
    } else if error.is_connect() {
        ProviderErrorKind::Transport
    } else {
        ProviderErrorKind::UnknownOutcome
    };
    ProviderError {
        kind,
        safe_message: match kind {
            ProviderErrorKind::Timeout => "AI 服务响应超时",
            ProviderErrorKind::Transport => "无法连接 AI 服务",
            _ => "AI 请求结果无法确认",
        }
        .into(),
        retry_after_ms: None,
    }
}

/// 创建一个无原始响应内容的无效响应错误。
fn invalid_response(message: &str) -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: message.into(),
        retry_after_ms: None,
    }
}

/// 方舟已完成计费生成后，保留脱敏原因但不得进入普通自动重试并再次发起生成。
fn ark_post_generation_download_failure(error: ProviderError) -> ProviderError {
    let detail = match error.kind {
        ProviderErrorKind::Authentication | ProviderErrorKind::InvalidRequest => {
            "临时图片下载地址已失效或被拒绝"
        }
        ProviderErrorKind::RateLimited => "临时图片下载请求被限流",
        ProviderErrorKind::Timeout => "临时图片下载超时",
        ProviderErrorKind::Transport => "临时图片下载网络连接失败",
        ProviderErrorKind::Server => "临时图片存储服务暂时不可用",
        ProviderErrorKind::ContentRejected => "临时图片下载被安全策略拒绝",
        ProviderErrorKind::Cancelled => "临时图片下载已取消",
        ProviderErrorKind::UnknownOutcome => "临时图片下载结果无法确认",
        ProviderErrorKind::NotConfigured => "临时图片下载配置无效",
        ProviderErrorKind::InvalidResponse => safe_ark_download_failure_detail(&error.safe_message),
    };
    ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: format!(
            "火山方舟已生成图片，但临时文件下载失败：{detail}；为避免重复计费，不会自动重试，请手动重新生成配图"
        ),
        retry_after_ms: None,
    }
}

/// 防御性拒绝意外携带 URL/query 的“安全”原因；当前下载路径只产生静态脱敏消息。
fn safe_ark_download_failure_detail(message: &str) -> &str {
    let message = message.trim();
    let lowercase = message.to_ascii_lowercase();
    if message.is_empty()
        || message.len() > 200
        || message.contains("://")
        || message
            .chars()
            .any(|character| matches!(character, '?' | '#' | '&' | '=' | '/' | '\\'))
        || lowercase.contains("x-tos-")
    {
        "具体下载原因已隐藏"
    } else {
        message
    }
}

/// 无幂等保证的付费方舟请求一旦可能发出，结果不明时不得自动再次 POST。
fn ark_generation_outcome_unknown() -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message:
            "火山方舟生成请求结果无法确认；为避免重复计费，不会自动重试，请确认方舟记录后手动重新生成配图"
                .into(),
        retry_after_ms: None,
    }
}

/// 解析整数秒形式的 Retry-After，并设置一分钟上限。
fn parse_retry_after_ms(headers: &header::HeaderMap) -> Option<u64> {
    headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1_000).min(60_000))
}

/// OpenAI-compatible 对话请求体。
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 1],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
}

/// OpenAI-compatible 单条用户消息。
#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

/// OpenAI-compatible 对话响应体的最小安全视图。
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

/// 一条文本候选。
#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

/// 文本候选的内容载荷。
#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

/// 可选 token 使用量。
#[derive(Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

impl ChatUsage {
    /// Chat Completions 的 prompt/completion 字段映射为中立输入/输出字段。
    fn into_token_usage(self) -> Option<TokenUsage> {
        TokenUsage::reported(
            self.prompt_tokens,
            self.completion_tokens,
            self.total_tokens,
        )
    }
}

/// OpenAI-compatible 图片请求体。
#[derive(Serialize)]
struct ImageRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    size: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'static str>,
}

/// 火山方舟 Seedream 单图 URL 响应请求体；字段保持用户提供契约的固定语义。
#[derive(Serialize)]
struct VolcengineArkImageRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    sequential_image_generation: &'static str,
    response_format: &'static str,
    size: &'a str,
    stream: bool,
    watermark: bool,
}

/// OpenAI-compatible 图片响应体的最小安全视图。
#[derive(Deserialize)]
struct ImageResponse {
    data: Vec<ImageData>,
    usage: Option<ImageUsage>,
}

/// OpenAI 图片与 Ark 图片共享的真实 token 字段最小视图。
#[derive(Deserialize)]
struct ImageUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

impl ImageUsage {
    /// 各字段保持 Provider 的缺失状态，不从总量或输出量反推输入量。
    fn into_token_usage(self) -> Option<TokenUsage> {
        TokenUsage::reported(self.input_tokens, self.output_tokens, self.total_tokens)
    }
}

/// 单张图片按冻结契约接收 Base64 或受限 URL；其他字段由 serde 忽略。
#[derive(Deserialize)]
struct ImageData {
    b64_json: Option<String>,
    url: Option<String>,
}

#[cfg(test)]
mod tests;
