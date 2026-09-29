use super::*;

/// 方舟适配器精确发送用户契约，并立即下载 URL 且不转发任何生成凭据。
#[tokio::test]
async fn generates_and_downloads_volcengine_ark_image_without_forwarding_credentials() {
    const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let expected_image = BASE64.decode(PNG_BASE64).unwrap();
    let (download_url, downloaded_request) =
        serve_binary_once("200 OK", "image/png", &[], expected_image.clone());
    let response_body = serde_json::json!({
        "data": [{"url": download_url}],
        "usage": {"generated_images": 1, "output_tokens": 144, "total_tokens": 144}
    })
    .to_string();
    let (fixture_base, generated_request) =
        serve_once_capturing("200 OK", &[("x-tt-logid", "ark-request-1")], &response_body);
    let api_base = format!("{}/api/v3", fixture_base.trim_end_matches("/v1"));
    let mut provider = config(api_base);
    provider.kind = ProviderKind::VolcengineArkImage;
    let client = OpenAiCompatibleClient::new().unwrap();

    let result = client
        .generate_image_with_contract(
            &provider,
            "sensitive-ark-key",
            &ImageGenerationRequest {
                model: "ep-20260318093407-x5qm5".into(),
                prompt: "星际穿越，黑洞".into(),
                parameters: ImageGenerationParameters {
                    size: "2K".into(),
                    watermark: true,
                    timeout_ms: 2_000,
                },
                idempotency_key: "must-not-be-sent-to-ark".into(),
            },
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .unwrap();

    assert_eq!(result.provider_request_id.as_deref(), Some("ark-request-1"));
    assert_eq!(
        result.token_usage,
        Some(TokenUsage {
            input_tokens: None,
            output_tokens: Some(144),
            total_tokens: Some(144),
        })
    );
    assert_eq!(
        BASE64.decode(result.image_base64.unwrap()).unwrap(),
        expected_image
    );
    assert!(result.image_url.is_none());

    let generated_request = String::from_utf8(generated_request.recv().unwrap()).unwrap();
    let generated_lower = generated_request.to_ascii_lowercase();
    assert!(generated_request.starts_with("POST /api/v3/images/generations HTTP/1.1\r\n"));
    assert!(generated_lower.contains("authorization: bearer sensitive-ark-key\r\n"));
    assert!(!generated_lower.contains("idempotency-key:"));
    let body = generated_request.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(body).unwrap(),
        serde_json::json!({
            "model": "ep-20260318093407-x5qm5",
            "prompt": "星际穿越，黑洞",
            "sequential_image_generation": "disabled",
            "response_format": "url",
            "size": "2K",
            "stream": false,
            "watermark": true
        })
    );

    let downloaded_request = String::from_utf8(downloaded_request.recv().unwrap()).unwrap();
    let downloaded_lower = downloaded_request.to_ascii_lowercase();
    assert!(downloaded_request.starts_with("GET /generated.png?x-tos-signature=test HTTP/1.1\r\n"));
    assert!(!downloaded_lower.contains("authorization:"));
    assert!(!downloaded_lower.contains("idempotency-key:"));
    assert!(!downloaded_request.contains("sensitive-ark-key"));
    assert!(!downloaded_request.contains("must-not-be-sent-to-ark"));
}

/// 方舟返回非 TOS 主机时必须在建立下载连接前安全拒绝。
#[tokio::test]
async fn rejects_untrusted_volcengine_ark_image_url() {
    let sensitive_url = "https://example.com/private.png?secret=signed-value";
    let response_body = serde_json::json!({"data": [{"url": sensitive_url}]}).to_string();
    let (fixture_base, _) = serve_once_capturing("200 OK", &[], &response_body);
    let api_base = format!("{}/api/v3", fixture_base.trim_end_matches("/v1"));
    let mut provider = config(api_base);
    provider.kind = ProviderKind::VolcengineArkImage;

    let error = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_image_with_contract(
            &provider,
            "sensitive-ark-key",
            &ImageGenerationRequest {
                model: "ark-image".into(),
                prompt: "配图".into(),
                parameters: ImageGenerationParameters::volcengine_ark_default(),
                idempotency_key: "not-forwarded".into(),
            },
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(error.safe_message.contains("下载地址未通过安全校验"));
    assert!(!error.safe_message.contains("example.com"));
    assert!(!error.safe_message.contains("signed-value"));
    assert!(!error.safe_message.contains("sensitive-ark-key"));
}

/// 即使下载字节有图片魔数，非图片 MIME 仍必须失败关闭且不自动重复生成。
#[tokio::test]
async fn rejects_volcengine_ark_download_with_non_image_content_type() {
    let (download_url, _) = serve_binary_once(
        "200 OK",
        "text/html",
        &[],
        b"\x89PNG\r\n\x1a\nnot-an-image-response".to_vec(),
    );
    let response_body = serde_json::json!({
        "data": [{"url": download_url}],
        "usage": {"generated_images": 1, "output_tokens": 37, "total_tokens": 41}
    })
    .to_string();
    let (fixture_base, _) = serve_once_capturing("200 OK", &[], &response_body);
    let api_base = format!("{}/api/v3", fixture_base.trim_end_matches("/v1"));
    let mut provider = config(api_base);
    provider.kind = ProviderKind::VolcengineArkImage;

    let error = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_image_with_usage_error(
            &provider,
            "test-key",
            &ImageGenerationRequest {
                model: "ark-image".into(),
                prompt: "配图".into(),
                parameters: ImageGenerationParameters::volcengine_ark_default(),
                idempotency_key: "not-forwarded".into(),
            },
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .unwrap_err();

    assert_eq!(error.error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.error.is_retryable());
    assert_eq!(
        error.token_usage,
        Some(TokenUsage {
            input_tokens: None,
            output_tokens: Some(37),
            total_tokens: Some(41),
        })
    );
    assert!(error.error.safe_message.contains("已生成图片"));
    assert!(error.error.safe_message.contains("下载响应类型无效"));
    assert!(error.error.safe_message.contains("不会自动重试"));
}

/// 方舟生成成功后的临时 URL 失效不能误报为 API 密钥错误或进入付费重试。
#[tokio::test]
async fn marks_post_generation_download_http_failure_as_non_retryable() {
    let (download_url, _) = serve_binary_once("403 Forbidden", "text/plain", &[], Vec::new());
    let response_body = serde_json::json!({
        "data": [{"url": download_url}],
        "usage": {"generated_images": 1, "output_tokens": 37, "total_tokens": 41}
    })
    .to_string();
    let (fixture_base, _) = serve_once_capturing("200 OK", &[], &response_body);
    let api_base = format!("{}/api/v3", fixture_base.trim_end_matches("/v1"));
    let mut provider = config(api_base);
    provider.kind = ProviderKind::VolcengineArkImage;

    let error = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_image_with_usage_error(
            &provider,
            "test-key",
            &ImageGenerationRequest {
                model: "ark-image".into(),
                prompt: "配图".into(),
                parameters: ImageGenerationParameters::volcengine_ark_default(),
                idempotency_key: "not-forwarded".into(),
            },
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .unwrap_err();

    assert_eq!(error.error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.error.is_retryable());
    assert_eq!(
        error.token_usage,
        Some(TokenUsage {
            input_tokens: None,
            output_tokens: Some(37),
            total_tokens: Some(41),
        })
    );
    assert!(error.error.safe_message.contains("不会自动重试"));
    assert!(
        error
            .error
            .safe_message
            .contains("临时图片下载地址已失效或被拒绝")
    );
    assert!(!error.error.safe_message.contains("鉴权失败"));
}

/// 方舟 POST 已发出但响应头超时时，普通重试不能再次发起可能重复计费的生成。
#[tokio::test]
async fn marks_ambiguous_volcengine_ark_post_timeout_as_non_retryable() {
    let fixture_base = serve_delayed_response();
    let api_base = format!("{}/api/v3", fixture_base.trim_end_matches("/v1"));
    let mut provider = config(api_base);
    provider.kind = ProviderKind::VolcengineArkImage;

    let error = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_image_with_contract(
            &provider,
            "test-key",
            &ImageGenerationRequest {
                model: "ark-image".into(),
                prompt: "配图".into(),
                parameters: ImageGenerationParameters {
                    timeout_ms: 1_000,
                    ..ImageGenerationParameters::volcengine_ark_default()
                },
                idempotency_key: "not-forwarded".into(),
            },
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.is_retryable());
    assert!(error.safe_message.contains("结果无法确认"));
    assert!(error.safe_message.contains("不会自动重试"));
}

/// 方舟 2xx 响应正文中断同样表示生成结果未知，不能转成普通网络重试。
#[tokio::test]
async fn marks_ambiguous_volcengine_ark_response_body_as_non_retryable() {
    let fixture_base = serve_stalling_body("{\"data\":[", 100);
    let api_base = format!("{}/api/v3", fixture_base.trim_end_matches("/v1"));
    let mut provider = config(api_base);
    provider.kind = ProviderKind::VolcengineArkImage;

    let error = OpenAiCompatibleClient::new()
        .unwrap()
        .generate_image_with_contract(
            &provider,
            "test-key",
            &ImageGenerationRequest {
                model: "ark-image".into(),
                prompt: "配图".into(),
                parameters: ImageGenerationParameters {
                    timeout_ms: 1_000,
                    ..ImageGenerationParameters::volcengine_ark_default()
                },
                idempotency_key: "not-forwarded".into(),
            },
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(!error.is_retryable());
    assert!(error.safe_message.contains("结果无法确认"));
}

/// 下载客户端不跟随 TOS 返回的重定向，也不会访问 Location 目标。
#[tokio::test]
async fn refuses_volcengine_ark_image_redirect() {
    let redirected = TcpListener::bind("127.0.0.1:0").unwrap();
    redirected.set_nonblocking(true).unwrap();
    let location = format!("http://{}/capture", redirected.local_addr().unwrap());
    let (download_url, _) = serve_binary_once(
        "302 Found",
        "text/plain",
        &[("Location", location.as_str())],
        Vec::new(),
    );

    let error = download_ark_image(&download_url, Duration::from_secs(2))
        .await
        .unwrap_err();

    assert_eq!(error.kind, ProviderErrorKind::InvalidResponse);
    assert!(matches!(
        redirected.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
}

/// URL 与通用公网策略拒绝凭据、片段、非 TOS 主机和全部本地/保留地址。
#[test]
fn validates_volcengine_ark_download_boundaries() {
    assert!(
            validate_ark_image_url(
                "https://ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png?X-Tos-Signature=test"
            )
            .is_ok()
        );
    for value in [
        "http://ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https:@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https:/@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https:////@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https:////ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https:\\\\ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://\t/@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://\t/ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://user@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://user:@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://user:password@ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png",
        "https://example.com/image.png",
        "https://ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com:444/image.png",
        "https://ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png#fragment",
    ] {
        assert!(validate_ark_image_url(value).is_err(), "accepted {value}");
    }
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "100.64.0.1",
        "169.254.169.254",
        "192.168.1.1",
        "198.18.0.1",
        "224.0.0.1",
        "::1",
        "fd00::1",
        "fe80::1",
        "2001:db8::1",
    ] {
        assert!(
            !is_public_download_ip(address.parse().unwrap()),
            "accepted {address}"
        );
    }
    assert!(is_public_download_ip("8.8.8.8".parse().unwrap()));
    assert!(is_public_download_ip(
        "2606:4700:4700::1111".parse().unwrap()
    ));
    assert!(image_bytes_match_content_type(
        "image/png",
        b"\x89PNG\r\n\x1a\nrest"
    ));
    assert!(!image_bytes_match_content_type(
        "image/jpeg",
        b"\x89PNG\r\n\x1a\nrest"
    ));
}

/// 仅官方 TOS 可使用纯 198.18.0.0/15 DNS 集合；混合答案和其他私网继续失败关闭。
#[test]
fn accepts_only_pure_clash_tun_fake_ips_for_official_ark_tos() {
    let official_url = validate_ark_image_url(
            "https://ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png?X-Tos-Signature=sensitive",
        )
        .unwrap();
    let untrusted_url = Url::parse("https://example.com/image.png").unwrap();

    assert!(ark_download_addresses_are_safe(
        &official_url,
        ["198.18.0.0", "198.19.255.255"].map(|address| address.parse().unwrap()),
    ));
    assert!(ark_download_addresses_are_safe(
        &official_url,
        ["8.8.8.8", "2606:4700:4700::1111"].map(|address| address.parse().unwrap()),
    ));
    assert!(!ark_download_addresses_are_safe(
        &official_url,
        ["198.18.0.1", "8.8.8.8"].map(|address| address.parse().unwrap()),
    ));
    assert!(!ark_download_addresses_are_safe(
        &official_url,
        ["198.18.0.1", "192.168.1.1"].map(|address| address.parse().unwrap()),
    ));
    assert!(!ark_download_addresses_are_safe(
        &official_url,
        ["8.8.8.8", "10.0.0.1"].map(|address| address.parse().unwrap()),
    ));
    assert!(!ark_download_addresses_are_safe(
        &official_url,
        ["10.0.0.1"].map(|address| address.parse().unwrap()),
    ));
    for address in ["198.17.255.255", "198.20.0.0"] {
        let address = address.parse().unwrap();
        assert!(!is_clash_tun_fake_ip(address));
        assert!(ark_download_addresses_are_safe(&official_url, [address]));
    }
    assert!(!ark_download_addresses_are_safe(
        &official_url,
        std::iter::empty(),
    ));
    assert!(!ark_download_addresses_are_safe(
        &untrusted_url,
        ["198.18.0.1"].map(|address| address.parse().unwrap()),
    ));
    assert!(!is_public_download_ip("198.18.0.1".parse().unwrap()));
}

/// 生成后的错误保留静态具体原因，并对意外 URL/query 内容执行防御性隐藏。
#[test]
fn preserves_safe_ark_download_failure_detail_without_url_or_query() {
    let detailed =
        ark_post_generation_download_failure(invalid_response("AI 图片下载响应类型无效"));
    assert!(detailed.safe_message.contains("AI 图片下载响应类型无效"));
    assert!(!detailed.is_retryable());

    let redacted = ark_post_generation_download_failure(invalid_response(
        "https://ark-content-generation-v2-cn-beijing.tos-cn-beijing.volces.com/image.png?X-Tos-Signature=secret",
    ));
    assert!(redacted.safe_message.contains("具体下载原因已隐藏"));
    assert!(!redacted.safe_message.contains("tos-cn-beijing"));
    assert!(!redacted.safe_message.contains("X-Tos-Signature"));
    assert!(!redacted.safe_message.contains("secret"));
}
