//! CloudBase 统计路由和外部版本网关凭据的失效保护配置。

use bifang_content_production_assistant_core::public_services::{
    is_safe_public_https_url, validate_application_version_number,
};
use reqwest::{
    Method, Request, Url,
    header::{ACCEPT, HeaderValue},
};

use super::{APP_ID, PublicServicesError};

const TELEMETRY_ORIGIN: &str =
    "https://bftoos-sh-d3ga7hahz59229f0a-1484029710.ap-shanghai.app.tcloudbase.com";
const VERSION_BASE_URL: &str = "https://gateway.kaiwhale.com/tools/version-control/v1";
const VERSION_APP_KEY: &str = "versionControl";
const VERSION_APP_SECRET: &str = "ef430c0a8fc8068c";
const APP_KEY_HEADER: &str = "x-app-key";
const APP_SECRET_HEADER: &str = "x-app-secret";

/// 固定到当前 APP_ID 的公开 telemetry 端点。
#[derive(Clone)]
pub(super) struct TelemetryConfig {
    endpoint: Url,
}

impl TelemetryConfig {
    /// 从受版本控制的环境和应用事实构造端点。
    pub(super) fn registered() -> Option<Self> {
        // 小红书发行版尚未登记独立公共服务，不能使用 SEO 版的服务身份。
        if cfg!(feature = "xhs") {
            return None;
        }
        let endpoint = format!("{TELEMETRY_ORIGIN}/apps/{APP_ID}/v1/track");
        if !is_safe_public_https_url(&endpoint) {
            return None;
        }
        let url = Url::parse(&endpoint).ok()?;
        (url.host_str()?.ends_with(".app.tcloudbase.com")
            && url.path() == format!("/apps/{APP_ID}/v1/track"))
        .then_some(Self { endpoint: url })
    }

    /// 返回已经严格校验的固定端点。
    pub(super) fn endpoint(&self) -> Url {
        self.endpoint.clone()
    }
}

/// Rust 可信层持有的固定版本请求和可轮换低权限客户端令牌。
#[derive(Clone)]
pub(super) struct VersionApiConfig {
    request_url: Url,
    app_key: HeaderValue,
    app_secret: HeaderValue,
}

impl VersionApiConfig {
    /// 从受版本控制的固定网关、发布流与应用凭据构造请求。
    pub(super) fn registered() -> Option<Self> {
        // 小红书发行版尚未登记独立公共服务，不能使用 SEO 版的服务身份。
        if cfg!(feature = "xhs") {
            return None;
        }
        Self::from_values(
            release_code(),
            Some(VERSION_APP_KEY),
            Some(VERSION_APP_SECRET),
        )
    }

    /// 构造无 body、无 query、只含固定认证头的 GET 请求。
    pub(super) fn build_request(
        &self,
        client: &reqwest::Client,
    ) -> Result<Request, PublicServicesError> {
        client
            .request(Method::GET, self.request_url.clone())
            .header(ACCEPT, "application/json")
            .header(APP_KEY_HEADER, self.app_key.clone())
            .header(APP_SECRET_HEADER, self.app_secret.clone())
            .build()
            .map_err(|_| PublicServicesError::InvalidResponse)
    }

    fn from_values(
        release_code: Option<&str>,
        app_key: Option<&str>,
        app_secret: Option<&str>,
    ) -> Option<Self> {
        let mut app_key = credential_header(app_key?)?;
        let mut app_secret = credential_header(app_secret?)?;
        app_key.set_sensitive(true);
        app_secret.set_sensitive(true);
        Some(Self {
            request_url: build_version_url(release_code?, env!("CARGO_PKG_VERSION"))?,
            app_key,
            app_secret,
        })
    }
}

fn credential_header(value: &str) -> Option<HeaderValue> {
    if value.is_empty()
        || value.len() > 512
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return None;
    }
    HeaderValue::from_str(value).ok()
}

fn build_version_url(code: &str, current_version: &str) -> Option<Url> {
    validate_application_version_number(current_version).ok()?;
    if !is_release_code(code) {
        return None;
    }
    if !is_safe_public_https_url(VERSION_BASE_URL) {
        return None;
    }
    let mut url = Url::parse(VERSION_BASE_URL).ok()?;
    {
        let mut segments = url.path_segments_mut().ok()?;
        segments
            .pop_if_empty()
            .push("versions")
            .push(code)
            .push(current_version);
    }
    Some(url)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn release_code() -> Option<&'static str> {
    Some("bifang-content-production-assistant-windows")
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn release_code() -> Option<&'static str> {
    Some("bifang-content-production-assistant-macos-arm64")
}

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
fn release_code() -> Option<&'static str> {
    None
}

fn is_release_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (byte == b'-' && index > 0 && index + 1 < value.len())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "xhs"))]
    #[test]
    fn public_routes_match_registered_cloudbase_resources() {
        assert_eq!(
            TelemetryConfig::registered()
                .expect("公开统计路由应有效")
                .endpoint()
                .as_str(),
            "https://bftoos-sh-d3ga7hahz59229f0a-1484029710.ap-shanghai.app.tcloudbase.com/apps/bifang-content-production-assistant/v1/track"
        );
        let code = release_code().expect("受支持目标应有发布流");
        assert_eq!(
            build_version_url(code, env!("CARGO_PKG_VERSION"))
                .expect("公开版本路由应有效")
                .as_str(),
            format!(
                "{VERSION_BASE_URL}/versions/{code}/{}",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[cfg(feature = "xhs")]
    #[test]
    fn xhs_does_not_use_seo_public_service_routes() {
        assert_eq!(APP_ID, "bifang-xiaohongshu-assistant");
        assert!(TelemetryConfig::registered().is_none());
        assert!(VersionApiConfig::registered().is_none());
        let transport = super::super::transport::ReqwestPublicServicesTransport::create();
        assert!(!transport.telemetry_configured());
        assert!(!transport.updates_configured());
    }

    #[test]
    fn missing_or_invalid_credentials_disable_version_requests() {
        let code = release_code().expect("受支持目标应有发布流");
        assert!(VersionApiConfig::from_values(Some(code), None, None).is_none());
        assert!(VersionApiConfig::from_values(Some(code), Some("offline-key"), None).is_none());
        assert!(
            VersionApiConfig::from_values(Some(code), Some("bad\r\nkey"), Some("offline-secret"))
                .is_none()
        );
    }

    #[cfg(not(feature = "xhs"))]
    #[test]
    fn request_uses_one_v1_path_and_sensitive_headers() {
        let config = VersionApiConfig::registered().expect("固定版本配置应有效");
        let request = config
            .build_request(&reqwest::Client::new())
            .expect("请求应可构造");
        assert_eq!(request.method(), Method::GET);
        assert!(request.url().query().is_none());
        assert_eq!(request.headers().len(), 3);
        assert_eq!(request.headers()[APP_KEY_HEADER], VERSION_APP_KEY);
        assert!(request.headers()[APP_KEY_HEADER].is_sensitive());
        assert!(request.headers()[APP_SECRET_HEADER].is_sensitive());
    }
}
