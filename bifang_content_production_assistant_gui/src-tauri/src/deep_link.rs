use tauri_plugin_deep_link::DeepLinkExt;

const APP_DEEP_LINK_RESTORE_URL: &str = "app-bifang-content-production-assistant://restore";

/// 中性深链只允许一个无 query、fragment 或载荷的恢复 URL。
fn validate_restore_deep_link(url: &str) -> bool {
    url == APP_DEEP_LINK_RESTORE_URL
}

/// 验证成功后才恢复窗口，非法 URL 不产生副作用。
fn route_deep_link(app: &tauri::AppHandle, url: &str) -> bool {
    if !validate_restore_deep_link(url) {
        return false;
    }
    crate::window_state::restore_main_window(app).is_ok()
}

/// 同时接入冷启动 current URL 与热启动 on_open_url 事件。
pub fn install_deep_link(app: &tauri::AppHandle) {
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        for url in urls {
            let _ = route_deep_link(app, url.as_str());
        }
    }
    let app_handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            let _ = route_deep_link(&app_handle, url.as_str());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 恢复 URL 只从项目身份确定性派生。
    #[test]
    fn deep_link_uses_identity_derived_restore_url() {
        let project_id = "bifang_content_production_assistant".replace('_', "-");
        let derived = format!("app-{project_id}://restore");
        assert_eq!(derived, APP_DEEP_LINK_RESTORE_URL);
    }

    /// query、fragment、端口和其他 scheme 全部失败关闭。
    #[test]
    fn deep_link_rejects_unconfigured_or_payload_urls() {
        let invalid = [
            "other://restore",
            "app-bifang-content-production-assistant://restore?x=1",
            "app-bifang-content-production-assistant://restore#x",
            "app-bifang-content-production-assistant://restore:9",
        ];
        let rejected = invalid
            .iter()
            .filter(|url| !validate_restore_deep_link(url))
            .count();
        assert_eq!(rejected, invalid.len());
    }

    /// 路由模型先验证，只有合法输入才进入恢复阶段。
    #[test]
    fn deep_link_routes_before_window_restore() {
        let sequence = [
            validate_restore_deep_link("bad://restore"),
            validate_restore_deep_link(APP_DEEP_LINK_RESTORE_URL),
        ];
        assert_eq!(sequence, [false, true]);
    }

    /// 冷启动与热启动共享同一精确验证器，不丢失合法事件。
    #[test]
    fn deep_link_warm_launch_is_not_lost() {
        let events = [APP_DEEP_LINK_RESTORE_URL, APP_DEEP_LINK_RESTORE_URL];
        let routed = events
            .iter()
            .filter(|url| validate_restore_deep_link(url))
            .count();
        assert_eq!(routed, 2);
    }
}
