use super::*;

/// 从火山方舟返回的受限 TOS 地址立即下载图片，不携带生成请求凭据。
pub(super) async fn download_ark_image(
    value: &str,
    timeout: Duration,
) -> Result<Vec<u8>, ProviderError> {
    let url = validate_ark_image_url(value)?;
    let host = url
        .host_str()
        .ok_or_else(|| invalid_response("火山方舟图片下载地址无效"))?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| invalid_response("火山方舟图片下载地址无效"))?;
    let lookup_host = host.clone();
    let lookup_timeout = timeout.min(Duration::from_secs(10));
    let addresses = tokio::time::timeout(
        lookup_timeout,
        tauri::async_runtime::spawn_blocking(move || {
            (lookup_host.as_str(), port)
                .to_socket_addrs()
                .map(|addresses| addresses.collect::<Vec<_>>())
        }),
    )
    .await
    .map_err(|_| transport_error("解析 AI 图片下载地址超时"))?
    .map_err(|_| transport_error("无法解析 AI 图片下载地址"))?
    .map_err(|_| transport_error("无法解析 AI 图片下载地址"))?;
    if !ark_download_addresses_are_safe(&url, addresses.iter().map(|address| address.ip())) {
        return Err(invalid_response("火山方舟图片下载地址未通过安全校验"));
    }

    // 保留原始 TOS host 作为 HTTP Host 与 TLS SNI，只把已校验的 DNS 结果固定给客户端。
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .resolve_to_addrs(&host, &addresses)
        .build()
        .map_err(|_| transport_error("无法初始化 AI 图片下载客户端"))?;
    let response = client
        .get(url)
        .header(header::ACCEPT, "image/png, image/jpeg, image/webp")
        .timeout(timeout)
        .send()
        .await
        .map_err(map_reqwest_error)?;
    let status = response.status();
    if !status.is_success() {
        return Err(ProviderError::from_http_status(
            status.as_u16(),
            parse_retry_after_ms(response.headers()),
        ));
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(supported_image_content_type)
        .ok_or_else(|| invalid_response("AI 图片下载响应类型无效"))?;
    let body = read_limited_body(
        response,
        MAX_DOWNLOADED_IMAGE_BYTES,
        "AI 图片下载超过安全大小上限",
    )
    .await?;
    if body.is_empty() || !image_bytes_match_content_type(content_type, &body) {
        return Err(invalid_response("AI 图片下载内容格式无效"));
    }
    Ok(body)
}

/// 校验方舟图片 URL 的结构和 TOS 主机边界；测试只额外允许本机 HTTP 夹具。
pub(super) fn validate_ark_image_url(value: &str) -> Result<Url, ProviderError> {
    let has_canonical_authority = url_text_has_canonical_authority(value);
    let has_credentials = url_text_has_credentials(value);
    let url = Url::parse(value).map_err(|_| invalid_response("火山方舟图片下载地址无效"))?;
    let loopback_test = is_loopback_test_url(&url);
    if (!loopback_test && !is_official_ark_tos_url(&url))
        || !has_canonical_authority
        || has_credentials
        || url.fragment().is_some()
    {
        return Err(invalid_response("火山方舟图片下载地址未通过安全校验"));
    }
    Ok(url)
}

/// URL 原文必须恰好以标准 `scheme://authority` 起始；拒绝解析器会自动修复的额外斜杠或反斜杠。
pub(super) fn url_text_has_canonical_authority(value: &str) -> bool {
    if value.contains('\\') || value.bytes().any(|byte| byte <= b' ' || byte == 0x7f) {
        return false;
    }
    value.split_once("://").is_some_and(|(scheme, remainder)| {
        !scheme.is_empty()
            && !remainder.starts_with('/')
            && remainder
                .split(|character| matches!(character, '/' | '?' | '#'))
                .next()
                .is_some_and(|authority| !authority.is_empty())
    })
}

/// 只认 HTTPS、443 且无凭据/片段的北京 TOS 官方主机；签名查询参数仍由原 URL 保留。
pub(super) fn is_official_ark_tos_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|host| {
            host == "tos-cn-beijing.volces.com" || host.ends_with(ARK_IMAGE_HOST_SUFFIX)
        })
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

/// 在 URL 归一化前通过 authority 的 `@` 分隔符识别凭据，也覆盖空用户名。
pub(super) fn url_text_has_credentials(value: &str) -> bool {
    match value.split_once("://") {
        Some((_, remainder)) => remainder
            .split(|character| matches!(character, '/' | '?' | '#'))
            .next()
            .is_some_and(|authority| authority.contains('@')),
        // `url` 会把 `https:@host`、`https:/@host` 等非标准写法归一化；在此之前
        // 发现 `@` 时失败关闭，避免空 userinfo 在解析后消失。
        None => value.contains('@'),
    }
}

/// 仅测试构建允许明确的本机 HTTP 服务模拟方舟和 TOS 双请求。
pub(super) fn is_loopback_test_url(url: &Url) -> bool {
    cfg!(test)
        && url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "::1"))
}

/// 校验一次 DNS 答案集合；官方 TOS 可接受全公网或纯 Clash/TUN fake-IP，禁止混合降级。
pub(super) fn ark_download_addresses_are_safe(
    url: &Url,
    addresses: impl IntoIterator<Item = IpAddr>,
) -> bool {
    let addresses = addresses.into_iter();
    if is_loopback_test_url(url) {
        return addresses.count() > 0;
    }
    if !is_official_ark_tos_url(url) {
        return false;
    }

    let mut saw_address = false;
    let mut all_public = true;
    let mut all_clash_tun_fake = true;
    for address in addresses {
        saw_address = true;
        all_public &= is_public_download_ip(address);
        all_clash_tun_fake &= is_clash_tun_fake_ip(address);
    }
    saw_address && (all_public || all_clash_tun_fake)
}

/// 只识别 Clash/TUN 常用的 IPv4 benchmark fake-IP 网段，不把它提升为通用公网地址。
pub(super) fn is_clash_tun_fake_ip(address: IpAddr) -> bool {
    matches!(address, IpAddr::V4(address) if {
        let [first, second, _, _] = address.octets();
        first == 198 && matches!(second, 18 | 19)
    })
}

/// 火山方舟 Provider 只允许固定北京 API v3 图片端点；测试可使用本机夹具。
pub(crate) fn validate_ark_generation_endpoint(endpoint: &Url) -> Result<(), ProviderError> {
    let host = endpoint
        .host_str()
        .ok_or_else(|| invalid_request("火山方舟服务地址无效"))?;
    let loopback_test = cfg!(test)
        && endpoint.scheme() == "http"
        && matches!(host, "127.0.0.1" | "localhost" | "::1");
    if !loopback_test
        && (endpoint.scheme() != "https"
            || host != "ark.cn-beijing.volces.com"
            || endpoint.port_or_known_default() != Some(443)
            || endpoint.path() != "/api/v3/images/generations")
    {
        return Err(invalid_request(
            "火山方舟服务地址必须是 https://ark.cn-beijing.volces.com/api/v3",
        ));
    }
    Ok(())
}

/// 只允许公网地址，避免下载 URL 访问本机、局域网、链路本地或保留网段。
pub(super) fn is_public_download_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

/// 判断 IPv4 地址是否属于可连接公网范围。
pub(super) fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, third, _] = address.octets();
    !(address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_unspecified()
        || address.is_multicast()
        || first == 0
        || (first == 100 && (64..=127).contains(&second))
        || (first == 192 && second == 0 && third == 0)
        || (first == 198 && matches!(second, 18 | 19))
        || first >= 240)
}

/// 判断 IPv6 地址是否属于公网单播范围，并拒绝映射出的非公网 IPv4。
pub(super) fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && !address.is_unique_local()
        && !address.is_unicast_link_local()
        && segments[0] & 0xe000 == 0x2000
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
}

/// 规范化并验证允许的远端图片 MIME。
pub(super) fn supported_image_content_type(value: &str) -> Option<&'static str> {
    match value
        .split(';')
        .next()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("image/png") => Some("image/png"),
        Some("image/jpeg") => Some("image/jpeg"),
        Some("image/webp") => Some("image/webp"),
        _ => None,
    }
}

/// 同时校验 MIME 和魔数，避免把 HTML 或伪装载荷交给图片解码器。
pub(super) fn image_bytes_match_content_type(content_type: &str, bytes: &[u8]) -> bool {
    match content_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

/// 响应头到达后的读取失败属于超时或传输中断，而不是内容格式错误。
pub(super) fn map_response_body_error(error: reqwest::Error) -> ProviderError {
    let kind = if error.is_timeout() {
        ProviderErrorKind::Timeout
    } else {
        ProviderErrorKind::Transport
    };
    ProviderError {
        kind,
        safe_message: match kind {
            ProviderErrorKind::Timeout => "读取 AI 服务响应超时",
            _ => "AI 服务响应在传输过程中中断",
        }
        .into(),
        retry_after_ms: None,
    }
}
