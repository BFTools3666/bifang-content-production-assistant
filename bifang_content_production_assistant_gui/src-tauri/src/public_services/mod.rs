//! 当前应用主动调用的匿名日活与 application-version API v1 适配器。

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use bifang_content_production_assistant_core::public_services::{
    ApplicationVersionDecision, ApplicationVersionResponse, DailyActivePayload, PublicArchitecture,
    PublicPlatform, decide_application_version, is_safe_public_https_url,
};
use serde::Serialize;
use tauri::Manager;
use tokio::sync::{Mutex as AsyncMutex, RwLock, watch};

mod config;
mod state;
#[cfg(test)]
mod tests;
mod transport;

use state::StateStore;
use transport::{PublicServicesTransport, ReqwestPublicServicesTransport};

#[cfg(not(bifang_xhs))]
pub(crate) const APP_ID: &str = "bifang-content-production-assistant";
#[cfg(bifang_xhs)]
pub(crate) const APP_ID: &str = "bifang-xiaohongshu-assistant";
pub(crate) const STATE_SCHEMA_VERSION: u8 = 1;
pub(crate) const MAX_TELEMETRY_BYTES: usize = 1_024;
pub(crate) const MAX_UPDATE_BYTES: usize = 65_536;

/// 关于页与根级强更门消费的稳定更新展示对象。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePresentation {
    status: String,
    current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    download_ready: bool,
}

impl UpdatePresentation {
    fn status(status: &str) -> Self {
        Self {
            status: status.to_owned(),
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            title: None,
            message: None,
            download_ready: false,
        }
    }

    fn from_response(
        status: &str,
        response: &ApplicationVersionResponse,
        download_ready: bool,
    ) -> Self {
        Self {
            status: status.to_owned(),
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            title: Some(response.prompt_content.clone()),
            message: (!response.update_content.is_empty()).then(|| response.update_content.clone()),
            download_ready,
        }
    }
}

/// 公共服务失败的稳定、脱敏类别。
#[derive(Clone, Copy, Debug)]
pub enum PublicServicesError {
    StorageUnavailable,
    InvalidState,
    NetworkUnavailable,
    InvalidResponse,
    UpdateNotConfigured,
    ShuttingDown,
    DownloadUnavailable,
}

impl std::fmt::Display for PublicServicesError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::StorageUnavailable => "public_services_storage_unavailable",
            Self::InvalidState => "public_services_state_invalid",
            Self::NetworkUnavailable => "public_services_network_unavailable",
            Self::InvalidResponse => "public_services_response_invalid",
            Self::UpdateNotConfigured => "public_services_update_not_configured",
            Self::ShuttingDown => "public_services_shutting_down",
            Self::DownloadUnavailable => "public_services_download_unavailable",
        })
    }
}

impl std::error::Error for PublicServicesError {}

impl Serialize for PublicServicesError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clone)]
struct ClockSnapshot {
    occurred_at: String,
    activity_day: String,
}

struct RuntimeInner {
    store: StateStore,
    transport: Arc<dyn PublicServicesTransport>,
    clock: Arc<dyn Fn() -> ClockSnapshot + Send + Sync>,
    update: RwLock<UpdatePresentation>,
    download_url: RwLock<Option<String>>,
    update_gate: AsyncMutex<()>,
    tasks: Mutex<Vec<tauri::async_runtime::JoinHandle<()>>>,
    shutdown_tx: watch::Sender<u64>,
    shutting_down: AtomicBool,
    startup_telemetry_started: AtomicBool,
    startup_update_checked: AtomicBool,
}

/// 由应用生命周期拥有、可供窄命令共享的公共服务运行时。
#[derive(Clone)]
pub struct PublicServicesRuntime {
    inner: Arc<RuntimeInner>,
}

impl PublicServicesRuntime {
    /// 从应用专属 AppConfig 和生产 HTTP transport 构造运行时。
    pub fn new(app_config_root: PathBuf) -> Self {
        Self::with_components(
            app_config_root,
            ReqwestPublicServicesTransport::create(),
            Arc::new(current_clock_snapshot),
        )
    }

    fn with_components(
        app_config_root: PathBuf,
        transport: Arc<dyn PublicServicesTransport>,
        clock: Arc<dyn Fn() -> ClockSnapshot + Send + Sync>,
    ) -> Self {
        let (shutdown_tx, _) = watch::channel(0_u64);
        let initial_update = if transport.updates_configured() {
            UpdatePresentation::status("idle")
        } else {
            UpdatePresentation::status("not-configured")
        };
        Self {
            inner: Arc::new(RuntimeInner {
                store: StateStore::new(app_config_root),
                transport,
                clock,
                update: RwLock::new(initial_update),
                download_url: RwLock::new(None),
                update_gate: AsyncMutex::new(()),
                tasks: Mutex::new(Vec::new()),
                shutdown_tx,
                shutting_down: AtomicBool::new(false),
                startup_telemetry_started: AtomicBool::new(false),
                startup_update_checked: AtomicBool::new(false),
            }),
        }
    }

    async fn start_after_window_ready(&self) -> UpdatePresentation {
        if !self
            .inner
            .startup_telemetry_started
            .swap(true, Ordering::AcqRel)
        {
            let _ = self.queue_daily_active().await;
        }
        self.check_updates_on_startup().await
    }

    async fn queue_daily_active(&self) -> Result<(), PublicServicesError> {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err(PublicServicesError::ShuttingDown);
        }
        if !self.inner.transport.telemetry_configured() {
            return Ok(());
        }
        let clock = (self.inner.clock)();
        let Some(state) = self
            .inner
            .store
            .claim_daily_active(&clock.activity_day)
            .await?
        else {
            return Ok(());
        };
        let payload = DailyActivePayload::new(
            APP_ID,
            env!("CARGO_PKG_VERSION"),
            &state.install_id,
            current_platform(),
            current_architecture(),
            &clock.occurred_at,
        )
        .map_err(|_| PublicServicesError::InvalidState)?;
        let runtime = self.clone();
        let mut shutdown = self.inner.shutdown_tx.subscribe();
        let task = tauri::async_runtime::spawn(async move {
            let _ = tokio::select! {
                _ = shutdown.changed() => false,
                result = runtime.inner.transport.send_telemetry(payload) => result,
            };
        });
        if let Ok(mut owner) = self.inner.tasks.lock() {
            owner.push(task);
        } else {
            task.abort();
        }
        Ok(())
    }

    async fn check_updates(&self) -> UpdatePresentation {
        let _gate = self.inner.update_gate.lock().await;
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return self.record_update_failure().await;
        }
        if !self.inner.transport.updates_configured() {
            *self.inner.download_url.write().await = None;
            let status = UpdatePresentation::status("not-configured");
            *self.inner.update.write().await = status.clone();
            return status;
        }
        *self.inner.update.write().await = UpdatePresentation::status("checking");
        let mut shutdown = self.inner.shutdown_tx.subscribe();
        let fetched = tokio::select! {
            _ = shutdown.changed() => Err(PublicServicesError::ShuttingDown),
            result = self.inner.transport.fetch_update() => result,
        };
        match fetched.and_then(|bytes| parse_update_bytes(&bytes)) {
            Ok((presentation, download_url)) => {
                *self.inner.download_url.write().await = download_url;
                *self.inner.update.write().await = presentation.clone();
                presentation
            }
            Err(_) => self.record_update_failure().await,
        }
    }

    async fn check_updates_on_startup(&self) -> UpdatePresentation {
        if self
            .inner
            .startup_update_checked
            .swap(true, Ordering::AcqRel)
        {
            return self.inner.update.read().await.clone();
        }
        self.check_updates().await
    }

    async fn record_update_failure(&self) -> UpdatePresentation {
        *self.inner.download_url.write().await = None;
        let status = UpdatePresentation::status("failed");
        *self.inner.update.write().await = status.clone();
        status
    }

    async fn open_download(&self) -> Result<(), PublicServicesError> {
        let url = self
            .inner
            .download_url
            .read()
            .await
            .clone()
            .ok_or(PublicServicesError::DownloadUnavailable)?;
        if !is_safe_public_https_url(&url) {
            return Err(PublicServicesError::DownloadUnavailable);
        }
        open_in_system_browser(&url)
    }

    /// 中止本模块拥有的任务并让在途请求停止等待。
    pub fn shutdown(&self) {
        self.inner.shutting_down.store(true, Ordering::Release);
        let generation = self.inner.shutdown_tx.borrow().wrapping_add(1);
        let _ = self.inner.shutdown_tx.send(generation);
        if let Ok(mut tasks) = self.inner.tasks.lock() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
    }
}

/// 主窗口就绪后启动一次公共服务检查。
#[tauri::command]
pub async fn start_public_services(
    runtime: tauri::State<'_, PublicServicesRuntime>,
) -> Result<UpdatePresentation, PublicServicesError> {
    Ok(runtime.start_after_window_ready().await)
}

/// 手动检查 application-version API v1。
#[tauri::command]
pub async fn check_public_service_updates(
    runtime: tauri::State<'_, PublicServicesRuntime>,
) -> Result<UpdatePresentation, PublicServicesError> {
    Ok(runtime.check_updates().await)
}

/// 读取本地权威统计同意状态，不触发网络。
#[tauri::command]
pub async fn get_telemetry_enabled(
    runtime: tauri::State<'_, PublicServicesRuntime>,
) -> Result<bool, PublicServicesError> {
    Ok(runtime.inner.store.read().await?.telemetry_enabled)
}

/// 保存统计同意状态；首次启用时尝试发送当天唯一日活。
#[tauri::command]
pub async fn set_telemetry_enabled(
    enabled: bool,
    runtime: tauri::State<'_, PublicServicesRuntime>,
) -> Result<bool, PublicServicesError> {
    let enabled = runtime.inner.store.set_telemetry_enabled(enabled).await?;
    if enabled {
        let _ = runtime.queue_daily_active().await;
    }
    Ok(enabled)
}

/// 打开 Rust 最近一次保存并二次校验的下载地址。
#[tauri::command]
pub async fn open_update_download(
    runtime: tauri::State<'_, PublicServicesRuntime>,
) -> Result<(), PublicServicesError> {
    runtime.open_download().await
}

/// 先关闭本应用拥有的任务，再完整退出进程。
#[tauri::command]
pub fn exit_application(app: tauri::AppHandle) {
    app.state::<PublicServicesRuntime>().shutdown();
    app.exit(0);
}

fn parse_update_bytes(
    bytes: &[u8],
) -> Result<(UpdatePresentation, Option<String>), PublicServicesError> {
    if bytes.len() > MAX_UPDATE_BYTES {
        return Err(PublicServicesError::InvalidResponse);
    }
    let response: ApplicationVersionResponse =
        serde_json::from_slice(bytes).map_err(|_| PublicServicesError::InvalidResponse)?;
    let decision =
        decide_application_version(response).map_err(|_| PublicServicesError::InvalidResponse)?;
    Ok(match decision {
        ApplicationVersionDecision::None => (UpdatePresentation::status("up-to-date"), None),
        ApplicationVersionDecision::Optional(response) => (
            UpdatePresentation::from_response("optional-update", &response, true),
            Some(response.download_url),
        ),
        ApplicationVersionDecision::Required(response) => (
            UpdatePresentation::from_response("required-update", &response, true),
            Some(response.download_url),
        ),
        ApplicationVersionDecision::Stopped(response) => (
            UpdatePresentation::from_response("updates-paused", &response, false),
            None,
        ),
    })
}

fn current_clock_snapshot() -> ClockSnapshot {
    let timestamp = jiff::Timestamp::now();
    let seconds = timestamp.as_second().saturating_add(8 * 60 * 60);
    let days = seconds.div_euclid(86_400);
    let (year, month, day) = civil_from_unix_days(days);
    ClockSnapshot {
        occurred_at: timestamp.to_string(),
        activity_day: format!("{year:04}-{month:02}-{day:02}"),
    }
}

fn civil_from_unix_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn current_platform() -> PublicPlatform {
    #[cfg(target_os = "macos")]
    return PublicPlatform::Macos;
    #[allow(unreachable_code)]
    PublicPlatform::Windows
}

fn current_architecture() -> PublicArchitecture {
    #[cfg(target_arch = "x86_64")]
    return PublicArchitecture::X64;
    #[cfg(target_arch = "aarch64")]
    return PublicArchitecture::Arm64;
    #[cfg(target_arch = "x86")]
    return PublicArchitecture::Ia32;
    #[allow(unreachable_code)]
    PublicArchitecture::X64
}

#[cfg(windows)]
fn open_in_system_browser(url: &str) -> Result<(), PublicServicesError> {
    std::process::Command::new("explorer.exe")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|_| PublicServicesError::DownloadUnavailable)
}

#[cfg(target_os = "macos")]
fn open_in_system_browser(url: &str) -> Result<(), PublicServicesError> {
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|_| PublicServicesError::DownloadUnavailable)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn open_in_system_browser(_url: &str) -> Result<(), PublicServicesError> {
    Err(PublicServicesError::DownloadUnavailable)
}
