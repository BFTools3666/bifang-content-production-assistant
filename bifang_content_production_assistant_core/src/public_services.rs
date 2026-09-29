//! 毕方公共服务的稳定数据契约与本地安全判定。

use std::net::{Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const TELEMETRY_SCHEMA_VERSION: u8 = 2;
const PUBLIC_SERVICES_SDK_VERSION: &str = "3.0.0";

/// 公共服务协议支持的平台标识。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PublicPlatform {
    /// Windows 桌面系统。
    Windows,
    /// macOS 桌面系统。
    Macos,
}

/// 公共服务协议支持的处理器架构。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PublicArchitecture {
    /// 64 位 x86。
    X64,
    /// 64 位 ARM。
    Arm64,
    /// 32 位 x86。
    Ia32,
}

/// 每日启动统计的固定十字段载荷。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DailyActivePayload {
    /// 协议版本，固定为 2。
    pub schema_version: u8,
    /// 本次尝试独立生成的 UUID v4。
    pub event_id: String,
    /// 事件名，固定为 `daily_active`。
    pub event: String,
    /// 运营助手登记的应用标识。
    pub app_id: String,
    /// 当前应用 SemVer。
    pub app_version: String,
    /// 仅本应用使用的随机安装 UUID v4。
    pub install_id: String,
    /// 当前平台。
    pub platform: PublicPlatform,
    /// 当前架构。
    pub arch: PublicArchitecture,
    /// 公共服务协议实现版本。
    pub sdk_version: String,
    /// 事件产生时刻的 UTC RFC 3339 文本。
    pub occurred_at: String,
}

impl DailyActivePayload {
    /// 构造并验证一条隐私最小化的启动统计载荷。
    pub fn new(
        app_id: &str,
        app_version: &str,
        install_id: &str,
        platform: PublicPlatform,
        arch: PublicArchitecture,
        occurred_at: &str,
    ) -> Result<Self, PublicServiceContractError> {
        validate_app_id(app_id)?;
        validate_semver(app_version)?;
        validate_uuid_v4(install_id)?;
        validate_timestamp(occurred_at)?;
        Ok(Self {
            schema_version: TELEMETRY_SCHEMA_VERSION,
            event_id: Uuid::new_v4().to_string(),
            event: "daily_active".to_owned(),
            app_id: app_id.to_owned(),
            app_version: app_version.to_owned(),
            install_id: install_id.to_owned(),
            platform,
            arch,
            sdk_version: PUBLIC_SERVICES_SDK_VERSION.to_owned(),
            occurred_at: occurred_at.to_owned(),
        })
    }
}

/// application-version API v1 返回的严格七字段对象。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ApplicationVersionResponse {
    /// 服务端是否提供更新。
    pub update: bool,
    /// 经校验后才允许交给系统浏览器的下载地址。
    pub download_url: String,
    /// 作为不可信纯文本展示的提示标题。
    pub prompt_content: String,
    /// 是否必须更新后才能继续使用。
    pub forced_update: bool,
    /// 是否显式提示本次更新。
    pub prompt_update: bool,
    /// 是否暂停当前发布流的更新分发。
    pub stop_update: bool,
    /// 兼容字段；前端仍只能把它作为纯文本展示。
    pub update_content: String,
}

/// 对已验证七字段响应作出的客户端决定。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationVersionDecision {
    /// 当前版本没有可展示更新。
    None,
    /// 普通或提示更新，不阻断产品功能。
    Optional(ApplicationVersionResponse),
    /// 强制更新，由根级门阻断普通功能。
    Required(ApplicationVersionResponse),
    /// 服务端暂停更新分发，不解释为停止应用。
    Stopped(ApplicationVersionResponse),
}

/// 公共服务核心拒绝的稳定输入类别。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicServiceContractError {
    /// 应用标识不符合公共协议。
    InvalidAppId,
    /// 版本不是严格三段 SemVer。
    InvalidSemver,
    /// 安装标识不是 UUID v4。
    InvalidUuidV4,
    /// 时间戳不是有效 RFC 3339。
    InvalidTimestamp,
    /// 更新响应结构或字段不符合冻结契约。
    InvalidApplicationVersionResponse,
    /// 版本检查路径不是数字点分版本。
    InvalidApplicationVersionNumber,
}

/// 验证七字段响应并收敛为无更新、可选、强制或暂停状态。
pub fn decide_application_version(
    response: ApplicationVersionResponse,
) -> Result<ApplicationVersionDecision, PublicServiceContractError> {
    validate_application_version_response(&response)?;
    if response.stop_update {
        return Ok(ApplicationVersionDecision::Stopped(response));
    }
    if !response.update {
        return Ok(ApplicationVersionDecision::None);
    }
    if response.forced_update {
        return Ok(ApplicationVersionDecision::Required(response));
    }
    Ok(ApplicationVersionDecision::Optional(response))
}

/// 验证 application-version 路径要求的数字点分版本号。
pub fn validate_application_version_number(value: &str) -> Result<(), PublicServiceContractError> {
    let valid = !value.is_empty()
        && value.len() <= 50
        && value.split('.').all(|segment| {
            !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
        });
    valid
        .then_some(())
        .ok_or(PublicServiceContractError::InvalidApplicationVersionNumber)
}

/// 判断外部链接是否是不含凭据、指向公共网络的 HTTPS 地址。
#[must_use]
pub fn is_safe_public_https_url(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 2_048
        || !value.is_ascii()
        || value.chars().any(char::is_whitespace)
        || value.contains(['\\', '#'])
    {
        return false;
    }
    let Some(remainder) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = remainder.split(['/', '?']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        let Some((host, suffix)) = bracketed.split_once(']') else {
            return false;
        };
        if !matches!(suffix, "" | ":443") {
            return false;
        }
        return host.parse::<Ipv6Addr>().is_ok_and(is_public_ipv6);
    } else {
        let (host, port) = authority
            .rsplit_once(':')
            .map_or((authority, None), |(host, port)| (host, Some(port)));
        if port.is_some_and(|port| port != "443") {
            return false;
        }
        host
    };
    host.parse::<Ipv4Addr>()
        .map_or_else(|_| is_public_domain(host), is_public_ipv4)
}

fn validate_application_version_response(
    response: &ApplicationVersionResponse,
) -> Result<(), PublicServiceContractError> {
    let state_count = [
        response.forced_update,
        response.prompt_update,
        response.stop_update,
    ]
    .into_iter()
    .filter(|value| *value)
    .count();
    let invalid = state_count > 1
        || (!response.update && (response.forced_update || response.prompt_update))
        || (response.update && response.stop_update)
        || response.download_url.chars().count() > 500
        || !is_safe_public_https_url(&response.download_url)
        || !is_safe_plain_text(&response.prompt_content, 100)
        || response.update_content.chars().count() > 10_000
        || response
            .update_content
            .chars()
            .any(is_unsafe_display_character);
    if invalid {
        return Err(PublicServiceContractError::InvalidApplicationVersionResponse);
    }
    Ok(())
}

fn validate_app_id(value: &str) -> Result<(), PublicServiceContractError> {
    let valid = !value.is_empty()
        && value.len() <= 50
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (byte == b'-' && index > 0 && index + 1 < value.len())
        });
    valid
        .then_some(())
        .ok_or(PublicServiceContractError::InvalidAppId)
}

fn validate_semver(value: &str) -> Result<(), PublicServiceContractError> {
    let mut segments = value.split('.');
    let valid = (0..3).all(|_| {
        segments.next().is_some_and(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| byte.is_ascii_digit())
                && (segment == "0" || !segment.starts_with('0'))
        })
    }) && segments.next().is_none();
    valid
        .then_some(())
        .ok_or(PublicServiceContractError::InvalidSemver)
}

fn validate_uuid_v4(value: &str) -> Result<(), PublicServiceContractError> {
    let parsed = Uuid::parse_str(value).map_err(|_| PublicServiceContractError::InvalidUuidV4)?;
    (parsed.get_version_num() == 4)
        .then_some(())
        .ok_or(PublicServiceContractError::InvalidUuidV4)
}

fn validate_timestamp(value: &str) -> Result<(), PublicServiceContractError> {
    if value.len() > 35 || value.parse::<jiff::Timestamp>().is_err() {
        return Err(PublicServiceContractError::InvalidTimestamp);
    }
    Ok(())
}

fn is_safe_plain_text(value: &str, maximum: usize) -> bool {
    let length = value.chars().count();
    length > 0
        && length <= maximum
        && !value.contains(['<', '>'])
        && !value.chars().any(is_unsafe_display_character)
}

fn is_unsafe_display_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{00ad}'
                | '\u{061c}'
                | '\u{180e}'
                | '\u{200b}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
        )
}

fn is_public_domain(domain: &str) -> bool {
    let host = domain.trim_end_matches('.').to_ascii_lowercase();
    host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        && ![
            ".example",
            ".home.arpa",
            ".internal",
            ".invalid",
            ".local",
            ".localhost",
            ".onion",
            ".test",
        ]
        .into_iter()
        .any(|suffix| host == suffix[1..] || host.ends_with(suffix))
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [first, second, third, _fourth] = address.octets();
    !(first == 0
        || first == 10
        || first == 127
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 0)
        || (first == 192 && second == 88 && third == 99)
        || (first == 192 && second == 168)
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
        || first >= 224)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    address.to_ipv4_mapped().is_none_or(is_public_ipv4)
        && !(address.is_loopback()
            || address.is_unspecified()
            || address.is_multicast()
            || address.is_unique_local()
            || address.is_unicast_link_local()
            || (address.segments()[0] == 0x2001 && address.segments()[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALL_ID: &str = "2c9079ef-6e1c-4cc7-9bea-d11bcd9df75d";

    fn response() -> ApplicationVersionResponse {
        ApplicationVersionResponse {
            update: true,
            download_url: "https://downloads.example.com/assistant.exe".to_owned(),
            prompt_content: "发现新版本".to_owned(),
            forced_update: false,
            prompt_update: true,
            stop_update: false,
            update_content: "请前往官方下载页获取更新。".to_owned(),
        }
    }

    #[test]
    fn daily_active_payload_has_exact_contract() {
        let payload = DailyActivePayload::new(
            "bifang-content-production-assistant",
            "0.2.17",
            INSTALL_ID,
            PublicPlatform::Windows,
            PublicArchitecture::X64,
            "2026-09-29T00:00:00.000Z",
        )
        .expect("测试载荷应有效");
        let value = serde_json::to_value(payload).expect("载荷应可序列化");
        assert_eq!(value.as_object().expect("应为对象").len(), 10);
        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["event"], "daily_active");
        assert_eq!(value["sdk_version"], "3.0.0");
    }

    #[test]
    fn application_version_truth_table_is_stable() {
        assert!(matches!(
            decide_application_version(response()),
            Ok(ApplicationVersionDecision::Optional(_))
        ));
        let mut required = response();
        required.prompt_update = false;
        required.forced_update = true;
        assert!(matches!(
            decide_application_version(required),
            Ok(ApplicationVersionDecision::Required(_))
        ));
        let mut none = response();
        none.update = false;
        none.prompt_update = false;
        assert_eq!(
            decide_application_version(none),
            Ok(ApplicationVersionDecision::None)
        );
        let mut stopped = response();
        stopped.update = false;
        stopped.prompt_update = false;
        stopped.stop_update = true;
        assert!(matches!(
            decide_application_version(stopped),
            Ok(ApplicationVersionDecision::Stopped(_))
        ));
    }

    #[test]
    fn invalid_response_and_unsafe_links_fail_closed() {
        let mut conflicting = response();
        conflicting.forced_update = true;
        assert_eq!(
            decide_application_version(conflicting),
            Err(PublicServiceContractError::InvalidApplicationVersionResponse)
        );
        for value in [
            "http://downloads.example.com/a.exe",
            "https://user:secret@downloads.example.com/a.exe",
            "https://127.0.0.1/a.exe",
            "https://downloads.invalid/a.exe",
        ] {
            assert!(!is_safe_public_https_url(value));
        }
    }

    #[test]
    fn version_path_accepts_only_numeric_segments() {
        assert!(validate_application_version_number("0.2.17").is_ok());
        assert!(validate_application_version_number("0.2.17.1").is_ok());
        assert!(validate_application_version_number("v0.2.17").is_err());
        assert!(validate_application_version_number("0.2.17-beta").is_err());
    }
}
