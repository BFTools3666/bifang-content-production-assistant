use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::{Duration, Instant},
};

use assert_cmd::Command;
use bifang_content_production_assistant_core::{
    ArticleTask, ArticleWorkspaceSnapshot, CURRENT_WORKSPACE_SCHEMA_VERSION,
};
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("bifang_content_production_assistant_cli").unwrap()
}

fn parse_single_json_document(output: &[u8]) -> serde_json::Value {
    assert!(!output.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert_eq!(output.last(), Some(&b'\n'));
    std::str::from_utf8(output).unwrap();
    serde_json::from_slice(output).unwrap()
}

fn assert_metadata(value: &serde_json::Value, command: &str) {
    assert_eq!(value["meta"]["schemaVersion"], "1");
    assert_eq!(value["meta"]["command"], command);
    let timestamp = value["meta"]["timestamp"].as_str().unwrap();
    assert!(timestamp.contains('T'));
    assert!(timestamp.ends_with('Z'));
}

fn isolated_app_data_dir() -> (TempDir, PathBuf) {
    let temporary_root = tempfile::tempdir().unwrap();
    let app_data_dir = temporary_root.path().join("workspace % 空格");
    fs::create_dir(&app_data_dir).unwrap();
    (temporary_root, app_data_dir)
}

async fn seed_workspace(
    app_data_dir: &Path,
    stored_schema_version: u32,
    payload_schema_version: u32,
    articles: Vec<ArticleTask>,
) -> PathBuf {
    let database_path = app_data_dir.join("content-workbench.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    sea_orm::sqlx::query(
        "CREATE TABLE workspace_snapshot (\
            id INTEGER PRIMARY KEY CHECK (id = 1), \
            schema_version INTEGER NOT NULL, \
            payload TEXT NOT NULL, \
            updated_at_ms INTEGER NOT NULL\
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    let payload = serde_json::to_string(&ArticleWorkspaceSnapshot {
        schema_version: payload_schema_version,
        articles,
    })
    .unwrap();
    sea_orm::sqlx::query(
        "INSERT INTO workspace_snapshot (id, schema_version, payload, updated_at_ms) \
         VALUES (1, ?, ?, 1)",
    )
    .bind(i64::from(stored_schema_version))
    .bind(payload)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    database_path
}

fn directory_entries(path: &Path) -> Vec<OsString> {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[tokio::test]
async fn lists_articles_in_product_order_without_mutating_workspace() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    let mut oldest = ArticleTask::new("article-old", "旧文章", 10);
    oldest.created_at_ms = 10;
    let mut same_time_a = ArticleTask::new("article-a", "同刻 A", 30);
    same_time_a.created_at_ms = 30;
    let mut same_time_z = ArticleTask::new("article-z", "同刻 Z", 30);
    same_time_z.created_at_ms = 30;
    let database_path = seed_workspace(
        &app_data_dir,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        vec![oldest, same_time_a, same_time_z],
    )
    .await;
    let bytes_before = fs::read(&database_path).unwrap();
    let entries_before = directory_entries(&app_data_dir);

    let output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["total"], 3);
    assert_eq!(
        value["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["article-z", "article-a", "article-old"]
    );
    assert_eq!(value["error"], serde_json::Value::Null);
    assert_metadata(&value, "article.list");
    assert_eq!(fs::read(database_path).unwrap(), bytes_before);
    assert_eq!(directory_entries(&app_data_dir), entries_before);
}

#[tokio::test]
async fn gets_safe_article_detail_without_local_image_references() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    let mut article = ArticleTask::new("Article-Exact", "文章详情", 30);
    article.keywords = vec!["关键词".into()];
    article.requirements = "保持准确".into();
    article.artifacts.markdown = Some("# 正文".into());
    article.artifacts.summary = Some("简介".into());
    article.artifacts.html = Some("<h1>正文</h1>".into());
    article.artifacts.image_ref = Some("/private/current.png".into());
    article.artifacts.original_image_ref = Some("/private/original.png".into());
    article.artifacts.image_edit_revision = 4;
    seed_workspace(
        &app_data_dir,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        vec![article],
    )
    .await;

    let output = cmd()
        .args(["article", "get", "--app-data-dir"])
        .arg(&app_data_dir)
        .args(["--id", "Article-Exact", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["data"]["article"]["id"], "Article-Exact");
    assert_eq!(value["data"]["artifacts"]["markdown"], "# 正文");
    assert_eq!(value["data"]["artifacts"]["hasImage"], true);
    assert_eq!(value["data"]["artifacts"]["hasOriginalImage"], true);
    assert_eq!(value["data"]["artifacts"]["imageEditRevision"], 4);
    let serialized = String::from_utf8(output.stdout).unwrap();
    assert!(!serialized.contains("/private/"));
    assert!(!serialized.contains("imageRef"));
    assert!(!serialized.contains("originalImageRef"));
    assert_metadata(&value, "article.get");
}

#[tokio::test]
async fn renders_article_list_and_detail_for_humans() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    let mut article = ArticleTask::new("article-human", "人类输出", 30);
    article.artifacts.markdown = Some("可读正文".into());
    article.artifacts.image_ref = Some("/private/never-print.png".into());
    seed_workspace(
        &app_data_dir,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        vec![article],
    )
    .await;

    let list_output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .output()
        .unwrap();
    assert!(list_output.status.success());
    assert!(list_output.stderr.is_empty());
    let list_stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(list_stdout.contains("共 1 篇文章"));
    assert!(list_stdout.contains("article-human | 人类输出"));

    let detail_output = cmd()
        .args(["article", "get", "--app-data-dir"])
        .arg(&app_data_dir)
        .args(["--id", "article-human"])
        .output()
        .unwrap();
    assert!(detail_output.status.success());
    assert!(detail_output.stderr.is_empty());
    let detail_stdout = String::from_utf8(detail_output.stdout).unwrap();
    assert!(detail_stdout.contains("Markdown：可读正文"));
    assert!(detail_stdout.contains("配图：有"));
    assert!(!detail_stdout.contains("/private/"));
}

#[tokio::test]
async fn reports_exact_missing_article_with_exit_three() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    seed_workspace(
        &app_data_dir,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        vec![ArticleTask::new("Case-Sensitive", "文章", 1)],
    )
    .await;

    let output = cmd()
        .args(["article", "get", "--app-data-dir"])
        .arg(&app_data_dir)
        .args(["--id", "case-sensitive", "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "ARTICLE_NOT_FOUND");
    assert_metadata(&value, "article.get");
}

#[test]
fn missing_database_is_not_created() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    let database_path = app_data_dir.join("content-workbench.sqlite3");

    let output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .arg("--json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    assert!(!database_path.exists());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "WORKSPACE_NOT_FOUND");
    assert_metadata(&value, "article.list");
}

#[test]
fn rejects_relative_app_data_directory_as_invalid_argument() {
    let output = cmd()
        .args([
            "article",
            "list",
            "--app-data-dir",
            "relative-workspace",
            "--json",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "WORKSPACE_PATH_INVALID");
    assert_metadata(&value, "article.list");
}

#[test]
fn rejects_blank_article_id_before_accessing_the_workspace() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    let database_path = app_data_dir.join("content-workbench.sqlite3");

    let output = cmd()
        .args(["article", "get", "--app-data-dir"])
        .arg(&app_data_dir)
        .args(["--id", "  ", "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    assert!(!database_path.exists());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "ARTICLE_ID_INVALID");
    assert_metadata(&value, "article.get");
}

#[tokio::test]
async fn reports_existing_database_without_snapshot_table_as_invalid() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    let database_path = app_data_dir.join("content-workbench.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    pool.close().await;

    let output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .arg("--json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(10));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "WORKSPACE_INVALID");
    assert_metadata(&value, "article.list");
}

#[test]
fn reports_non_sqlite_database_file_as_invalid() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    fs::write(
        app_data_dir.join("content-workbench.sqlite3"),
        b"not a sqlite database",
    )
    .unwrap();

    let output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .arg("--json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(10));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "WORKSPACE_INVALID");
    assert_metadata(&value, "article.list");
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_app_data_directory() {
    use std::os::unix::fs::symlink;

    let temporary_root = tempfile::tempdir().unwrap();
    let real_directory = temporary_root.path().join("real-workspace");
    let linked_directory = temporary_root.path().join("linked-workspace");
    fs::create_dir(&real_directory).unwrap();
    symlink(&real_directory, &linked_directory).unwrap();

    let output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&linked_directory)
        .arg("--json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "WORKSPACE_PATH_INVALID");
    assert_metadata(&value, "article.list");
}

#[tokio::test]
async fn rejects_non_current_workspace_schema() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    seed_workspace(&app_data_dir, 10, 10, Vec::new()).await;

    let output = cmd()
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .arg("--json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(10));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["error"]["code"], "WORKSPACE_SCHEMA_UNSUPPORTED");
    assert_metadata(&value, "article.list");
}

#[tokio::test]
async fn article_list_does_not_wait_for_open_stdin() {
    let (_temporary_root, app_data_dir) = isolated_app_data_dir();
    seed_workspace(
        &app_data_dir,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        CURRENT_WORKSPACE_SCHEMA_VERSION,
        Vec::new(),
    )
    .await;
    let executable = env!("CARGO_BIN_EXE_bifang_content_production_assistant_cli");
    let mut child = ProcessCommand::new(executable)
        .args(["article", "list", "--app-data-dir"])
        .arg(&app_data_dir)
        .arg("--json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _open_stdin = child.stdin.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            let value = parse_single_json_document(&output.stdout);
            assert_eq!(value["data"]["total"], 0);
            assert_metadata(&value, "article.list");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("article list waited for stdin for more than five seconds");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
