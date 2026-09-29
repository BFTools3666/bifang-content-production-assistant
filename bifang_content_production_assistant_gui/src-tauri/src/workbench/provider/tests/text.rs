use super::*;

/// 文本适配器映射请求并解析内容、request id 和 usage。
#[tokio::test]
async fn parses_successful_chat_completion() {
    let (base, captured) = serve_once_capturing(
        "200 OK",
        &[("x-request-id", "request-1")],
        r#"{"choices":[{"message":{"content":"生成正文"}}],"usage":{"prompt_tokens":12,"completion_tokens":30,"total_tokens":42}}"#,
    );
    let client = OpenAiCompatibleClient::new().unwrap();
    let result = client
        .generate_text(
            &config(base),
            "test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    temperature_milli: Some(700),
                    max_output_tokens: Some(4_096),
                    timeout_ms: 2_000,
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(result.text, "生成正文");
    assert_eq!(result.provider_request_id.as_deref(), Some("request-1"));
    assert_eq!(
        result.token_usage,
        Some(TokenUsage {
            input_tokens: Some(12),
            output_tokens: Some(30),
            total_tokens: Some(42),
        })
    );
    let request = String::from_utf8(captured.recv().unwrap()).unwrap();
    let lower = request.to_ascii_lowercase();
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    assert!(lower.contains("authorization: bearer test-key\r\n"));
    assert!(lower.contains("content-type: application/json\r\n"));
    assert!(lower.contains("idempotency-key: task:v1:article\r\n"));
    let body = request.split_once("\r\n\r\n").unwrap().1;
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(json["model"], "text-model");
    assert_eq!(json["messages"][0]["role"], "user");
    assert_eq!(json["messages"][0]["content"], "标题");
    assert_eq!(json["temperature"], 0.7);
    assert_eq!(json["max_completion_tokens"], 4_096);
    assert!(json.get("max_tokens").is_none());
}

/// Provider 完全不返回 usage 时保持缺失，不依据请求参数或文本长度估算。
#[tokio::test]
async fn preserves_missing_chat_usage_without_estimation() {
    let base = serve_once(
        "200 OK",
        &[],
        r#"{"choices":[{"message":{"content":"无 usage 正文"}}]}"#,
    );
    let result = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_text(
            &config(base),
            "test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "很长的输入也不能用于估算".into(),
                parameters: TextGenerationParameters::default(),
                idempotency_key: "task:v1:article:no-usage".into(),
            },
        )
        .await
        .unwrap();

    assert!(result.token_usage.is_none());
}

/// 缺模型、缺凭据或参数越界必须在建立 TCP 连接前失败关闭。
#[tokio::test]
async fn missing_configuration_and_invalid_parameters_make_zero_connections() {
    for (model, api_key, parameters, expected_kind) in [
        (
            "",
            "test-key",
            TextGenerationParameters::default(),
            ProviderErrorKind::NotConfigured,
        ),
        (
            "text-model",
            "",
            TextGenerationParameters::default(),
            ProviderErrorKind::NotConfigured,
        ),
        (
            "text-model",
            "test-key",
            TextGenerationParameters {
                temperature_milli: Some(2_001),
                ..TextGenerationParameters::default()
            },
            ProviderErrorKind::InvalidRequest,
        ),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let client = OpenAiCompatibleClient::new().unwrap();
        let error = client
            .generate_text(
                &config(base),
                api_key,
                &TextGenerationRequest {
                    model: model.into(),
                    prompt: "标题".into(),
                    parameters,
                    idempotency_key: "zero-outbound".into(),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(error.kind, expected_kind);
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
        assert!(!error.safe_message.contains("test-key"));
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let mut disabled = config(base);
    disabled.enabled = false;
    let error = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_text(
            &disabled,
            "test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters::default(),
                idempotency_key: "disabled-zero-outbound".into(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, ProviderErrorKind::NotConfigured);
    assert!(matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

/// 429 响应映射为可重试限流且遵守 Retry-After 上限。
#[tokio::test]
async fn maps_rate_limit_without_exposing_response_body() {
    let base = serve_once(
        "429 Too Many Requests",
        &[("Retry-After", "3")],
        r#"{"error":{"message":"secret provider details"}}"#,
    );
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::RateLimited);
    assert_eq!(error.retry_after_ms, Some(3_000));
    assert!(!error.safe_message.contains("secret provider details"));
    assert!(!error.safe_message.contains("test-key"));
}

/// Provider 明确返回 HTTP 408 时映射为可重试超时，而不是响应格式错误。
#[tokio::test]
async fn maps_http_request_timeout_to_retryable_timeout() {
    let base = serve_once("408 Request Timeout", &[], r#"{"error":"timeout"}"#);
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "sensitive-test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::Timeout);
    assert!(error.is_retryable());
    assert!(!error.safe_message.contains("sensitive-test-key"));
}

/// 正式端点拒绝 HTTP、内嵌凭据和查询参数。
#[test]
fn rejects_unsafe_provider_endpoints() {
    assert!(provider_endpoint("http://example.com/v1", "chat/completions").is_err());
    assert!(provider_endpoint("https://user@example.com/v1", "chat/completions").is_err());
    assert!(provider_endpoint("https://@example.com/v1", "chat/completions").is_err());
    assert!(provider_endpoint("https:////example.com/v1", "chat/completions").is_err());
    assert!(provider_endpoint("https:\\\\example.com/v1", "chat/completions").is_err());
    assert!(provider_endpoint("https://\t/example.com/v1", "chat/completions").is_err());
    assert!(provider_endpoint("https://example.com/v1?token=x", "chat/completions").is_err());
    assert!(provider_endpoint("https://example.com/v1", "chat/completions").is_ok());
}

/// 响应体超过硬上限时在 JSON 解析前拒绝，不暴露载荷。
#[tokio::test]
async fn rejects_oversized_text_response_before_parsing() {
    let oversized = "x".repeat(MAX_TEXT_RESPONSE_BYTES + 1);
    let base = serve_once("200 OK", &[], &oversized);
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(error.safe_message.contains("安全大小上限"));
}

/// 无 Content-Length 的 chunked 响应仍受逐块硬上限约束。
#[tokio::test]
async fn rejects_oversized_chunked_text_response() {
    let base = serve_chunked_body(vec![b'x'; MAX_TEXT_RESPONSE_BYTES + 1]);
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "sensitive-test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(error.safe_message.contains("安全大小上限"));
    assert!(!error.safe_message.contains("sensitive-test-key"));
}

/// 3xx 响应不会被跟随，认证头也不会到达 Location 指向的第二主机。
#[tokio::test]
async fn refuses_redirect_without_forwarding_bearer_credentials() {
    let redirected = TcpListener::bind("127.0.0.1:0").unwrap();
    redirected.set_nonblocking(true).unwrap();
    let location = format!("http://{}/capture", redirected.local_addr().unwrap());
    let base = serve_once("302 Found", &[("Location", location.as_str())], "{}");
    let client = OpenAiCompatibleClient::new().unwrap();

    let error = client
        .generate_text(
            &config(base),
            "sensitive-test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(
        redirected.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    assert!(!error.safe_message.contains("sensitive-test-key"));
}

/// 所有冻结契约都拒绝 URL-only 图片返回，避免访问服务端指定的任意地址。
#[tokio::test]
async fn rejects_url_only_image_response() {
    for contract in [
        ImageRequestContract::ExplicitBase64V1,
        ImageRequestContract::ProviderDefaultBase64V2,
    ] {
        let base = serve_once(
            "200 OK",
            &[],
            r#"{"data":[{"url":"https://example.com/private.png"}]}"#,
        );
        let client = OpenAiCompatibleClient::new().unwrap();
        let error = client
            .generate_image_with_contract(
                &config(base),
                "test-key",
                &ImageGenerationRequest {
                    model: "image-model".into(),
                    prompt: "配图".into(),
                    parameters: ImageGenerationParameters {
                        size: "1024x1024".into(),
                        timeout_ms: 2_000,
                        ..ImageGenerationParameters::default()
                    },
                    idempotency_key: "task:v1:image".into(),
                },
                contract,
            )
            .await
            .unwrap_err();

        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(error.safe_message.contains("Base64"));
    }
}

/// 真实 HTTP 401 在适配器边界映射为不可重试鉴权错误且不泄露正文或密钥。
#[tokio::test]
async fn maps_authentication_without_exposing_response_body_or_key() {
    let base = serve_once(
        "401 Unauthorized",
        &[],
        r#"{"error":{"message":"sensitive upstream body"}}"#,
    );
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "sensitive-test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::Authentication);
    assert!(!error.is_retryable());
    assert!(!error.safe_message.contains("sensitive upstream body"));
    assert!(!error.safe_message.contains("sensitive-test-key"));
}

/// 200 但 JSON 无效或文本为空时返回稳定、不可重试的响应错误。
#[tokio::test]
async fn rejects_invalid_json_and_empty_text_responses() {
    for body in [
        "sensitive invalid json",
        r#"{"choices":[{"message":{"content":"   "}}]}"#,
    ] {
        let base = serve_once("200 OK", &[], body);
        let client = OpenAiCompatibleClient::new().unwrap();
        let error = client
            .generate_text(
                &config(base),
                "sensitive-test-key",
                &TextGenerationRequest {
                    model: "text-model".into(),
                    prompt: "标题".into(),
                    parameters: TextGenerationParameters {
                        timeout_ms: 2_000,
                        ..TextGenerationParameters::default()
                    },
                    idempotency_key: "task:v1:article".into(),
                },
            )
            .await
            .unwrap_err();

        assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
        assert!(!error.is_retryable());
        assert!(!error.safe_message.contains(body));
        assert!(!error.safe_message.contains("sensitive-test-key"));
    }
}

/// 响应头后的提前断开属于可重试传输失败，而不是不可重试内容错误。
#[tokio::test]
async fn maps_truncated_response_body_to_transport() {
    let base = serve_truncated_body("{", 100);
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "sensitive-test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 2_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::Transport);
    assert!(error.is_retryable());
    assert!(!error.safe_message.contains("sensitive-test-key"));
}

/// 响应正文停滞遵守请求超时并返回可重试超时分类。
#[tokio::test]
async fn maps_stalled_response_body_to_timeout() {
    let base = serve_stalling_body("{", 100);
    let client = OpenAiCompatibleClient::new().unwrap();
    let error = client
        .generate_text(
            &config(base),
            "sensitive-test-key",
            &TextGenerationRequest {
                model: "text-model".into(),
                prompt: "标题".into(),
                parameters: TextGenerationParameters {
                    timeout_ms: 1_000,
                    ..TextGenerationParameters::default()
                },
                idempotency_key: "task:v1:article".into(),
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::Timeout);
    assert!(error.is_retryable());
    assert!(!error.safe_message.contains("sensitive-test-key"));
}
