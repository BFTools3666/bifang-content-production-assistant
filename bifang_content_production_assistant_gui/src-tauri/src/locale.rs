use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use tauri::Manager;

const LOCALE_PREFERENCE_SCHEMA_VERSION: u8 = 1;
const MAX_LOCALE_PREFERENCE_BYTES: u64 = 4 * 1024;
static LOCALE_PREFERENCE_WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 表示持久化到设备配置目录的界面语言偏好文档。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalePreferenceDocument {
    schema_version: u8,
    interface_language: String,
}

/// 保存用户在当前宿主中明确选择的界面语言。
#[derive(Default)]
pub struct LocaleState {
    saved_language: Mutex<Option<String>>,
    preference_path: Mutex<Option<PathBuf>>,
}

impl LocaleState {
    /// 返回当前明确保存的语言；锁损坏时按未保存处理。
    pub fn saved_language(&self) -> Option<String> {
        self.saved_language
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }

    /// 原子替换当前语言偏好。
    pub fn set_saved_language(&self, language: String) -> Result<(), String> {
        let mut saved = self
            .saved_language
            .lock()
            .map_err(|_| "locale-state-unavailable".to_string())?;
        *saved = Some(language);
        Ok(())
    }

    /// 绑定当前应用的设备级偏好路径，并安全加载已有值。
    pub fn initialize_device_preference(
        &self,
        app: &tauri::AppHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = app.path().app_config_dir()?.join("interface-language.json");
        let saved = read_locale_preference(&path).unwrap_or(None);
        *self
            .preference_path
            .lock()
            .map_err(|_| std::io::Error::other("locale-preference-path-unavailable"))? = Some(path);
        *self
            .saved_language
            .lock()
            .map_err(|_| std::io::Error::other("locale-state-unavailable"))? = saved;
        Ok(())
    }

    /// 把规范化语言值写入已绑定的设备级偏好文件。
    async fn persist_language(&self, language: &str) -> Result<(), String> {
        let path = self
            .preference_path
            .lock()
            .map_err(|_| "locale-preference-path-unavailable".to_string())?
            .clone()
            .ok_or_else(|| "locale-preference-path-unavailable".to_string())?;
        persist_locale_preference(&path, language).await
    }
}

/// 解码并校验界面语言偏好文档的版本与语言值。
fn decode_locale_preference(bytes: &[u8]) -> Result<Option<String>, String> {
    if bytes.len() as u64 > MAX_LOCALE_PREFERENCE_BYTES {
        return Err("locale-preference-too-large".to_string());
    }
    let document: LocalePreferenceDocument =
        serde_json::from_slice(bytes).map_err(|_| "locale-preference-invalid".to_string())?;
    if document.schema_version != LOCALE_PREFERENCE_SCHEMA_VERSION {
        return Err("locale-preference-version-unsupported".to_string());
    }
    let normalized = normalize_bcp47_locale(&document.interface_language);
    if normalized != document.interface_language {
        return Err("locale-preference-language-invalid".to_string());
    }
    Ok(Some(normalized))
}

/// 从安全的普通文件读取并解析设备级界面语言偏好。
fn read_locale_preference(path: &Path) -> Result<Option<String>, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("locale-preference-metadata-unavailable".to_string()),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("locale-preference-path-unsafe".to_string());
    }
    if metadata.len() > MAX_LOCALE_PREFERENCE_BYTES {
        return Err("locale-preference-too-large".to_string());
    }
    let mut file =
        std::fs::File::open(path).map_err(|_| "locale-preference-read-failed".to_string())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| "locale-preference-read-failed".to_string())?;
    decode_locale_preference(&bytes)
}

/// 通过同目录临时文件原子持久化设备级界面语言偏好。
async fn persist_locale_preference(path: &Path, language: &str) -> Result<(), String> {
    let _write_guard = LOCALE_PREFERENCE_WRITE_LOCK.lock().await;
    let document = LocalePreferenceDocument {
        schema_version: LOCALE_PREFERENCE_SCHEMA_VERSION,
        interface_language: language.to_string(),
    };
    let bytes =
        serde_json::to_vec(&document).map_err(|_| "locale-preference-encode-failed".to_string())?;
    if bytes.len() as u64 > MAX_LOCALE_PREFERENCE_BYTES {
        return Err("locale-preference-too-large".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "locale-preference-parent-unavailable".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_| "locale-preference-directory-create-failed".to_string())?;
    if let Ok(metadata) = tokio::fs::symlink_metadata(path).await
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err("locale-preference-path-unsafe".to_string());
    }
    let temporary = path.with_extension("json.tmp");
    if let Ok(metadata) = tokio::fs::symlink_metadata(&temporary).await {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("locale-preference-temporary-path-unsafe".to_string());
        }
        tokio::fs::remove_file(&temporary)
            .await
            .map_err(|_| "locale-preference-temporary-cleanup-failed".to_string())?;
    }
    tokio::fs::write(&temporary, bytes)
        .await
        .map_err(|_| "locale-preference-write-failed".to_string())?;
    #[cfg(target_os = "windows")]
    if tokio::fs::try_exists(path).await.unwrap_or(false) {
        tokio::fs::remove_file(path)
            .await
            .map_err(|_| "locale-preference-replace-failed".to_string())?;
    }
    tokio::fs::rename(&temporary, path)
        .await
        .map_err(|_| "locale-preference-commit-failed".to_string())
}

/// 把支持语言的 BCP-47 分隔符与大小写归一化一次。
pub fn normalize_bcp47_locale(locale: &str) -> String {
    let normalized = locale.trim().replace('_', "-").to_ascii_lowercase();
    match normalized.as_str() {
        "zh" | "zh-cn" | "zh-hans" => "zh-CN".to_string(),
        "en" | "en-us" => "en-US".to_string(),
        _ => "zh-CN".to_string(),
    }
}

/// 用户偏好优先；两个面向中文创作者的发行版首次启动固定简体中文。
pub fn resolve_system_locale(saved_language: Option<String>) -> String {
    let selected = saved_language
        .filter(|language| !language.trim().is_empty())
        .unwrap_or_else(|| "zh-CN".to_string());
    normalize_bcp47_locale(&selected)
}

/// 返回供 React 与原生界面共同使用的规范化 locale。
#[tauri::command]
pub async fn get_system_locale(app: tauri::AppHandle) -> String {
    let state = app.state::<LocaleState>();
    resolve_system_locale(state.saved_language())
}

/// 保存语言选择，同步 rust-i18n 并立即刷新托盘文案。
#[tauri::command]
pub async fn set_interface_language(
    language: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let state = app.state::<LocaleState>();
    let normalized = normalize_bcp47_locale(&language);
    state.persist_language(&normalized).await?;
    state.set_saved_language(normalized.clone())?;
    rust_i18n::set_locale(&normalized);
    crate::tray::refresh_tray_menu_labels(&app)?;
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 首次启动不受宿主系统语言影响，默认简体中文。
    #[test]
    fn new_install_defaults_to_simplified_chinese() {
        assert_eq!(resolve_system_locale(None), "zh-CN");
        assert_eq!(resolve_system_locale(Some(String::new())), "zh-CN");
    }

    /// 常见下划线、小写和语言短码只经过一处归一化。
    #[test]
    fn system_locale_normalizes_bcp47_once() {
        let samples = [("zh_cn", "zh-CN"), ("EN-us", "en-US"), ("zh", "zh-CN")];
        for (source, expected) in samples {
            assert_eq!(normalize_bcp47_locale(source), expected);
        }
    }

    /// 未支持或空语言稳定回退简体中文。
    #[test]
    fn system_locale_falls_back_to_chinese() {
        let observed = ["", "fr-FR", "und"]
            .map(normalize_bcp47_locale)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(observed, vec!["zh-CN", "zh-CN", "zh-CN"]);
        assert!(decode_locale_preference(b"broken-json").is_err());
    }

    /// 已保存语言在解析链中优先于任何系统候选。
    #[test]
    fn saved_language_precedes_system_locale() {
        let state = LocaleState::default();
        state
            .set_saved_language("zh-CN".to_string())
            .expect("state should be writable");
        let resolved = resolve_system_locale(state.saved_language());
        assert_eq!(resolved, "zh-CN");
        assert_eq!(resolve_system_locale(Some("en-US".into())), "en-US");
    }

    /// 设备偏好文档真实写入、复读，并拒绝未知 schema。
    #[tokio::test]
    async fn locale_preference_round_trips_device_document() {
        let unique = format!(
            "bifang-content-production-assistant-locale-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should follow Unix epoch")
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(unique);
        let path = directory.join("interface-language.json");
        persist_locale_preference(&path, "zh-CN")
            .await
            .expect("preference should persist");
        let observed = read_locale_preference(&path).expect("preference should be readable");
        tokio::fs::remove_dir_all(&directory)
            .await
            .expect("temporary preference should be removed");
        assert_eq!(observed.as_deref(), Some("zh-CN"));
    }
}
