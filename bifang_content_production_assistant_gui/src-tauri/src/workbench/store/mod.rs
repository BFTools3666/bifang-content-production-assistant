//! 使用 SQLite/SeaORM 原子保存小型本地工作区快照。

use std::path::Path;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use tokio::sync::{Mutex, MutexGuard};

use bifang_content_production_assistant_core::{
    recover_abandoned_runs, recover_interrupted_articles,
};

use super::models::WorkspaceData;

#[cfg(test)]
use super::models::RunStatus;

/// 序列化所有工作区更新并原子写入 SQLite 的仓储。
pub struct WorkspaceRepository {
    database: DatabaseConnection,
    data: Mutex<WorkspaceData>,
}

impl WorkspaceRepository {
    /// 在应用数据目录打开或创建工作区数据库。
    pub async fn open(app_data_dir: &Path) -> Result<Self, String> {
        tokio::fs::create_dir_all(app_data_dir)
            .await
            .map_err(|_| "无法创建应用数据目录".to_string())?;
        let database_path = app_data_dir.join("content-workbench.sqlite3");
        let database_url = format!("sqlite://{}?mode=rwc", database_path.to_string_lossy());
        Self::open_database_url_with_initial_content(&database_url, true).await
    }

    /// 打开明确数据库 URL；该入口也用于隔离的内存集成测试。
    pub async fn open_database_url(database_url: &str) -> Result<Self, String> {
        Self::open_database_url_with_initial_content(database_url, false).await
    }

    /// 打开数据库，并在普通应用入口中选择是否执行一次性业务初始数据迁移。
    async fn open_database_url_with_initial_content(
        database_url: &str,
        seed_initial_content: bool,
    ) -> Result<Self, String> {
        let mut options = ConnectOptions::new(database_url.to_owned());
        options.max_connections(1).sqlx_logging(false);
        let database = Database::connect(options)
            .await
            .map_err(|_| "无法打开本地工作区数据库".to_string())?;
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE IF NOT EXISTS workspace_snapshot (\
                    id INTEGER PRIMARY KEY CHECK (id = 1), \
                    schema_version INTEGER NOT NULL, \
                    payload TEXT NOT NULL, \
                    updated_at_ms INTEGER NOT NULL\
                )"
                .to_string(),
            ))
            .await
            .map_err(|_| "无法初始化本地工作区数据库".to_string())?;

        let row = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM workspace_snapshot WHERE id = 1".to_string(),
            ))
            .await
            .map_err(|_| "无法读取本地工作区".to_string())?;
        let is_new = row.is_none();
        let mut data = match row {
            Some(row) => {
                let payload: String = row
                    .try_get("", "payload")
                    .map_err(|_| "本地工作区数据无法读取".to_string())?;
                serde_json::from_str::<WorkspaceData>(&payload)
                    .map_err(|_| "本地工作区数据格式无效".to_string())?
            }
            None => WorkspaceData::with_mode_defaults(super::models::fixture_mode_enabled()),
        };
        let schema_changed = data.migrate_to_current_schema()?;
        let environment_changed =
            data.normalize_environment_for_mode(super::models::fixture_mode_enabled());
        let now_ms = current_time_ms();
        let initial_content_changed = seed_initial_content
            && data.apply_initial_content(super::models::fixture_mode_enabled(), now_ms)?;
        let recovered_tasks = recover_interrupted_articles(&mut data, now_ms);
        let recovered_runs = recover_abandoned_runs(&mut data, now_ms);
        let repository = Self {
            database,
            data: Mutex::new(data),
        };
        if is_new
            || schema_changed
            || environment_changed
            || initial_content_changed
            || recovered_tasks > 0
            || recovered_runs > 0
        {
            let snapshot = repository.snapshot().await;
            repository.persist(&snapshot).await?;
        }
        Ok(repository)
    }

    /// 返回当前内存权威状态的独立快照。
    pub async fn snapshot(&self) -> WorkspaceData {
        self.data.lock().await.clone()
    }

    /// 把最终外部写入检查与子进程启动纳入同一内存状态序列；启动后必须立即释放。
    pub(crate) async fn lock_for_external_write(&self) -> MutexGuard<'_, WorkspaceData> {
        self.data.lock().await
    }

    /// 只按文章 ID 读取当前配图引用，避免 Grid 缩略图为每行克隆整个工作区。
    pub async fn article_image_reference(&self, task_id: &str) -> Result<Option<String>, String> {
        self.data
            .lock()
            .await
            .articles
            .iter()
            .find(|article| article.id == task_id)
            .map(|article| article.artifacts.image_ref.clone())
            .ok_or_else(|| "找不到指定文章".to_string())
    }

    /// 在单一写入序列中修改状态并持久化；外部网络调用不得进入闭包。
    pub async fn update<R>(
        &self,
        update: impl FnOnce(&mut WorkspaceData) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut data = self.data.lock().await;
        let mut candidate = data.clone();
        let result = update(&mut candidate)?;
        self.persist(&candidate).await?;
        *data = candidate;
        Ok(result)
    }

    /// 在一个 SQLite 事务中替换完整快照，避免部分状态可见。
    async fn persist(&self, data: &WorkspaceData) -> Result<(), String> {
        let payload =
            serde_json::to_string(data).map_err(|_| "无法序列化本地工作区".to_string())?;
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| "无法开始本地保存事务".to_string())?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO workspace_snapshot (id, schema_version, payload, updated_at_ms) \
                 VALUES (1, ?, ?, ?) \
                 ON CONFLICT(id) DO UPDATE SET \
                 schema_version = excluded.schema_version, \
                 payload = excluded.payload, \
                 updated_at_ms = excluded.updated_at_ms",
                [
                    i64::from(data.schema_version).into(),
                    payload.into(),
                    current_time_ms().into(),
                ],
            ))
            .await
            .map_err(|_| "无法写入本地工作区".to_string())?;
        transaction
            .commit()
            .await
            .map_err(|_| "无法提交本地工作区".to_string())?;
        Ok(())
    }

    /// 测试专用：读取 SQLite 中实际保存的原始快照，验证秘密不会进入载荷。
    #[cfg(test)]
    pub(crate) async fn raw_snapshot_payload(&self) -> String {
        self.database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM workspace_snapshot WHERE id = 1".to_string(),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "payload")
            .unwrap()
    }

    /// 测试专用：让后续快照更新失败，验证远端副作用后的失败关闭语义。
    #[cfg(test)]
    pub(crate) async fn reject_snapshot_updates_for_test(&self) {
        self.database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TRIGGER reject_snapshot_update_for_test BEFORE UPDATE ON workspace_snapshot \
                 BEGIN SELECT RAISE(FAIL, 'simulated persistence failure'); END"
                    .to_string(),
            ))
            .await
            .unwrap();
    }
}

/// 返回当前 Unix 毫秒时间；时钟只由适配器读取。
pub fn current_time_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests;
