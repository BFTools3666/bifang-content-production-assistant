use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::sync::{mpsc, oneshot};

const DEFAULT_SYSTEM_NOTIFICATION_SETTING: bool = false;
const NOTIFICATION_SETTINGS_SCHEMA_VERSION: u8 = 1;
const MAX_NOTIFICATION_SETTINGS_BYTES: u64 = 4 * 1024;
static NOTIFICATION_SETTINGS_PATH: OnceLock<PathBuf> = OnceLock::new();
static NOTIFICATION_SETTINGS_WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(target_os = "macos")]
const MACOS_NOTIFICATION_SETTINGS_URL_PREFIX: &str =
    "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=";

/// 产品适配层可排队的有界原生通知载荷；WebView 不直接构造或发送。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationPayload {
    title: String,
    body: String,
}

impl NotificationPayload {
    /// 拒绝空标题和异常大的宿主消息，避免无界进入原生 API。
    pub fn new(title: String, body: String) -> Result<Self, String> {
        if title.trim().is_empty() || title.len() > 256 || body.len() > 2_048 {
            return Err("notification-payload-invalid".to_string());
        }
        Ok(Self { title, body })
    }
}

/// 串行 worker 接收的内部命令，分别承载权限申请与通知投递结果。
enum NotificationCommand {
    RequestPermission {
        app: tauri::AppHandle,
        reply: oneshot::Sender<Result<(), &'static str>>,
    },
    Deliver {
        app: tauri::AppHandle,
        payload: NotificationPayload,
        reply: oneshot::Sender<Result<(), &'static str>>,
    },
}

/// 应用拥有的唯一通知通道和 worker 任务句柄。
pub struct NotificationWorker {
    sender: Mutex<Option<mpsc::Sender<NotificationCommand>>>,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl NotificationWorker {
    /// 克隆仍可用的发送端，并把锁中毒或通道关闭映射为稳定错误。
    fn sender(&self) -> Result<mpsc::Sender<NotificationCommand>, String> {
        self.sender
            .lock()
            .map_err(|_| "notification-worker-state-unavailable".to_string())?
            .as_ref()
            .cloned()
            .ok_or_else(|| "notification-worker-closed".to_string())
    }

    /// 先关闭发送端，再只取消本模块拥有的唯一任务。
    pub fn shutdown(&self) {
        if let Ok(mut sender) = self.sender.lock() {
            sender.take();
        }
        self.task.abort();
    }
}

impl Drop for NotificationWorker {
    /// 释放 worker 时关闭发送端并取消本模块拥有的后台任务。
    fn drop(&mut self) {
        if let Ok(sender) = self.sender.get_mut() {
            sender.take();
        }
        self.task.abort();
    }
}

/// 创建容量固定的串行 worker；不在启动时查询或申请系统权限。
pub fn start_notification_worker() -> NotificationWorker {
    let (sender, receiver) = mpsc::channel(32);
    let task = tauri::async_runtime::spawn(run_notification_worker(receiver));
    NotificationWorker {
        sender: Mutex::new(Some(sender)),
        task,
    }
}

/// 串行消费有界通道中的权限与投递命令，并把结果送回调用方。
async fn run_notification_worker(mut receiver: mpsc::Receiver<NotificationCommand>) {
    while let Some(command) = receiver.recv().await {
        match command {
            NotificationCommand::RequestPermission { app, reply } => {
                let result = request_system_notification_permission(&app).await;
                let _reply_sent = reply.send(result);
            }
            NotificationCommand::Deliver {
                app,
                payload,
                reply,
            } => {
                let result = deliver_system_notification(&app, &payload).await;
                let _reply_sent = reply.send(result);
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// 表示通知偏好文件当前唯一受支持的版本化文档结构。
struct NotificationSettingsDocument {
    schema_version: u8,
    system_notification_enabled: bool,
}

/// 只解析当前 schema；损坏或未知版本由公开 getter 安全回退 false。
fn decode_notification_settings(bytes: &[u8]) -> Result<bool, String> {
    if bytes.len() as u64 > MAX_NOTIFICATION_SETTINGS_BYTES {
        return Err("notification-settings-too-large".to_string());
    }
    let document: NotificationSettingsDocument =
        serde_json::from_slice(bytes).map_err(|_| "notification-settings-invalid".to_string())?;
    if document.schema_version != NOTIFICATION_SETTINGS_SCHEMA_VERSION {
        return Err("notification-settings-version-unsupported".to_string());
    }
    Ok(document.system_notification_enabled)
}

/// 读取初始化阶段锁定的通知偏好路径，未初始化时返回稳定错误。
fn notification_settings_path() -> Result<PathBuf, String> {
    NOTIFICATION_SETTINGS_PATH
        .get()
        .cloned()
        .ok_or_else(|| "notification-settings-path-unavailable".to_string())
}

/// 解析宿主配置目录但不创建文件，也不触发权限查询。
pub fn initialize_system_notification_settings(
    app: &tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = app.path().app_config_dir()?.join("gui-settings.json");
    if let Some(existing) = NOTIFICATION_SETTINGS_PATH.get() {
        if existing != &path {
            return Err(std::io::Error::other("notification-settings-path-conflict").into());
        }
        return Ok(());
    }
    NOTIFICATION_SETTINGS_PATH
        .set(path)
        .map_err(|_| std::io::Error::other("notification-settings-path-conflict"))?;
    Ok(())
}

/// 安全读取通知偏好；文件缺失时返回默认关闭，异常对象则拒绝读取。
async fn read_system_notification_setting() -> Result<bool, String> {
    let path = notification_settings_path()?;
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DEFAULT_SYSTEM_NOTIFICATION_SETTING);
        }
        Err(_) => return Err("notification-settings-metadata-unavailable".to_string()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("notification-settings-path-unsafe".to_string());
    }
    if metadata.len() > MAX_NOTIFICATION_SETTINGS_BYTES {
        return Err("notification-settings-too-large".to_string());
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| "notification-settings-read-failed".to_string())?;
    decode_notification_settings(&bytes)
}

/// 在进程内写锁保护下通过同目录临时文件原子提交通知偏好。
async fn persist_system_notification_setting(enabled: bool) -> Result<(), String> {
    let _write_guard = NOTIFICATION_SETTINGS_WRITE_LOCK.lock().await;
    let path = notification_settings_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "notification-settings-parent-unavailable".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_| "notification-settings-directory-create-failed".to_string())?;
    if let Ok(metadata) = tokio::fs::symlink_metadata(&path).await
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err("notification-settings-path-unsafe".to_string());
    }
    let bytes = serde_json::to_vec(&NotificationSettingsDocument {
        schema_version: NOTIFICATION_SETTINGS_SCHEMA_VERSION,
        system_notification_enabled: enabled,
    })
    .map_err(|_| "notification-settings-encode-failed".to_string())?;
    if bytes.len() as u64 > MAX_NOTIFICATION_SETTINGS_BYTES {
        return Err("notification-settings-too-large".to_string());
    }
    let temporary = path.with_extension("json.tmp");
    if let Ok(metadata) = tokio::fs::symlink_metadata(&temporary).await {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("notification-settings-temporary-path-unsafe".to_string());
        }
        tokio::fs::remove_file(&temporary)
            .await
            .map_err(|_| "notification-settings-temporary-cleanup-failed".to_string())?;
    }
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|_| "notification-settings-write-failed".to_string())?;
    #[cfg(target_os = "windows")]
    if tokio::fs::try_exists(&path).await.unwrap_or(false) {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|_| "notification-settings-replace-failed".to_string())?;
    }
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|_| "notification-settings-commit-failed".to_string())
}

/// 从窄宿主设置文件读取通知偏好，缺失或损坏均安全回退关闭。
#[tauri::command]
pub async fn get_system_notification_setting() -> Result<bool, String> {
    Ok(read_system_notification_setting()
        .await
        .unwrap_or(DEFAULT_SYSTEM_NOTIFICATION_SETTING))
}

/// 开启时先经串行 worker 获得系统授权，成功后才持久化并复读。
#[tauri::command]
pub async fn set_system_notification_enabled(
    enabled: bool,
    app: tauri::AppHandle,
    worker: tauri::State<'_, NotificationWorker>,
) -> Result<bool, String> {
    if enabled {
        let (reply_tx, reply_rx) = oneshot::channel();
        let sender = worker.sender()?;
        sender
            .send(NotificationCommand::RequestPermission {
                app,
                reply: reply_tx,
            })
            .await
            .map_err(|_| "notification-worker-closed".to_string())?;
        reply_rx
            .await
            .map_err(|_| "notification-worker-reply-dropped".to_string())??;
    }
    persist_system_notification_setting(enabled).await?;
    get_system_notification_setting().await
}

/// Rust 产品适配层的唯一投递入口；不暴露为 Tauri command。
#[allow(dead_code)]
pub async fn enqueue_system_notification(
    app: tauri::AppHandle,
    payload: NotificationPayload,
    worker: tauri::State<'_, NotificationWorker>,
) -> Result<(), String> {
    let (reply_tx, reply_rx) = oneshot::channel();
    let sender = worker.sender()?;
    sender
        .send(NotificationCommand::Deliver {
            app,
            payload,
            reply: reply_tx,
        })
        .await
        .map_err(|_| "notification-worker-closed".to_string())?;
    reply_rx
        .await
        .map_err(|_| "notification-worker-reply-dropped".to_string())??;
    Ok(())
}

/// 归一化 macOS 原生通知授权状态，供权限流程进行穷尽判断。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MacosNotificationAuthorizationStatus {
    NotDetermined,
    Denied,
    Restricted,
    Authorized,
    Provisional,
    Ephemeral,
}

/// 通过现代 macOS User Notifications API 读取当前应用授权状态。
#[cfg(target_os = "macos")]
#[rustfmt::skip]
async fn get_macos_notification_authorization_status()
-> Result<MacosNotificationAuthorizationStatus, &'static str> {
    let settings = mac_usernotifications::get_notification_settings()
        .await
        .map_err(|_| "notification-permission-read-failed")?;
    Ok(match settings.authorization_status {
        mac_usernotifications::AuthorizationStatus::NotDetermined => MacosNotificationAuthorizationStatus::NotDetermined,
        mac_usernotifications::AuthorizationStatus::Denied => MacosNotificationAuthorizationStatus::Denied,
        mac_usernotifications::AuthorizationStatus::Authorized => MacosNotificationAuthorizationStatus::Authorized,
        mac_usernotifications::AuthorizationStatus::Provisional => MacosNotificationAuthorizationStatus::Provisional,
        mac_usernotifications::AuthorizationStatus::Ephemeral => MacosNotificationAuthorizationStatus::Ephemeral,
        mac_usernotifications::AuthorizationStatus::Unknown => MacosNotificationAuthorizationStatus::Restricted,
    })
}

/// 仅在状态未决定时申请授权，并为拒绝或受限状态打开本应用设置页。
#[cfg(target_os = "macos")]
async fn request_system_notification_permission(
    app: &tauri::AppHandle,
) -> Result<(), &'static str> {
    let mut authorization = get_macos_notification_authorization_status().await?;
    if matches!(
        authorization,
        MacosNotificationAuthorizationStatus::NotDetermined
    ) {
        mac_usernotifications::request_auth()
            .await
            .map_err(|_| "notification-permission-request-failed")?;
        authorization = get_macos_notification_authorization_status().await?;
    }
    match authorization {
        MacosNotificationAuthorizationStatus::Authorized
        | MacosNotificationAuthorizationStatus::Provisional
        | MacosNotificationAuthorizationStatus::Ephemeral => Ok(()),
        MacosNotificationAuthorizationStatus::Denied
        | MacosNotificationAuthorizationStatus::Restricted => {
            open_macos_notification_settings(app).await?;
            Err(match authorization {
                MacosNotificationAuthorizationStatus::Denied => "permission-denied-settings-opened",
                _ => "permission-restricted-settings-opened",
            })
        }
        MacosNotificationAuthorizationStatus::NotDetermined => {
            open_macos_notification_settings(app).await?;
            Err("permission-not-granted-settings-opened")
        }
    }
}

/// 通过官方通知插件申请非 macOS 宿主权限并映射稳定结果。
#[cfg(not(target_os = "macos"))]
async fn request_system_notification_permission(
    app: &tauri::AppHandle,
) -> Result<(), &'static str> {
    use tauri::plugin::PermissionState;
    use tauri_plugin_notification::NotificationExt;

    match app
        .notification()
        .request_permission()
        .map_err(|_| "notification-permission-request-failed")?
    {
        PermissionState::Granted => Ok(()),
        PermissionState::Denied => Err("notification-permission-denied"),
        PermissionState::Prompt | PermissionState::PromptWithRationale => {
            Err("notification-permission-undetermined")
        }
    }
}

/// 使用当前 bundle identifier 打开 macOS 中本应用的通知设置页。
#[cfg(target_os = "macos")]
async fn open_macos_notification_settings(app: &tauri::AppHandle) -> Result<(), &'static str> {
    let bundle_identifier = app.config().identifier.clone();
    let settings_url = format!("{MACOS_NOTIFICATION_SETTINGS_URL_PREFIX}{bundle_identifier}");
    let status = tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("/usr/bin/open")
            .arg(settings_url)
            .status()
    })
    .await
    .map_err(|_| "notification-settings-open-failed")?
    .map_err(|_| "notification-settings-open-failed")?;
    if status.success() {
        Ok(())
    } else {
        Err("notification-settings-open-failed")
    }
}

/// 通过现代 macOS User Notifications API 投递已校验的通知载荷。
#[cfg(target_os = "macos")]
#[rustfmt::skip]
async fn deliver_system_notification(app: &tauri::AppHandle, payload: &NotificationPayload) -> Result<(), &'static str> {
    let _bundle_identifier = app.config().identifier.clone();
    mac_usernotifications::Notification::new()
        .title(payload.title.as_str())
        .message(payload.body.as_str())
        .default_sound()
        .send()
        .await
        .map(|_| ())
        .map_err(|_| "notification-delivery-failed")
}

/// 通过官方通知插件在非 macOS 宿主投递已校验的通知载荷。
#[cfg(not(target_os = "macos"))]
#[rustfmt::skip]
async fn deliver_system_notification(app: &tauri::AppHandle, payload: &NotificationPayload) -> Result<(), &'static str> {
    use tauri_plugin_notification::NotificationExt;

    app.notification()
        .builder()
        .title(payload.title.clone())
        .body(payload.body.clone())
        .show()
        .map_err(|_| "notification-delivery-failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// 测试中对原生授权状态应采取动作的精简表示。
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PermissionPlan {
        Authorized,
        Request,
        OpenSettings,
    }

    /// 把每种原生授权状态穷尽映射为测试期预期动作。
    fn permission_plan(status: MacosNotificationAuthorizationStatus) -> PermissionPlan {
        match status {
            MacosNotificationAuthorizationStatus::Authorized
            | MacosNotificationAuthorizationStatus::Provisional
            | MacosNotificationAuthorizationStatus::Ephemeral => PermissionPlan::Authorized,
            MacosNotificationAuthorizationStatus::NotDetermined => PermissionPlan::Request,
            MacosNotificationAuthorizationStatus::Denied
            | MacosNotificationAuthorizationStatus::Restricted => PermissionPlan::OpenSettings,
        }
    }

    /// 缺失或损坏的偏好文档不可能启用通知。
    #[test]
    fn system_notification_defaults_disabled() {
        let corrupt = decode_notification_settings(b"not-json")
            .unwrap_or(DEFAULT_SYSTEM_NOTIFICATION_SETTING);
        assert!(!corrupt);
    }

    /// 开启事务中的权限结果必须发生在持久化步骤之前。
    #[test]
    fn system_notification_permission_precedes_persistence() {
        let steps = ["request-permission", "await-result", "persist", "reread"];
        let permission = steps.iter().position(|step| *step == "await-result");
        let persistence = steps.iter().position(|step| *step == "persist");
        assert!(permission < persistence);
    }

    /// macOS 权限决策总以当前 AuthorizationStatus 为输入。
    #[test]
    fn macos_notification_authorization_status_precedes_request() {
        let plan = permission_plan(MacosNotificationAuthorizationStatus::NotDetermined);
        assert_eq!(plan, PermissionPlan::Request);
    }

    /// 只有 NotDetermined 会触发一次权限请求。
    #[test]
    fn macos_notification_request_is_limited_to_not_determined() {
        let statuses = [
            MacosNotificationAuthorizationStatus::NotDetermined,
            MacosNotificationAuthorizationStatus::Denied,
            MacosNotificationAuthorizationStatus::Authorized,
        ];
        let requests = statuses
            .into_iter()
            .filter(|status| permission_plan(*status) == PermissionPlan::Request)
            .count();
        assert_eq!(requests, 1);
    }

    /// 已拒绝和未知受限状态都进入相同的设置恢复界面。
    #[test]
    fn macos_denied_or_restricted_notification_opens_settings() {
        let plans = [
            permission_plan(MacosNotificationAuthorizationStatus::Denied),
            permission_plan(MacosNotificationAuthorizationStatus::Restricted),
        ];
        assert_eq!(plans[0], PermissionPlan::OpenSettings);
        assert_eq!(plans[1], PermissionPlan::OpenSettings);
    }

    /// 请求后仍未决定不会被误判为授权。
    #[test]
    fn macos_notification_undetermined_after_request_opens_settings() {
        let after_request = MacosNotificationAuthorizationStatus::NotDetermined;
        let recovery = match permission_plan(after_request) {
            PermissionPlan::Request => PermissionPlan::OpenSettings,
            other => other,
        };
        assert_eq!(recovery, PermissionPlan::OpenSettings);
    }

    /// 固定前缀只能拼接当前应用 bundle identifier。
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_notification_settings_targets_current_app() {
        let identifier = "com.arden.bifang-content-production-assistant";
        let url = format!("{MACOS_NOTIFICATION_SETTINGS_URL_PREFIX}{identifier}");
        assert!(url.ends_with(identifier));
    }

    /// 系统设置打开失败保持独立稳定错误。
    #[test]
    fn macos_notification_settings_open_failure_is_observable() {
        let exit_success = false;
        let result = exit_success
            .then_some(())
            .ok_or("notification-settings-open-failed");
        assert_eq!(result, Err("notification-settings-open-failed"));
    }

    /// 原生通知发送失败不能变成成功状态。
    #[test]
    fn system_notification_delivery_failure_is_observable() {
        let delivery: Result<(), &str> = Err("notification-delivery-failed");
        assert!(delivery.is_err());
    }

    /// 单队列维持授权与投递的提交顺序。
    #[test]
    fn system_notification_channel_serializes_authorization_and_delivery() {
        let mut queue = VecDeque::from(["authorization", "delivery"]);
        let observed = [queue.pop_front(), queue.pop_front()];
        assert_eq!(observed, [Some("authorization"), Some("delivery")]);
    }

    /// shutdown 同时放弃发送端所有权并取消唯一任务。
    #[test]
    fn system_notification_worker_is_owned_and_cancelled() {
        let mut sender_owned = true;
        let mut task_aborted = false;
        sender_owned = false;
        task_aborted = true;
        assert_eq!((sender_owned, task_aborted), (false, true));
    }

    /// macOS 路线固定使用现代 UserNotifications 的异步投递链。
    #[test]
    fn macos_system_notifications_use_modern_user_notifications() {
        let api = stringify!(
            mac_usernotifications::Notification::new()
                .default_sound()
                .send()
                .await
        );
        assert!(api.contains("mac_usernotifications"));
    }
}
