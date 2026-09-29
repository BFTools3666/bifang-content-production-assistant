//! 工作区 SQLite 快照的严格只读 CLI 适配器。

use std::{io::ErrorKind, path::Path};

use bifang_content_production_assistant_core::{
    ArticleReadError, ArticleReadFuture, ArticleReadRepository, ArticleWorkspaceSnapshot,
    CURRENT_WORKSPACE_SCHEMA_VERSION,
};
use sea_orm::sqlx::{
    self, Error as SqlxError, Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

const DATABASE_FILE_NAME: &str = "content-workbench.sqlite3";

/// 只允许读取一个经过路径边界校验的现有 SQLite 工作区。
pub(crate) struct ReadOnlySqliteArticleRepository {
    database: SqlitePool,
}

impl ReadOnlySqliteArticleRepository {
    /// 校验显式应用数据目录并以只读、禁止创建模式打开其中的工作区数据库。
    pub(crate) async fn open(app_data_dir: &Path) -> Result<Self, ArticleReadError> {
        if !app_data_dir.is_absolute() {
            return Err(ArticleReadError::WorkspacePathInvalid);
        }

        let directory_metadata = metadata_or_workspace_error(app_data_dir, true).await?;
        if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
            return Err(ArticleReadError::WorkspacePathInvalid);
        }

        let database_path = app_data_dir.join(DATABASE_FILE_NAME);
        let database_metadata = metadata_or_workspace_error(&database_path, false).await?;
        if database_metadata.file_type().is_symlink() || !database_metadata.is_file() {
            return Err(ArticleReadError::WorkspacePathInvalid);
        }

        let canonical_directory = tokio::fs::canonicalize(app_data_dir)
            .await
            .map_err(|_| ArticleReadError::WorkspaceUnavailable)?;
        let canonical_database = tokio::fs::canonicalize(&database_path)
            .await
            .map_err(|_| ArticleReadError::WorkspaceUnavailable)?;
        if canonical_database.parent() != Some(canonical_directory.as_path()) {
            return Err(ArticleReadError::WorkspacePathInvalid);
        }

        let options = SqliteConnectOptions::new()
            .filename(canonical_database)
            .read_only(true)
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(map_sqlite_read_error)?;

        Ok(Self { database: pool })
    }

    async fn load_snapshot(&self) -> Result<ArticleWorkspaceSnapshot, ArticleReadError> {
        let row =
            sqlx::query("SELECT schema_version, payload FROM workspace_snapshot WHERE id = 1")
                .fetch_optional(&self.database)
                .await
                .map_err(map_sqlite_read_error)?
                .ok_or(ArticleReadError::WorkspaceNotFound)?;
        let stored_schema: i64 = row
            .try_get("schema_version")
            .map_err(|_| ArticleReadError::WorkspaceInvalid)?;
        let payload: String = row
            .try_get("payload")
            .map_err(|_| ArticleReadError::WorkspaceInvalid)?;
        let stored_schema =
            u32::try_from(stored_schema).map_err(|_| ArticleReadError::WorkspaceInvalid)?;
        if stored_schema != CURRENT_WORKSPACE_SCHEMA_VERSION {
            return Err(ArticleReadError::WorkspaceSchemaUnsupported);
        }
        let snapshot = serde_json::from_str::<ArticleWorkspaceSnapshot>(&payload)
            .map_err(|_| ArticleReadError::WorkspaceInvalid)?;
        if stored_schema != snapshot.schema_version {
            return Err(ArticleReadError::WorkspaceInvalid);
        }
        Ok(snapshot)
    }
}

fn map_sqlite_read_error(error: SqlxError) -> ArticleReadError {
    match error {
        SqlxError::Database(database_error) => {
            let primary_code = database_error
                .code()
                .and_then(|code| code.parse::<i32>().ok())
                .map(|code| code & 0xff);
            // SQLite: PERM、BUSY、LOCKED、READONLY、IOERR、CANTOPEN、PROTOCOL、AUTH。
            if matches!(primary_code, Some(3 | 5 | 6 | 8 | 10 | 14 | 15 | 23)) {
                ArticleReadError::WorkspaceUnavailable
            } else {
                ArticleReadError::WorkspaceInvalid
            }
        }
        SqlxError::Io(_)
        | SqlxError::Tls(_)
        | SqlxError::PoolTimedOut
        | SqlxError::PoolClosed
        | SqlxError::WorkerCrashed => ArticleReadError::WorkspaceUnavailable,
        _ => ArticleReadError::WorkspaceInvalid,
    }
}

impl ArticleReadRepository for ReadOnlySqliteArticleRepository {
    fn load_articles(&self) -> ArticleReadFuture<'_> {
        Box::pin(async move { Ok(self.load_snapshot().await?.articles) })
    }
}

async fn metadata_or_workspace_error(
    path: &Path,
    is_directory: bool,
) -> Result<std::fs::Metadata, ArticleReadError> {
    tokio::fs::symlink_metadata(path).await.map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            ArticleReadError::WorkspaceNotFound
        } else if is_directory && error.kind() == ErrorKind::InvalidInput {
            ArticleReadError::WorkspacePathInvalid
        } else {
            ArticleReadError::WorkspaceUnavailable
        }
    })
}
