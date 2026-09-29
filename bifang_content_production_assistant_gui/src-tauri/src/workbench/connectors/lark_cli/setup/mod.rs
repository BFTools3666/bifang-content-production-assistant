//! 只读检查官方 CLI 版本、已有 Profile 与用户身份。

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use tokio::time::Instant;

use super::{
    process::{LarkCliClient, OperationRisk},
    protocol::{
        LarkCliError, map_inspection_process_failure, parse_cli_version, parse_json_object,
    },
};

const MINIMUM_LARK_CLI_VERSION: (u64, u64, u64) = (1, 0, 96);

/// 保存可向界面展示且不含应用凭据的 Profile 摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::workbench::connectors) struct LarkCliProfileSummary {
    pub name: String,
    pub user_name: Option<String>,
    pub token_status: Option<String>,
}

/// 汇总 CLI 版本、Profile 列表及所选用户授权状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::workbench::connectors) struct LarkCliSetupSnapshot {
    pub version: String,
    pub profiles: Vec<LarkCliProfileSummary>,
    pub selected_profile_configured: bool,
    pub selected_profile_authorized: bool,
    pub selected_user_name: Option<String>,
    pub authorization_issue_code: Option<&'static str>,
}

/// 仅反序列化官方 Profile 列表中允许使用的安全字段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliProfileListItem {
    name: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    token_status: Option<String>,
}

impl LarkCliClient {
    /// 只读检查 CLI、Profile 列表和所选 Profile 的真实用户授权。
    pub(in crate::workbench::connectors) async fn inspect_setup(
        &self,
        selected_profile: &str,
        timeout_ms: u64,
    ) -> Result<LarkCliSetupSnapshot, LarkCliError> {
        if !selected_profile.is_empty() {
            validate_profile_name(selected_profile)?;
        }
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let version = self
            .ensure_supported_version(remaining_timeout_ms(deadline)?)
            .await?;
        let profiles = self.list_profiles(remaining_timeout_ms(deadline)?).await?;
        let configured = !selected_profile.is_empty()
            && profiles
                .iter()
                .any(|profile| profile.name == selected_profile);
        if !configured {
            return Ok(LarkCliSetupSnapshot {
                version,
                profiles,
                selected_profile_configured: false,
                selected_profile_authorized: false,
                selected_user_name: None,
                authorization_issue_code: (!selected_profile.is_empty())
                    .then_some("CONNECTOR_CLI_PROFILE_NOT_CONFIGURED"),
            });
        }
        let authorization = self
            .inspect_user_authorization(selected_profile, remaining_timeout_ms(deadline)?)
            .await;
        let (authorized, user_name, issue) = match authorization {
            Ok(result) => result,
            Err(error) => (false, None, Some(error.code)),
        };
        Ok(LarkCliSetupSnapshot {
            version,
            profiles,
            selected_profile_configured: true,
            selected_profile_authorized: authorized,
            selected_user_name: user_name,
            authorization_issue_code: issue,
        })
    }

    /// 读取 Profile 列表并仅保留可安全展示的字段。
    async fn list_profiles(
        &self,
        timeout_ms: u64,
    ) -> Result<Vec<LarkCliProfileSummary>, LarkCliError> {
        let output = self
            .run_process(
                &["profile", "list"],
                None,
                None,
                timeout_ms,
                OperationRisk::Read,
            )
            .await?;
        if !output.status.success() {
            return Err(map_inspection_process_failure(
                &output,
                "CONNECTOR_CLI_PROFILE_LIST_FAILED",
                "无法读取飞书 CLI profile 列表",
            ));
        }
        let raw =
            serde_json::from_slice::<Vec<CliProfileListItem>>(&output.stdout).map_err(|_| {
                LarkCliError::new(
                    "CONNECTOR_CLI_INVALID_RESPONSE",
                    "飞书 CLI 返回了无法验证的 profile 列表",
                )
            })?;
        Ok(raw
            .into_iter()
            .filter(|item| validate_profile_name(&item.name).is_ok())
            .map(|item| LarkCliProfileSummary {
                name: item.name,
                user_name: item.user.as_deref().and_then(safe_user_name),
                token_status: item
                    .token_status
                    .filter(|value| is_safe_token_status(value)),
            })
            .collect())
    }

    /// 只读验证所选 Profile 的用户令牌可用性和服务端校验状态。
    async fn inspect_user_authorization(
        &self,
        profile: &str,
        timeout_ms: u64,
    ) -> Result<(bool, Option<String>, Option<&'static str>), LarkCliError> {
        let arguments = ["--profile", profile, "auth", "status", "--json", "--verify"];
        let output = self
            .run_process(&arguments, None, None, timeout_ms, OperationRisk::Read)
            .await?;
        if !output.status.success() {
            let error = map_inspection_process_failure(
                &output,
                "CONNECTOR_CLI_AUTH_STATUS_FAILED",
                "无法验证飞书 CLI 用户授权",
            );
            return Ok((false, None, Some(error.code)));
        }
        let envelope = parse_json_object(&output.stdout).ok_or_else(invalid_auth_status)?;
        let user = envelope
            .pointer("/identities/user")
            .ok_or_else(invalid_auth_status)?;
        let authorized = user.get("available").and_then(Value::as_bool) == Some(true)
            && user.get("verified").and_then(Value::as_bool) == Some(true);
        let user_name = user
            .get("userName")
            .and_then(Value::as_str)
            .and_then(safe_user_name);
        let issue = (!authorized).then_some(match user.get("status").and_then(Value::as_str) {
            Some("verify_failed" | "error") => "CONNECTOR_CLI_AUTH_VERIFY_FAILED",
            _ => "CONNECTOR_CLI_AUTH_REQUIRED",
        });
        Ok((authorized, user_name, issue))
    }

    /// 执行只读版本检查并拒绝未知格式或下一主版本。
    pub(in crate::workbench::connectors) async fn ensure_supported_version(
        &self,
        timeout_ms: u64,
    ) -> Result<String, LarkCliError> {
        let output = self
            .run_process(&["--version"], None, None, timeout_ms, OperationRisk::Read)
            .await?;
        if !output.status.success() || !output.input_written {
            return Err(LarkCliError::new(
                "CONNECTOR_CLI_EXECUTION_FAILED",
                "无法读取飞书 CLI 版本",
            ));
        }
        let version = parse_cli_version(&output.stdout).ok_or_else(|| {
            LarkCliError::new(
                "CONNECTOR_CLI_INVALID_RESPONSE",
                "飞书 CLI 返回了无法识别的版本",
            )
        })?;
        if version < MINIMUM_LARK_CLI_VERSION || version.0 >= 2 {
            return Err(LarkCliError::new(
                "CONNECTOR_CLI_VERSION_UNSUPPORTED",
                "飞书 CLI 版本不受支持，请使用 1.0.96 或更新的 1.x 版本",
            ));
        }
        Ok(format!("{}.{}.{}", version.0, version.1, version.2))
    }

    /// 只读确认 Profile 提供可用用户身份。
    pub(in crate::workbench::connectors) async fn ensure_user_profile(
        &self,
        profile: &str,
        timeout_ms: u64,
    ) -> Result<(), LarkCliError> {
        let arguments = ["--profile", profile, "whoami", "--as", "user"];
        let output = self
            .run_process(&arguments, None, None, timeout_ms, OperationRisk::Read)
            .await?;
        if !output.status.success() {
            return Err(map_inspection_process_failure(
                &output,
                "CONNECTOR_CLI_AUTH_REQUIRED",
                "飞书 CLI profile 尚未完成用户登录或登录已失效",
            ));
        }
        let envelope = parse_json_object(&output.stdout).ok_or_else(|| {
            LarkCliError::new(
                "CONNECTOR_CLI_INVALID_RESPONSE",
                "飞书 CLI 返回了无法验证的用户身份",
            )
        })?;
        let is_user = envelope
            .get("identity")
            .or_else(|| envelope.pointer("/data/identity"))
            .and_then(Value::as_str)
            == Some("user");
        let available = envelope.get("available").and_then(Value::as_bool) == Some(true)
            || envelope.get("ok").and_then(Value::as_bool) == Some(true);
        if !is_user || !available {
            return Err(LarkCliError::new(
                "CONNECTOR_CLI_IDENTITY_INVALID",
                "飞书 CLI profile 未提供可用的用户身份",
            ));
        }
        Ok(())
    }
}

/// 建立授权状态响应无法验证时的稳定错误。
fn invalid_auth_status() -> LarkCliError {
    LarkCliError::new(
        "CONNECTOR_CLI_INVALID_RESPONSE",
        "飞书 CLI 返回了无法验证的授权状态",
    )
}

/// 验证 Profile 名称并保留用户输入的精确值。
pub(in crate::workbench::connectors) fn validate_profile_name(
    profile: &str,
) -> Result<(), LarkCliError> {
    let valid = !profile.is_empty()
        && profile.chars().count() <= 64
        && profile.trim() == profile
        && !profile.chars().any(|character| {
            character.is_whitespace()
                || character.is_control()
                || matches!(
                    character,
                    '/' | '\\'
                        | ';'
                        | '&'
                        | '|'
                        | '$'
                        | '`'
                        | '"'
                        | '\''
                        | '<'
                        | '>'
                        | '('
                        | ')'
                        | '{'
                        | '}'
                        | '['
                        | ']'
                        | '*'
                        | '?'
                        | '!'
                        | '#'
                        | '~'
                )
        });
    if valid {
        Ok(())
    } else {
        Err(LarkCliError::new(
            "CONNECTOR_CLI_PROFILE_INVALID",
            "飞书 CLI profile 名称不安全或格式无效",
        ))
    }
}

/// 限制界面可见令牌状态为官方已知枚举。
fn is_safe_token_status(value: &str) -> bool {
    matches!(value, "valid" | "needs_refresh" | "expired" | "error")
}

/// 清理并限制界面可见用户名的长度和控制字符。
fn safe_user_name(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= 128 && !value.chars().any(char::is_control))
        .then(|| value.to_string())
}

/// 计算共享截止时间的剩余毫秒并拒绝耗尽的预算。
pub(in crate::workbench::connectors) fn remaining_timeout_ms(
    deadline: Instant,
) -> Result<u64, LarkCliError> {
    let millis = u64::try_from(
        deadline
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    if millis == 0 {
        Err(LarkCliError::new("CONNECTOR_TIMEOUT", "飞书 CLI 调用超时"))
    } else {
        Ok(millis)
    }
}
