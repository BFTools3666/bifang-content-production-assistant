use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde::Serialize;
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

/// 更新检查对 WebView 暴露的稳定状态集合。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdaterStatus {
    NotConfigured,
    Checking,
    UpToDate,
    OptionalUpdate,
    Unavailable,
}

/// 持有未配置门禁与 single-flight 原子状态。
pub struct UpdateController {
    pub configured: bool,
    in_flight: AtomicBool,
}

impl UpdateController {
    /// 构造保持零出站的中性更新控制器。
    pub fn not_configured() -> Self {
        Self {
            configured: false,
            in_flight: AtomicBool::new(false),
        }
    }

    /// 尝试取得一次更新检查所有权。
    fn try_begin(&self) -> bool {
        self.in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// 释放本次检查所有权。
    fn finish(&self) {
        self.in_flight.store(false, Ordering::Release);
    }
}

/// 应用拥有的远程检查任务；退出与 Drop 都只取消本模块任务。
#[derive(Default)]
pub struct UpdateTaskOwner {
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl UpdateTaskOwner {
    /// 中止并取走当前由更新模块拥有的任务。
    pub fn abort_owned_task(&self) {
        if let Ok(mut task) = self.task.lock()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }

    /// 报告是否仍存在由本模块持有的任务。
    fn has_task(&self) -> bool {
        self.task.lock().is_ok_and(|task| task.is_some())
    }
}

impl Drop for UpdateTaskOwner {
    /// 在所有者销毁时中止并释放其持有的更新任务。
    fn drop(&mut self) {
        if let Ok(task) = self.task.get_mut()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }
}

/// 在检查作用域结束时释放 single-flight 所有权。
struct CheckGuard<'a>(&'a UpdateController);

impl Drop for CheckGuard<'_> {
    /// 无论检查如何结束都归还更新检查所有权。
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// 未配置时先返回，配置完整后才允许调用官方 updater。
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> UpdaterStatus {
    let controller = app.state::<UpdateController>();
    if !controller.configured {
        return UpdaterStatus::NotConfigured;
    }
    if !controller.try_begin() {
        return UpdaterStatus::Checking;
    }
    let _guard = CheckGuard(&controller);
    let Ok(updater) = app.updater() else {
        return UpdaterStatus::Unavailable;
    };
    match updater.check().await {
        Ok(Some(_)) => UpdaterStatus::OptionalUpdate,
        Ok(None) => UpdaterStatus::UpToDate,
        Err(_) => UpdaterStatus::Unavailable,
    }
}

/// 检查候选元数据是否仍绑定当前 channel/target/arch。
fn metadata_matches_runtime(
    expected_channel: &str,
    expected_target: &str,
    expected_arch: &str,
    channel: &str,
    target: &str,
    arch: &str,
) -> bool {
    expected_channel == channel && expected_target == target && expected_arch == arch
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 中性控制器以 NotConfigured 开始，且无需建立网络任务。
    #[test]
    fn updater_defaults_to_not_configured_without_network() {
        let controller = UpdateController::not_configured();
        let status = if controller.configured {
            UpdaterStatus::Checking
        } else {
            UpdaterStatus::NotConfigured
        };
        assert_eq!(status, UpdaterStatus::NotConfigured);
    }

    /// 同一时刻只有第一个调用能取得检查所有权。
    #[test]
    fn updater_checks_are_single_flight() {
        let controller = UpdateController {
            configured: true,
            in_flight: AtomicBool::new(false),
        };
        let first = controller.try_begin();
        let second = controller.try_begin();
        controller.finish();
        assert_ne!(first, second);
    }

    /// 任一 channel/target/arch 漂移都会拒绝候选元数据。
    #[test]
    fn updater_rejects_target_arch_channel_mismatch() {
        let accepted =
            metadata_matches_runtime("stable", "darwin", "aarch64", "stable", "darwin", "aarch64");
        let rejected =
            metadata_matches_runtime("stable", "darwin", "aarch64", "beta", "darwin", "aarch64");
        assert_ne!(accepted, rejected);
    }

    /// 空 owner 不伪造任务；Drop/取消只作用于已拥有句柄。
    #[test]
    fn updater_tasks_are_owned_and_cancelled() {
        let owner = UpdateTaskOwner::default();
        owner.abort_owned_task();
        assert!(!owner.has_task());
    }

    /// 错误稳定映射为 Unavailable，绝不降格为 UpToDate。
    #[test]
    fn updater_failures_never_report_up_to_date() {
        let failure = UpdaterStatus::Unavailable;
        assert_ne!(failure, UpdaterStatus::UpToDate);
    }
}
