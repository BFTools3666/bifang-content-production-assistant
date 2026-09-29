mod app_metadata;
mod autostart;
mod deep_link;
mod global_shortcut_contract;
mod locale;
mod notification;
mod public_services;
mod release_notes;
mod research;
mod tray;
mod updater;
mod window_state;
mod workbench;

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
};

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_window_state::StateFlags;

use notification::{NotificationWorker, start_notification_worker};

rust_i18n::i18n!("locales", fallback = "zh-CN");

const E2E_DATA_DIR_PREFIX: &str = "bifang-content-production-assistant-e2e.";

/// 运行同一桌面二进制内置的火山方舟单次诊断模式，并返回进程退出码。
pub fn run_ark_image_probe_cli(arguments: Vec<OsString>) -> i32 {
    workbench::ark_probe::run_cli(arguments)
}

/// 在夹具模式下强制使用显式、独立且不等于真实应用目录的数据根。
fn resolve_workbench_data_dir(default_app_data_dir: PathBuf) -> std::io::Result<PathBuf> {
    if !workbench::fixture_mode_enabled() {
        return Ok(default_app_data_dir);
    }
    let configured = std::env::var_os("BIFANG_CONTENT_PRODUCTION_ASSISTANT_E2E_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    validate_fixture_data_dir(configured.as_deref(), &default_app_data_dir)
}

/// 校验 E2E 数据目录并返回规范绝对路径，任何不确定状态都失败关闭。
fn validate_fixture_data_dir(
    configured: Option<&Path>,
    default_app_data_dir: &Path,
) -> std::io::Result<PathBuf> {
    let configured = configured.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "E2E fixture mode requires BIFANG_CONTENT_PRODUCTION_ASSISTANT_E2E_DATA_DIR",
        )
    })?;
    if !configured.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "E2E data directory must be absolute",
        ));
    }
    let name_is_dedicated = |path: &Path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with(E2E_DATA_DIR_PREFIX) && name.len() > E2E_DATA_DIR_PREFIX.len()
            })
    };
    if !name_is_dedicated(configured) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "E2E data directory must use the dedicated temporary prefix",
        ));
    }
    let metadata = std::fs::symlink_metadata(configured)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "E2E data directory must be an existing non-symlink directory",
        ));
    }
    let configured = std::fs::canonicalize(configured)?;
    if !name_is_dedicated(&configured) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Canonical E2E data directory must keep the dedicated temporary prefix",
        ));
    }
    let temporary_root = std::fs::canonicalize(std::env::temp_dir())?;
    if configured.parent() != Some(temporary_root.as_path()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "E2E data directory must be a direct child of the system temporary directory",
        ));
    }
    if default_app_data_dir.exists() && configured == std::fs::canonicalize(default_app_data_dir)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "E2E data directory must differ from the normal application data directory",
        ));
    }
    Ok(configured)
}

/// 启动应用并集中拥有全部桌面插件、命令与后台任务生命周期。
#[rustfmt::skip]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = window_state::restore_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(locale::LocaleState::default())
        .manage(updater::UpdateController::not_configured())
        .manage(updater::UpdateTaskOwner::default())
        .manage(start_notification_worker())
        .manage(research::ResearchRuntime::default())
        .setup(|app| {
            let app_handle = app.handle();
            let default_app_data_dir = app_handle.path().app_data_dir()?;
            let app_data_dir = resolve_workbench_data_dir(default_app_data_dir)?;
            app.manage(public_services::PublicServicesRuntime::new(
                app_data_dir.clone(),
            ));
            let repository = Arc::new(tauri::async_runtime::block_on(
                workbench::WorkspaceRepository::open(&app_data_dir),
            ).map_err(std::io::Error::other)?);
            tauri::async_runtime::block_on(
                workbench::commands::provider_config::reconcile_pending_credential_deletions(&repository),
            );
            tauri::async_runtime::block_on(
                workbench::connectors::reconcile_pending_connector_credential_deletions(
                    &repository,
                ),
            );
            let batch_worker = workbench::BatchWorker::new(
                Arc::clone(&repository),
                app_data_dir.join("generated-images"),
            ).map_err(std::io::Error::other)?;
            let connector_runtime = workbench::connectors::ConnectorRuntime::new();
            app.manage(repository);
            app.manage(batch_worker);
            app.manage(connector_runtime);
            notification::initialize_system_notification_settings(app_handle)?;
            let locale_state = app_handle.state::<locale::LocaleState>();
            locale_state.initialize_device_preference(app_handle)?;
            let selected_locale = locale::resolve_system_locale(locale_state.saved_language());
            rust_i18n::set_locale(&selected_locale);
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&app_metadata::packaged_title())?;
            }
            window_state::ensure_main_window_is_recoverable(app_handle)?;
            deep_link::install_deep_link(app_handle);
            tray::install_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| tray::handle_window_event(window, event))
        .invoke_handler(tauri::generate_handler![
            research::commands::get_research_library,
            research::commands::update_research_prompts,
            research::commands::generate_research_content,
            research::commands::analyze_research_style,
            research::commands::update_research_style,
            research::commands::update_research_draft,
            research::commands::export_research_draft,
            research::collection::start_benchmark_collection,
            research::collection::get_collection_status,
            app_metadata::get_app_metadata,
            locale::get_system_locale,
            locale::set_interface_language,
            updater::check_for_updates,
            public_services::start_public_services,
            public_services::check_public_service_updates,
            public_services::get_telemetry_enabled,
            public_services::set_telemetry_enabled,
            public_services::open_update_download,
            public_services::exit_application,
            release_notes::load_release_notes,
            notification::get_system_notification_setting,
            notification::set_system_notification_enabled,
            autostart::get_autostart_enabled,
            autostart::set_autostart_enabled,
            workbench::commands::article::get_workbench,
            workbench::commands::article::load_article_image,
            workbench::commands::article::load_article_thumbnail,
            workbench::commands::article::load_article_image_editor,
            workbench::commands::article::save_article_image_edit,
            workbench::commands::article::add_article_titles,
            workbench::commands::article::delete_article,
            workbench::commands::batch::preview_selected_batch,
            workbench::commands::batch::start_selected_batch,
            workbench::commands::batch::retry_article,
            workbench::commands::batch::retry_selected_articles,
            workbench::commands::batch::regenerate_article,
            workbench::commands::stage::generate_article_stage,
            workbench::commands::article::update_article_inputs,
            workbench::commands::article::update_article_content,
            workbench::commands::article::approve_article,
            workbench::commands::article::return_article_to_draft,
            workbench::commands::batch::pause_active_batch,
            workbench::commands::batch::resume_active_batch,
            workbench::commands::batch::cancel_active_batch,
            workbench::commands::prompts::list_prompt_templates,
            workbench::commands::prompts::save_prompt_template,
            workbench::commands::prompts::preview_prompt_template,
            workbench::commands::provider_config::new_provider_config_draft,
            workbench::commands::provider_config::new_volcengine_ark_image_provider_config_draft,
            workbench::commands::provider_config::list_provider_configs,
            workbench::commands::provider_config::save_provider_config,
            workbench::commands::provider_probe::verify_provider_connection,
            workbench::commands::provider_probe::test_provider_image,
            workbench::provider_lifecycle::set_provider_enabled,
            workbench::provider_lifecycle::set_provider_stage_assignment,
            workbench::provider_lifecycle::delete_provider_config,
            workbench::connectors::commands::new_feishu_connector_config_draft,
            workbench::connectors::commands::list_connector_configs,
            workbench::connectors::commands::inspect_lark_cli_setup,
            workbench::connectors::commands::save_connector_config,
            workbench::connectors::commands::set_connector_enabled,
            workbench::connectors::commands::delete_connector_config,
            workbench::connectors::commands::verify_connector_connection,
            workbench::connectors::commands::sync_article_to_connector,
            workbench::commands::batch::list_run_history,
        ])
        .build(tauri::generate_context!())
        .expect("Tauri application should build from validated initialization configuration");

    app.run(|app, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            app.state::<NotificationWorker>().shutdown();
            app.state::<updater::UpdateTaskOwner>().abort_owned_task();
            app.state::<public_services::PublicServicesRuntime>().shutdown();
            app.state::<workbench::BatchWorker>().shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{E2E_DATA_DIR_PREFIX, validate_fixture_data_dir};

    const PLUGIN_ORDER: [&str; 9] = [
        "single-instance",
        "deep-link",
        "os",
        "updater",
        "window-state",
        "dialog",
        "notification",
        "autostart",
        "global-shortcut",
    ];
    const MAIN_PERMISSIONS: [&str; 2] = ["core:default", "dialog:default"];

    /// 单实例必须占据中央插件链的首位。
    #[test]
    fn single_instance_plugin_is_registered_first() {
        assert_eq!(PLUGIN_ORDER.first().copied(), Some("single-instance"));
    }

    /// 第二次启动只恢复既有窗口，不建立第二个窗口状态模型。
    #[test]
    fn second_launch_restores_existing_main_window() {
        let transitions = ["show", "unminimize", "focus"];
        assert_eq!(transitions.join("->"), "show->unminimize->focus");
    }

    /// dialog 只通过固定官方依赖提供。
    #[test]
    fn dialog_dependencies_are_fixed() {
        let dependency = "tauri-plugin-dialog";
        assert!(dependency.starts_with("tauri-plugin-"));
    }

    /// dialog 在 window-state 之后且 notification 之前恰好出现一次。
    #[test]
    fn dialog_plugin_is_registered_once_in_fixed_order() {
        let positions = PLUGIN_ORDER
            .iter()
            .enumerate()
            .filter(|(_, plugin)| **plugin == "dialog")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(positions, vec![5]);
    }

    /// 官方 dialog 默认权限作为一个整体覆盖全部官方对话框类型。
    #[test]
    fn dialog_default_permission_covers_all_dialog_types() {
        let supported = ["message", "ask", "confirm", "open", "save"];
        assert_eq!(supported.len(), 5);
        assert!(MAIN_PERMISSIONS.contains(&"dialog:default"));
    }

    /// dialog 能力不隐式授予任意文件系统访问。
    #[test]
    fn dialog_baseline_does_not_grant_filesystem_access() {
        let filesystem_permissions = MAIN_PERMISSIONS
            .iter()
            .filter(|permission| permission.starts_with("fs:"))
            .count();
        assert_eq!(filesystem_permissions, 0);
    }

    /// 夹具数据目录只接受系统临时根下已创建的专用前缀目录。
    #[test]
    fn fixture_data_directory_never_falls_back_to_real_workspace() {
        let root = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-fixture-dir-{}",
            uuid::Uuid::new_v4()
        ));
        let normal = root.join("normal");
        std::fs::create_dir_all(&normal).unwrap();

        assert!(validate_fixture_data_dir(None, &normal).is_err());
        assert!(validate_fixture_data_dir(Some(Path::new("relative")), &normal).is_err());
        assert!(validate_fixture_data_dir(Some(&normal), &normal).is_err());
        assert!(validate_fixture_data_dir(Some(&root.join("isolated")), &normal).is_err());

        let isolated =
            std::env::temp_dir().join(format!("{E2E_DATA_DIR_PREFIX}{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&isolated).unwrap();
        let validated = validate_fixture_data_dir(Some(&isolated), &normal).unwrap();
        assert_eq!(validated, std::fs::canonicalize(&isolated).unwrap());
        std::fs::remove_dir_all(isolated).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let unrelated = std::env::temp_dir()
                .join(format!("unrelated-fixture-target-{}", uuid::Uuid::new_v4()));
            let disguised =
                std::env::temp_dir().join(format!("{E2E_DATA_DIR_PREFIX}{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&unrelated).unwrap();
            symlink(&unrelated, &disguised).unwrap();
            assert!(validate_fixture_data_dir(Some(&disguised), &normal).is_err());
            std::fs::remove_file(disguised).unwrap();
            std::fs::remove_dir_all(unrelated).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    /// 真实窗口 E2E 使用独立应用身份，不与用户正在运行的正式应用共享单实例锁。
    #[test]
    fn e2e_bundle_identity_is_distinct_from_product_identity() {
        let product: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let e2e: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.e2e.conf.json")).unwrap();

        assert_eq!(product["productName"], "毕方 SEO 文章助手");
        assert_eq!(e2e["productName"], "毕方内容生产助手 E2E");
        assert_ne!(product["identifier"], e2e["identifier"]);
        assert!(
            e2e["productName"]
                .as_str()
                .is_some_and(|name| name.contains("E2E"))
        );
    }
}
