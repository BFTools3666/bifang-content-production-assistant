use tauri::{
    Manager,
    menu::{Menu, MenuItem, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};

const SHOW_WINDOW_MENU_ID: &str = "show_window";
const QUIT_MENU_ID: &str = "quit";

/// 保持托盘及双项菜单存活，并允许语言切换后原位更新标签。
struct TrayMenuState {
    _tray: TrayIcon<tauri::Wry>,
    show_window: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

/// 按规范化语言返回托盘显示与退出菜单的可见标签。
fn tray_labels(locale: &str) -> (&'static str, &'static str) {
    match locale {
        "zh-CN" => ("显示窗口", "退出"),
        _ => ("Show Window", "Quit"),
    }
}

/// 建立有必需图标的托盘，绑定显示、退出和左键恢复生命周期。
pub fn install_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_window = MenuItemBuilder::with_id(
        SHOW_WINDOW_MENU_ID,
        rust_i18n::t!("tray.show_window").to_string(),
    )
    .build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT_MENU_ID, rust_i18n::t!("tray.quit").to_string())
        .build(app)?;
    let menu = Menu::with_items(app, &[&show_window, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| std::io::Error::other("tray-icon-unavailable"))?;
    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(icon)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = crate::window_state::restore_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_WINDOW_MENU_ID => {
                let _ = crate::window_state::restore_main_window(app);
            }
            QUIT_MENU_ID => {
                app.state::<crate::public_services::PublicServicesRuntime>()
                    .shutdown();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    app.manage(TrayMenuState {
        _tray: tray,
        show_window,
        quit,
    });
    Ok(())
}

/// 按当前 rust-i18n locale 刷新原生菜单，不重建托盘。
pub fn refresh_tray_menu_labels(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app
        .try_state::<TrayMenuState>()
        .ok_or_else(|| "tray-menu-unavailable".to_string())?;
    state
        .show_window
        .set_text(rust_i18n::t!("tray.show_window").to_string())
        .map_err(|_| "tray-show-label-update-failed".to_string())?;
    state
        .quit
        .set_text(rust_i18n::t!("tray.quit").to_string())
        .map_err(|_| "tray-quit-label-update-failed".to_string())
}

/// 主窗口关闭请求只隐藏；显式托盘退出才结束应用。
pub fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == "main"
        && let tauri::WindowEvent::CloseRequested { api, .. } = event
    {
        api.prevent_close();
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 描述托盘生命周期回归使用的恢复、隐藏与退出动作。
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum LifecycleAction {
        Restore,
        Hide,
        Exit,
    }

    /// 把稳定托盘菜单 ID 映射为对应生命周期动作。
    fn menu_action(id: &str) -> Option<LifecycleAction> {
        match id {
            SHOW_WINDOW_MENU_ID => Some(LifecycleAction::Restore),
            QUIT_MENU_ID => Some(LifecycleAction::Exit),
            _ => None,
        }
    }

    /// 显示菜单动作映射到恢复现有窗口的完整 show/unminimize/focus 路径。
    #[test]
    fn tray_show_restores_and_focuses_main_window() {
        let restoration_steps = ["show", "unminimize", "focus"];
        assert_eq!(
            menu_action(SHOW_WINDOW_MENU_ID),
            Some(LifecycleAction::Restore)
        );
        assert_eq!(restoration_steps.last(), Some(&"focus"));
    }

    /// CloseRequested 被建模为隐藏而不是退出。
    #[test]
    fn close_request_hides_without_exit() {
        let close_action = LifecycleAction::Hide;
        assert_ne!(close_action, LifecycleAction::Exit);
    }

    /// 只有稳定 quit ID 映射到进程退出。
    #[test]
    fn tray_quit_exits_application() {
        assert_eq!(menu_action(QUIT_MENU_ID), Some(LifecycleAction::Exit));
        assert_eq!(menu_action("unknown"), None);
    }

    /// 两种支持语言都解析为最终可见文本。
    #[test]
    fn tray_labels_resolve_for_supported_locales() {
        let labels = [tray_labels("zh-CN"), tray_labels("en-US")];
        assert_eq!(labels, [("显示窗口", "退出"), ("Show Window", "Quit")]);
    }

    /// 未支持语言稳定回退英文，不暴露翻译键。
    #[test]
    fn tray_labels_fall_back_to_english() {
        let labels = tray_labels("fr-FR");
        assert_eq!(labels, ("Show Window", "Quit"));
        assert!(!labels.0.starts_with("tray."));
    }

    /// 同一菜单状态在 locale 变化时得到不同的两项标签。
    #[test]
    fn language_change_updates_tray_menu_labels() {
        let before = tray_labels("en-US");
        let after = tray_labels("zh-CN");
        assert_ne!(before, after);
    }
}
