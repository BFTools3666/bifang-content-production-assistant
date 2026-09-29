use super::*;

/// 图片适配器接受有效 Base64，并在网络边界拒绝不可解码载荷。
#[tokio::test]
async fn validates_image_base64_payload() {
    let (valid_base, captured) = serve_once_capturing(
        "200 OK",
        &[],
        r#"{"data":[{"b64_json":"aGVsbG8="}],"usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#,
    );
    let client = OpenAiCompatibleClient::new().unwrap();
    let result = client
        .generate_image_with_contract(
            &config(valid_base),
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
            ImageRequestContract::ProviderDefaultBase64V2,
        )
        .await
        .unwrap();
    assert_eq!(result.image_base64.as_deref(), Some("aGVsbG8="));
    assert_eq!(
        result.token_usage,
        Some(TokenUsage {
            input_tokens: Some(7),
            output_tokens: Some(11),
            total_tokens: Some(18),
        })
    );
    let request = String::from_utf8(captured.recv().unwrap()).unwrap();
    assert!(request.starts_with("POST /v1/images/generations HTTP/1.1\r\n"));
    let body = request.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(
        body,
        r#"{"model":"image-model","prompt":"配图","size":"1024x1024"}"#
    );

    let (legacy_base, legacy_captured) =
        serve_once_capturing("200 OK", &[], r#"{"data":[{"b64_json":"aGVsbG8="}]}"#);
    let legacy_result = client
        .generate_image_with_contract(
            &config(legacy_base),
            "test-key",
            &ImageGenerationRequest {
                model: "image-model".into(),
                prompt: "配图".into(),
                parameters: ImageGenerationParameters {
                    size: "1024x1024".into(),
                    timeout_ms: 2_000,
                    ..ImageGenerationParameters::default()
                },
                idempotency_key: "legacy-task:v1:image".into(),
            },
            ImageRequestContract::ExplicitBase64V1,
        )
        .await
        .unwrap();
    assert!(legacy_result.token_usage.is_none());
    let legacy_request = String::from_utf8(legacy_captured.recv().unwrap()).unwrap();
    assert!(
        legacy_request
            .to_ascii_lowercase()
            .contains("idempotency-key: legacy-task:v1:image\r\n")
    );
    let legacy_body = legacy_request.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(
        legacy_body,
        r#"{"model":"image-model","prompt":"配图","size":"1024x1024","response_format":"b64_json"}"#
    );

    let invalid_base = serve_once(
        "200 OK",
        &[],
        r#"{"data":[{"b64_json":"%%% sensitive invalid base64"}],"usage":{"output_tokens":9,"total_tokens":15}}"#,
    );
    let error = client
        .generate_image_with_usage_error(
            &config(invalid_base),
            "sensitive-test-key",
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
            ImageRequestContract::ProviderDefaultBase64V2,
        )
        .await
        .unwrap_err();
    assert_eq!(error.error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.error.is_retryable());
    assert_eq!(
        error.token_usage,
        Some(TokenUsage {
            input_tokens: None,
            output_tokens: Some(9),
            total_tokens: Some(15),
        })
    );
    assert!(
        !error
            .error
            .safe_message
            .contains("sensitive invalid base64")
    );
    assert!(!error.error.safe_message.contains("sensitive-test-key"));
}
