use tauri::{LogicalSize, Manager};

/// 供可恢复性判定使用的物理显示器矩形。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MonitorGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// 判断已恢复窗口是否满足最小尺寸且与任一当前显示器相交。
pub fn saved_window_geometry_is_recoverable(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitors: &[MonitorGeometry],
) -> bool {
    width >= 960
        && height >= 640
        && monitors.iter().any(|monitor| {
            let window_right = i64::from(x) + i64::from(width);
            let window_bottom = i64::from(y) + i64::from(height);
            let monitor_right = i64::from(monitor.x) + i64::from(monitor.width);
            let monitor_bottom = i64::from(monitor.y) + i64::from(monitor.height);
            i64::from(x) < monitor_right
                && window_right > i64::from(monitor.x)
                && i64::from(y) < monitor_bottom
                && window_bottom > i64::from(monitor.y)
        })
}

/// 在插件恢复后纠正过小或完全离屏的主窗口。
pub fn ensure_main_window_is_recoverable(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main-window-unavailable".to_string())?;
    window
        .set_min_size(Some(LogicalSize::new(960.0, 640.0)))
        .map_err(|_| "window-minimum-size-failed".to_string())?;
    let position = window
        .outer_position()
        .map_err(|_| "window-position-unavailable".to_string())?;
    let size = window
        .outer_size()
        .map_err(|_| "window-size-unavailable".to_string())?;
    let monitors = window
        .available_monitors()
        .map_err(|_| "window-monitors-unavailable".to_string())?
        .into_iter()
        .map(|monitor| MonitorGeometry {
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
        })
        .collect::<Vec<_>>();
    if !saved_window_geometry_is_recoverable(
        position.x,
        position.y,
        size.width,
        size.height,
        &monitors,
    ) {
        window
            .set_size(LogicalSize::new(1440.0, 900.0))
            .map_err(|_| "window-default-size-failed".to_string())?;
        window
            .center()
            .map_err(|_| "window-center-failed".to_string())?;
    }
    Ok(())
}

/// 恢复、取消最小化并聚焦既有主窗口。
pub fn restore_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main-window-unavailable".to_string())?;
    window
        .show()
        .map_err(|_| "window-show-failed".to_string())?;
    window
        .unminimize()
        .map_err(|_| "window-unminimize-failed".to_string())?;
    window
        .set_focus()
        .map_err(|_| "window-focus-failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造窗口状态回归使用的单显示器几何。
    fn display() -> Vec<MonitorGeometry> {
        vec![MonitorGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }]
    }

    /// 合法尺寸与位置可保留，最大化由精确插件 flags 另行恢复。
    #[test]
    fn window_state_restores_size_position_and_maximized() {
        let recoverable = saved_window_geometry_is_recoverable(120, 80, 1200, 800, &display());
        assert!(recoverable);
    }

    /// 隐藏状态不参与几何可恢复性判定。
    #[test]
    fn window_state_ignores_saved_visibility() {
        let visible = false;
        let recoverable = saved_window_geometry_is_recoverable(0, 0, 960, 640, &display());
        assert_ne!(visible, recoverable);
    }

    /// 过小与完全离屏几何都必须触发固定回退。
    #[test]
    fn window_state_falls_back_for_invalid_or_offscreen_state() {
        let small = saved_window_geometry_is_recoverable(0, 0, 959, 640, &display());
        let offscreen = saved_window_geometry_is_recoverable(50_000, 50_000, 1440, 900, &display());
        assert_eq!((small, offscreen), (false, false));
    }

    /// 首次窗口的 1440×900 默认值在有效显示器上可找回。
    #[test]
    fn window_state_preserves_first_launch_defaults() {
        let first_launch = saved_window_geometry_is_recoverable(200, 90, 1440, 900, &display());
        assert!(first_launch);
    }
}
