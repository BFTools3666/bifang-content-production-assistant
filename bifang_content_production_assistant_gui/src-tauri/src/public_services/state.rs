//! 匿名安装身份、统计同意与北京时间每日门禁的失效保护持久化。

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};
use uuid::Uuid;

use super::{APP_ID, PublicServicesError, STATE_SCHEMA_VERSION};

/// 当前应用独享的匿名身份、同意状态与最近一次已认领日期。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PersistedState {
    pub(super) schema_version: u8,
    pub(super) install_id: String,
    pub(super) created_at: String,
    pub(super) telemetry_enabled: bool,
    pub(super) claimed_activity_day: Option<String>,
}

/// 串行化单进程访问，并以独占 marker 协调并发进程。
#[derive(Clone)]
pub(super) struct StateStore {
    pub(super) path: PathBuf,
    current: Arc<Mutex<Option<PersistedState>>>,
}

impl StateStore {
    pub(super) fn new(app_config_root: PathBuf) -> Self {
        Self {
            path: app_config_root
                .join("public-services")
                .join(APP_ID)
                .join("state.json"),
            current: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) async fn read(&self) -> Result<PersistedState, PublicServicesError> {
        let mut owner = self.current.lock().await;
        if let Some(state) = owner.as_ref() {
            return Ok(state.clone());
        }
        let state = load_or_initialize(&self.path).await?;
        *owner = Some(state.clone());
        Ok(state)
    }

    pub(super) async fn set_telemetry_enabled(
        &self,
        enabled: bool,
    ) -> Result<bool, PublicServicesError> {
        let mut owner = self.current.lock().await;
        let mut state = match owner.as_ref() {
            Some(state) => state.clone(),
            None => load_or_initialize(&self.path).await?,
        };
        state.telemetry_enabled = enabled;
        write_state(&self.path, &state).await?;
        *owner = Some(state);
        Ok(enabled)
    }

    pub(super) async fn claim_daily_active(
        &self,
        day: &str,
    ) -> Result<Option<PersistedState>, PublicServicesError> {
        validate_activity_day(day)?;
        let mut owner = self.current.lock().await;
        let mut state = match owner.as_ref() {
            Some(state) => state.clone(),
            None => load_or_initialize(&self.path).await?,
        };
        if !state.telemetry_enabled || state.claimed_activity_day.as_deref() == Some(day) {
            return Ok(None);
        }
        if !claim_day_marker(&self.path, day).await? {
            return Ok(None);
        }
        state.claimed_activity_day = Some(day.to_owned());
        write_state(&self.path, &state).await?;
        *owner = Some(state.clone());
        Ok(Some(state))
    }
}

async fn load_or_initialize(path: &Path) -> Result<PersistedState, PublicServicesError> {
    if path_exists(path).await? {
        return read_state(path).await;
    }
    let candidate = PersistedState {
        schema_version: STATE_SCHEMA_VERSION,
        install_id: Uuid::new_v4().to_string(),
        created_at: jiff::Timestamp::now().to_string(),
        telemetry_enabled: false,
        claimed_activity_day: None,
    };
    publish_state_if_absent(path, &candidate).await?;
    read_state(path).await
}

async fn read_state(path: &Path) -> Result<PersistedState, PublicServicesError> {
    let bytes = fs::read(path)
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    let state: PersistedState =
        serde_json::from_slice(&bytes).map_err(|_| PublicServicesError::InvalidState)?;
    validate_persisted_state(&state)?;
    Ok(state)
}

async fn path_exists(path: &Path) -> Result<bool, PublicServicesError> {
    match fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(PublicServicesError::StorageUnavailable),
    }
}

async fn publish_state_if_absent(
    path: &Path,
    state: &PersistedState,
) -> Result<(), PublicServicesError> {
    let parent = path
        .parent()
        .ok_or(PublicServicesError::StorageUnavailable)?;
    fs::create_dir_all(parent)
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|_| PublicServicesError::StorageUnavailable)?;
    let candidate = parent.join(format!(".initial-{}", Uuid::new_v4()));
    write_synced_new_file(&candidate, &bytes).await?;
    let publish_result = fs::hard_link(&candidate, path).await;
    let _ = fs::remove_file(&candidate).await;
    match publish_result {
        Ok(()) => Ok(()),
        Err(_) if path_exists(path).await? => Ok(()),
        Err(_) => Err(PublicServicesError::StorageUnavailable),
    }
}

async fn claim_day_marker(path: &Path, day: &str) -> Result<bool, PublicServicesError> {
    let parent = path
        .parent()
        .ok_or(PublicServicesError::StorageUnavailable)?;
    fs::create_dir_all(parent)
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    let marker = parent.join(format!("daily-active-{day}.claim"));
    let mut file = match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(marker)
        .await
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(false),
        Err(_) => return Err(PublicServicesError::StorageUnavailable),
    };
    file.write_all(day.as_bytes())
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    file.flush()
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    file.sync_all()
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    Ok(true)
}

fn validate_persisted_state(state: &PersistedState) -> Result<(), PublicServicesError> {
    let install_id =
        Uuid::parse_str(&state.install_id).map_err(|_| PublicServicesError::InvalidState)?;
    if state.schema_version != STATE_SCHEMA_VERSION
        || install_id.get_version_num() != 4
        || state.created_at.parse::<jiff::Timestamp>().is_err()
        || state
            .claimed_activity_day
            .as_deref()
            .is_some_and(|day| validate_activity_day(day).is_err())
    {
        return Err(PublicServicesError::InvalidState);
    }
    Ok(())
}

fn validate_activity_day(day: &str) -> Result<(), PublicServicesError> {
    if day.len() != 10 || day.parse::<jiff::civil::Date>().is_err() {
        return Err(PublicServicesError::InvalidState);
    }
    Ok(())
}

async fn write_synced_new_file(path: &Path, bytes: &[u8]) -> Result<(), PublicServicesError> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    file.write_all(bytes)
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    file.flush()
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    file.sync_all()
        .await
        .map_err(|_| PublicServicesError::StorageUnavailable)?;
    Ok(())
}

async fn write_state(path: &Path, state: &PersistedState) -> Result<(), PublicServicesError> {
    let parent = path
        .parent()
        .ok_or(PublicServicesError::StorageUnavailable)?;
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|_| PublicServicesError::StorageUnavailable)?;
    let candidate = parent.join(format!(".candidate-{}", Uuid::new_v4()));
    let backup = parent.join(format!(".backup-{}", Uuid::new_v4()));
    write_synced_new_file(&candidate, &bytes).await?;
    let had_target = fs::try_exists(path).await.unwrap_or(false);
    if had_target {
        fs::rename(path, &backup)
            .await
            .map_err(|_| PublicServicesError::StorageUnavailable)?;
    }
    if fs::rename(&candidate, path).await.is_err() {
        if had_target {
            let _ = fs::rename(&backup, path).await;
        }
        return Err(PublicServicesError::StorageUnavailable);
    }
    if had_target {
        let _ = fs::remove_file(backup).await;
    }
    Ok(())
}
