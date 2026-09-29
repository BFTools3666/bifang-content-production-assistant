use std::sync::{Arc, Mutex as StdMutex};

use bifang_content_production_assistant_core::public_services::DailyActivePayload;
use tempfile::tempdir;
use tokio::fs;

use super::{
    APP_ID, ClockSnapshot, PublicServicesError, PublicServicesRuntime, UpdatePresentation,
    parse_update_bytes,
    state::StateStore,
    transport::{PublicServicesTransport, TransportFuture},
};

struct FakeTransport {
    telemetry: StdMutex<Vec<DailyActivePayload>>,
    update: Result<Vec<u8>, PublicServicesError>,
    update_requests: StdMutex<usize>,
    telemetry_configured: bool,
    updates_configured: bool,
}

impl FakeTransport {
    fn configured(update: Vec<u8>) -> Self {
        Self {
            telemetry: StdMutex::new(Vec::new()),
            update: Ok(update),
            update_requests: StdMutex::new(0),
            telemetry_configured: true,
            updates_configured: true,
        }
    }

    fn unconfigured() -> Self {
        Self {
            telemetry: StdMutex::new(Vec::new()),
            update: Err(PublicServicesError::UpdateNotConfigured),
            update_requests: StdMutex::new(0),
            telemetry_configured: false,
            updates_configured: false,
        }
    }
}

impl PublicServicesTransport for FakeTransport {
    fn send_telemetry(&self, payload: DailyActivePayload) -> TransportFuture<'_, bool> {
        Box::pin(async move {
            self.telemetry.lock().expect("测试锁").push(payload);
            true
        })
    }

    fn fetch_update(&self) -> TransportFuture<'_, Result<Vec<u8>, PublicServicesError>> {
        Box::pin(async move {
            *self.update_requests.lock().expect("测试锁") += 1;
            self.update.clone()
        })
    }

    fn telemetry_configured(&self) -> bool {
        self.telemetry_configured
    }

    fn updates_configured(&self) -> bool {
        self.updates_configured
    }
}

fn version_response(update: bool, forced: bool, prompt: bool, stopped: bool) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "update": update,
        "downloadUrl": "https://downloads.example.com/assistant.exe",
        "promptContent": "发现新版本",
        "forcedUpdate": forced,
        "promptUpdate": prompt,
        "stopUpdate": stopped,
        "updateContent": "请前往官方下载页获取更新。"
    }))
    .expect("测试响应")
}

fn fixed_clock() -> ClockSnapshot {
    ClockSnapshot {
        occurred_at: "2026-09-28T16:30:00Z".to_owned(),
        activity_day: "2026-09-29".to_owned(),
    }
}

async fn wait_for_telemetry(transport: &FakeTransport, expected: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if transport.telemetry.lock().expect("测试锁").len() >= expected {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("统计任务应执行");
}

#[tokio::test]
async fn telemetry_is_opt_in_and_only_sent_once_per_beijing_day() {
    let root = tempdir().expect("临时目录");
    let transport = Arc::new(FakeTransport::configured(version_response(
        false, false, false, false,
    )));
    let runtime = PublicServicesRuntime::with_components(
        root.path().to_path_buf(),
        transport.clone(),
        Arc::new(fixed_clock),
    );
    runtime.start_after_window_ready().await;
    assert!(transport.telemetry.lock().expect("测试锁").is_empty());
    runtime
        .inner
        .store
        .set_telemetry_enabled(true)
        .await
        .expect("启用统计");
    runtime.queue_daily_active().await.expect("排队统计");
    runtime.queue_daily_active().await.expect("重复请求");
    wait_for_telemetry(&transport, 1).await;
    assert_eq!(transport.telemetry.lock().expect("测试锁").len(), 1);
    assert_eq!(
        transport.telemetry.lock().expect("测试锁")[0].app_id,
        APP_ID
    );
    runtime.shutdown();
}

#[tokio::test]
async fn state_is_stable_and_corruption_fails_closed() {
    let root = tempdir().expect("临时目录");
    let store = StateStore::new(root.path().to_path_buf());
    let state = store.read().await.expect("创建状态");
    assert!(!state.telemetry_enabled);
    assert_eq!(
        uuid::Uuid::parse_str(&state.install_id)
            .expect("UUID")
            .get_version_num(),
        4
    );
    fs::write(&store.path, b"not-json")
        .await
        .expect("覆盖测试状态");
    let reloaded = StateStore::new(root.path().to_path_buf());
    assert!(matches!(
        reloaded.read().await,
        Err(PublicServicesError::InvalidState)
    ));
}

#[tokio::test]
async fn unconfigured_services_make_no_requests() {
    let root = tempdir().expect("临时目录");
    let transport = Arc::new(FakeTransport::unconfigured());
    let runtime = PublicServicesRuntime::with_components(
        root.path().to_path_buf(),
        transport.clone(),
        Arc::new(fixed_clock),
    );
    let status = runtime.start_after_window_ready().await;
    assert_eq!(status.status, "not-configured");
    assert!(transport.telemetry.lock().expect("测试锁").is_empty());
    assert_eq!(*transport.update_requests.lock().expect("测试锁"), 0);
}

#[test]
fn strict_seven_field_response_and_truth_table_are_preserved() {
    let cases = [
        ((false, false, false, false), "up-to-date", false),
        ((true, false, true, false), "optional-update", true),
        ((true, true, false, false), "required-update", true),
        ((false, false, false, true), "updates-paused", false),
    ];
    for ((update, forced, prompt, stopped), status, download_ready) in cases {
        let presentation = parse_update_bytes(&version_response(update, forced, prompt, stopped))
            .expect("真值表")
            .0;
        assert_eq!(presentation.status, status);
        assert_eq!(presentation.download_ready, download_ready);
    }
    let mut extra: serde_json::Value =
        serde_json::from_slice(&version_response(true, false, true, false)).expect("JSON");
    extra["targetVersion"] = serde_json::json!("1.0.0");
    assert!(parse_update_bytes(&serde_json::to_vec(&extra).expect("编码")).is_err());
}

#[tokio::test]
async fn startup_update_check_is_single_shot_and_manual_check_is_available() {
    let root = tempdir().expect("临时目录");
    let transport = Arc::new(FakeTransport::configured(version_response(
        false, false, false, false,
    )));
    let runtime = PublicServicesRuntime::with_components(
        root.path().to_path_buf(),
        transport.clone(),
        Arc::new(fixed_clock),
    );
    runtime.start_after_window_ready().await;
    runtime.start_after_window_ready().await;
    assert_eq!(*transport.update_requests.lock().expect("测试锁"), 1);
    let presentation: UpdatePresentation = runtime.check_updates().await;
    assert_eq!(presentation.status, "up-to-date");
    assert_eq!(*transport.update_requests.lock().expect("测试锁"), 2);
}
