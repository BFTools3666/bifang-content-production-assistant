//! 飞书 CLI JSON 协议解析与稳定错误映射。

use serde_json::Value;

use super::process::{OperationRisk, ProcessOutput};

/// 官方 CLI 成功创建或上传后的稳定远端文件身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::workbench::connectors) struct LarkCliUploadResult {
    pub file_token: String,
}

/// 不包含原始 CLI 输出、路径或 token 的稳定错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::workbench::connectors) struct LarkCliError {
    pub code: &'static str,
    pub message: &'static str,
}

impl LarkCliError {
    /// 建立仅含稳定错误码和安全展示文案的 CLI 错误。
    pub(in crate::workbench::connectors) fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
}

/// 解析完整且顶层为对象的 JSON 输出。

pub(in crate::workbench::connectors) fn parse_json_object(output: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(output)
        .ok()
        .filter(Value::is_object)
}

/// 从混合文本的末尾查找最后一个 JSON 对象。

fn parse_last_json_object(output: &[u8]) -> Option<Value> {
    std::str::from_utf8(output)
        .ok()?
        .lines()
        .rev()
        .find_map(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .filter(Value::is_object)
        })
}

/// 仅依据结构化字段把只读检查失败映射为稳定错误。

pub(in crate::workbench::connectors) fn map_inspection_process_failure(
    output: &ProcessOutput,
    fallback_code: &'static str,
    fallback_message: &'static str,
) -> LarkCliError {
    let envelope = parse_json_object(&output.stderr)
        .or_else(|| parse_last_json_object(&output.stderr))
        .or_else(|| parse_json_object(&output.stdout))
        .or_else(|| parse_last_json_object(&output.stdout));
    let Some(envelope) = envelope else {
        return LarkCliError::new(fallback_code, fallback_message);
    };
    let error = envelope.get("error").unwrap_or(&envelope);
    let structured = ["type", "subtype", "code"]
        .into_iter()
        .filter_map(|key| error.get(key))
        .filter_map(|value| match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let contains = |fragment: &str| structured.iter().any(|value| value.contains(fragment));
    let equals = |expected: &str| structured.iter().any(|value| value == expected);
    if contains("authentication")
        || contains("login")
        || contains("unauthorized")
        || contains("expired_token")
    {
        return LarkCliError::new(
            "CONNECTOR_CLI_AUTH_REQUIRED",
            "飞书 CLI profile 尚未完成用户登录或登录已失效",
        );
    }
    if contains("authorization")
        || contains("permission")
        || contains("scope")
        || contains("forbidden")
    {
        return LarkCliError::new("CONNECTOR_PERMISSION", "当前飞书 CLI 用户缺少所需权限");
    }
    if equals("not_configured")
        || equals("profile_not_configured")
        || equals("profile_not_found")
        || equals("profile-not-found")
        || (equals("profile") && equals("not_found"))
    {
        return LarkCliError::new(
            "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED",
            "飞书 CLI profile 尚未配置",
        );
    }
    if contains("timeout") {
        return LarkCliError::new("CONNECTOR_TIMEOUT", "飞书 CLI 调用超时");
    }
    if contains("network") || contains("transport") || contains("connection") {
        return LarkCliError::new(
            "CONNECTOR_NETWORK_FAILED",
            "暂时无法通过飞书 CLI 访问飞书服务",
        );
    }
    LarkCliError::new(fallback_code, fallback_message)
}

/// 从 CLI 版本文本中提取三段数字版本。

pub(in crate::workbench::connectors) fn parse_cli_version(
    output: &[u8],
) -> Option<(u64, u64, u64)> {
    std::str::from_utf8(output)
        .ok()?
        .split_whitespace()
        .find_map(|part| {
            let mut segments = part
                .trim_matches(|character: char| !character.is_ascii_digit() && character != '.')
                .split('.');
            let major = segments.next()?.parse().ok()?;
            let minor = segments.next()?.parse().ok()?;
            let patch = segments.next()?.parse().ok()?;
            segments.next().is_none().then_some((major, minor, patch))
        })
}

/// 解析普通 CLI 命令要求的单一 JSON 对象信封。

pub(in crate::workbench::connectors) fn parse_json_envelope(output: &[u8]) -> Option<Value> {
    parse_json_object(output)
}

/// 从成功信封提取非空远端文件 token。

pub(in crate::workbench::connectors) fn extract_file_token(
    envelope: &Value,
) -> Result<LarkCliUploadResult, LarkCliError> {
    envelope
        .pointer("/data/file_token")
        .and_then(Value::as_str)
        .filter(|token| !token.trim().is_empty())
        .map(|file_token| LarkCliUploadResult {
            file_token: file_token.to_string(),
        })
        .ok_or_else(|| {
            LarkCliError::new(
                "CONNECTOR_SYNC_OUTCOME_UNKNOWN",
                "飞书 CLI 报告成功但未返回文件 token；请先在飞书检查文件",
            )
        })
}

/// 依据结构化错误字段和操作风险映射发布错误。

pub(in crate::workbench::connectors) fn map_cli_error(
    envelope: &Value,
    risk: OperationRisk,
) -> LarkCliError {
    let error = envelope.get("error").unwrap_or(envelope);
    let structured = ["type", "subtype", "code"]
        .into_iter()
        .filter_map(|key| error.get(key))
        .map(|value| match value {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if risk == OperationRisk::Write
        && (structured.contains("internal")
            || structured.contains("invalid_response")
            || structured.contains("sdk_error"))
    {
        return LarkCliError::new(
            "CONNECTOR_SYNC_OUTCOME_UNKNOWN",
            "发布请求可能已经完成，但 CLI 无法确认结果；请先在飞书检查文件",
        );
    }
    if structured.contains("network")
        || structured.contains("transport")
        || structured.contains("protocol")
        || structured.contains("connection")
        || structured.contains("timeout")
    {
        return match risk {
            OperationRisk::Read if structured.contains("timeout") => {
                LarkCliError::new("CONNECTOR_TIMEOUT", "飞书 CLI 调用超时")
            }
            OperationRisk::Read => LarkCliError::new(
                "CONNECTOR_NETWORK_FAILED",
                "暂时无法通过飞书 CLI 访问飞书服务",
            ),
            OperationRisk::Write => LarkCliError::new(
                "CONNECTOR_SYNC_OUTCOME_UNKNOWN",
                "发布请求可能已经到达飞书，但结果无法确认；请先在飞书检查文件",
            ),
        };
    }
    if structured.contains("not_configured")
        || structured.contains("profile")
        || structured.contains("config")
    {
        return LarkCliError::new(
            "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED",
            "飞书 CLI profile 尚未配置",
        );
    }
    if structured.contains("authorization")
        || structured.contains("permission")
        || structured.contains("forbidden")
        || structured.contains("scope")
    {
        return LarkCliError::new(
            "CONNECTOR_PERMISSION",
            "当前飞书 CLI 用户无权访问目标文件夹",
        );
    }
    if structured.contains("authentication")
        || structured.contains("auth_required")
        || structured.contains("login")
        || structured.contains("token")
        || structured.contains("unauthorized")
    {
        return LarkCliError::new(
            "CONNECTOR_CLI_AUTH_REQUIRED",
            "飞书 CLI profile 尚未完成用户登录或登录已失效",
        );
    }
    if structured.contains("rate") || structured.contains("429") {
        return LarkCliError::new("CONNECTOR_RATE_LIMITED", "飞书接口繁忙，请稍后手动重试");
    }
    if structured.contains("quota") || structured.contains("storage") {
        return LarkCliError::new(
            "CONNECTOR_QUOTA_EXCEEDED",
            "飞书云空间配额不足，无法发布文件",
        );
    }
    if structured.contains("not_found") || structured.contains("folder") {
        return LarkCliError::new(
            "CONNECTOR_FOLDER_UNAVAILABLE",
            "目标飞书文件夹不存在或当前用户无权访问",
        );
    }
    match risk {
        OperationRisk::Read => {
            LarkCliError::new("CONNECTOR_REQUEST_REJECTED", "飞书 CLI 拒绝了当前只读请求")
        }
        OperationRisk::Write => {
            LarkCliError::new("CONNECTOR_UPLOAD_REJECTED", "飞书 CLI 未接受本次文件发布")
        }
    }
}

/// 为无法验证的响应生成与读写风险匹配的稳定错误。

pub(in crate::workbench::connectors) fn unverifiable_result(risk: OperationRisk) -> LarkCliError {
    match risk {
        OperationRisk::Read => LarkCliError::new(
            "CONNECTOR_CLI_INVALID_RESPONSE",
            "飞书 CLI 返回了无法验证的结果",
        ),
        OperationRisk::Write => LarkCliError::new(
            "CONNECTOR_SYNC_OUTCOME_UNKNOWN",
            "发布命令已经开始，但结果无法确认；请先在飞书检查文件",
        ),
    }
}
