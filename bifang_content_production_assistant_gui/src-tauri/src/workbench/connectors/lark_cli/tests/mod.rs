//! 飞书 CLI 适配器回归测试。

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;

mod discovery;

#[test]
/// 验证最低兼容版本、更新版本和异常版本文本的解析。
fn parses_minimum_and_newer_cli_versions() {
    assert_eq!(
        parse_cli_version(b"lark-cli version 1.0.96\n"),
        Some((1, 0, 96))
    );
    assert_eq!(parse_cli_version(b"lark-cli 2.4.1"), Some((2, 4, 1)));
    assert_eq!(parse_cli_version(b"unexpected"), None);
}

#[test]
/// 验证结构化错误映射不会泄露远端原始说明。
fn maps_structured_cli_errors_without_exposing_remote_text() {
    let error = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": {
                "type": "config",
                "subtype": "not_configured",
                "message": "secret remote detail"
            }
        }),
        OperationRisk::Read,
    );
    assert_eq!(error.code, "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED");
    assert!(!error.message.contains("secret remote detail"));
}

#[test]
/// 验证权限不足与用户登录失效得到不同稳定错误。
fn distinguishes_missing_scope_from_missing_login() {
    let permission = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": { "type": "authorization", "subtype": "missing_scope" }
        }),
        OperationRisk::Read,
    );
    let authentication = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": { "type": "authentication", "subtype": "token_expired" }
        }),
        OperationRisk::Read,
    );

    assert_eq!(permission.code, "CONNECTOR_PERMISSION");
    assert_eq!(authentication.code, "CONNECTOR_CLI_AUTH_REQUIRED");
}

#[test]
/// 验证写入传输失败被判为结果未知。
fn treats_a_write_transport_error_as_an_unknown_outcome() {
    let error = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": { "type": "network", "subtype": "transport_timeout" }
        }),
        OperationRisk::Write,
    );

    assert_eq!(error.code, "CONNECTOR_SYNC_OUTCOME_UNKNOWN");
}

#[test]
/// 验证写入内部响应异常被判为结果未知。
fn treats_an_internal_write_response_error_as_an_unknown_outcome() {
    let error = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": { "type": "internal", "subtype": "invalid_response" }
        }),
        OperationRisk::Write,
    );

    assert_eq!(error.code, "CONNECTOR_SYNC_OUTCOME_UNKNOWN");
}

#[test]
/// 验证只读网络失败和超时使用对应稳定错误。
fn gives_read_network_failures_a_relevant_stable_code() {
    let network = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": { "type": "network", "subtype": "connection_reset" }
        }),
        OperationRisk::Read,
    );
    let timeout = map_cli_error(
        &serde_json::json!({
            "ok": false,
            "error": { "type": "network", "subtype": "timeout" }
        }),
        OperationRisk::Read,
    );

    assert_eq!(network.code, "CONNECTOR_NETWORK_FAILED");
    assert_eq!(timeout.code, "CONNECTOR_TIMEOUT");
}

#[test]
/// 验证仅从成功信封接受非空文件 token。
fn extracts_only_a_non_empty_file_token() {
    assert_eq!(
        extract_file_token(&serde_json::json!({
            "ok": true,
            "data": { "file_token": "file_123" }
        }))
        .unwrap(),
        LarkCliUploadResult {
            file_token: "file_123".into()
        }
    );
    assert_eq!(
        extract_file_token(&serde_json::json!({"ok": true}))
            .unwrap_err()
            .code,
        "CONNECTOR_SYNC_OUTCOME_UNKNOWN"
    );
}

#[test]
/// 验证 Profile 名称拒绝空白、路径和命令元字符。
fn profile_names_follow_the_official_cli_safety_contract() {
    assert!(validate_profile_name("seo-publisher").is_ok());
    assert!(validate_profile_name("内容发布").is_ok());
    for invalid in [
        "",
        " leading",
        "trailing ",
        "with space",
        "prod;rm",
        "../escape",
        "name$HOME",
        "name#tag",
        "name~tag",
    ] {
        assert_eq!(
            validate_profile_name(invalid).unwrap_err().code,
            "CONNECTOR_CLI_PROFILE_INVALID"
        );
    }
}

#[cfg(unix)]
mod unix {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::{
            Arc, Condvar, Mutex as StdMutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use super::*;

    const TEST_OPERATION_TIMEOUT_MS: u64 = 10_000;

    /// 建立具有可执行权限的隔离 Unix 假 CLI。

    fn fake_cli(script: &str) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("fake-lark-cli");
        fs::write(&executable, script).unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();
        (directory, executable)
    }

    #[tokio::test]
    async fn folder_inspection_uses_the_supported_v1_params_contract() {
        let capture_directory = tempfile::tempdir().unwrap();
        let captured_arguments = capture_directory.path().join("folder-list-arguments");
        let script = format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{{"ok":true,"identity":"user"}}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '%s\n' "$@" > '{}'
  printf '{{"ok":true,"identity":"user","data":{{"files":[]}}}}\n'
else
  exit 8
fi
"##,
            captured_arguments.display()
        );
        let (_directory, executable) = fake_cli(&script);
        let client = LarkCliClient::for_test(executable);

        client
            .verify_folder("team-profile", "fld_target", TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap();
        let arguments = fs::read_to_string(captured_arguments).unwrap();
        assert_eq!(
            arguments.lines().collect::<Vec<_>>(),
            [
                "--profile",
                "team-profile",
                "drive",
                "files",
                "list",
                "--params",
                r#"{"folder_token":"fld_target","page_size":1}"#,
                "--as",
                "user",
                "--format",
                "json",
            ]
        );
    }

    #[tokio::test]
    async fn final_workspace_check_runs_after_folder_preflight_and_before_either_upload() {
        let capture_directory = tempfile::tempdir().unwrap();
        let preflight_marker = capture_directory.path().join("folder-preflight-complete");
        let script = format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{{"ok":true,"identity":"user"}}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  : > '{}'
  printf '{{"ok":true,"identity":"user","data":{{"files":[]}}}}\n'
else
  exit 9
fi
"##,
            preflight_marker.display()
        );
        let (_directory, executable) = fake_cli(&script);
        let client = LarkCliClient::for_test(executable);

        let markdown_error = client
            .create_markdown_with_check(
                "team-profile",
                "fld_target",
                "article.md",
                b"# article".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
                || async { Err::<(), _>(LarkCliError::new("STALE_WORKSPACE", "版本已变化")) },
            )
            .await
            .unwrap_err();
        assert_eq!(markdown_error.code, "STALE_WORKSPACE");
        assert!(preflight_marker.is_file());

        fs::remove_file(&preflight_marker).unwrap();
        let html_error = client
            .upload_html_with_check(
                "team-profile",
                "fld_target",
                "article.html",
                b"<h1>article</h1>".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
                || async { Err::<(), _>(LarkCliError::new("STALE_WORKSPACE", "版本已变化")) },
            )
            .await
            .unwrap_err();
        assert_eq!(html_error.code, "STALE_WORKSPACE");
        assert!(preflight_marker.is_file());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn write_fence_blocks_edits_during_final_cli_discovery() {
        let script = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{"ok":true,"identity":"user"}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '{"ok":true,"identity":"user","data":{"items":[]}}\n'
else
  cat >/dev/null
  printf '{"ok":true,"identity":"user","data":{"file_token":"uploaded"}}\n'
fi
"##;
        let (_directory, executable) = fake_cli(script);
        let discovery_count = Arc::new(AtomicUsize::new(0));
        let discovery_gate = Arc::new((StdMutex::new(false), Condvar::new()));
        let (at_write_tx, at_write_rx) = tokio::sync::oneshot::channel();
        let at_write_tx = Arc::new(StdMutex::new(Some(at_write_tx)));
        let client = LarkCliClient::for_test_discovery({
            let discovery_count = Arc::clone(&discovery_count);
            let discovery_gate = Arc::clone(&discovery_gate);
            let at_write_tx = Arc::clone(&at_write_tx);
            move || {
                if discovery_count.fetch_add(1, Ordering::SeqCst) == 3 {
                    if let Some(sender) = at_write_tx.lock().unwrap().take() {
                        let _ = sender.send(());
                    }
                    let (lock, cvar) = &*discovery_gate;
                    let mut released = lock.lock().unwrap();
                    while !*released {
                        released = cvar.wait(released).unwrap();
                    }
                }
                Ok(executable.clone())
            }
        });
        let workspace_lock = Arc::new(tokio::sync::Mutex::new(()));
        let upload = tokio::spawn({
            let workspace_lock = Arc::clone(&workspace_lock);
            async move {
                client
                    .create_markdown_with_check(
                        "team-profile",
                        "fld_target",
                        "article.md",
                        b"# article".to_vec(),
                        TEST_OPERATION_TIMEOUT_MS,
                        || async { Ok::<_, LarkCliError>(workspace_lock.lock_owned().await) },
                    )
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(5), at_write_rx)
            .await
            .unwrap()
            .unwrap();
        let mut edit = tokio::spawn({
            let workspace_lock = Arc::clone(&workspace_lock);
            async move {
                let _guard = workspace_lock.lock_owned().await;
            }
        });
        let edit_was_blocked = tokio::time::timeout(Duration::from_millis(100), &mut edit)
            .await
            .is_err();
        let (lock, cvar) = &*discovery_gate;
        *lock.lock().unwrap() = true;
        cvar.notify_all();
        assert!(
            edit_was_blocked,
            "write fence must survive executable discovery"
        );
        assert_eq!(upload.await.unwrap().unwrap().file_token, "uploaded");
        edit.await.unwrap();
    }

    #[tokio::test]
    /// 验证 Markdown 内容只经标准输入和固定用户参数发送。
    async fn markdown_content_uses_stdin_and_fixed_user_arguments() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{"ok":true,"identity":"user"}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '{"ok":true,"identity":"user","data":{"items":[]}}\n'
else
  body=$(cat)
  args=" $* "
  case "$args" in *" markdown +create "*) ;; *) exit 8 ;; esac
  case "$args" in *" --as user "*) ;; *) exit 8 ;; esac
  case "$args" in *" --content - "*) ;; *) exit 8 ;; esac
  [ "$body" = "# title" ] && printf '{"ok":true,"identity":"user","data":{"file_token":"markdown-file"}}\n' || exit 9
fi
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let result = client
            .create_markdown(
                "team-profile",
                "fld_target",
                "article-v1.md",
                b"# title".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
            )
            .await
            .unwrap();

        assert_eq!(result.file_token, "markdown-file");
    }

    #[tokio::test]
    /// 验证 HTML 使用受控相对文件并清理临时目录。
    async fn html_uses_a_relative_file_and_cleans_its_private_directory() {
        let capture_directory = tempfile::tempdir().unwrap();
        let captured_working_directory = capture_directory.path().join("working-directory");
        let script = format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{{"ok":true,"identity":"user"}}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '{{"ok":true,"identity":"user","data":{{"items":[]}}}}\n'
else
  args=" $* "
  case "$args" in *" --profile team-profile "*) ;; *) exit 8 ;; esac
  case "$args" in *" drive +upload "*) ;; *) exit 8 ;; esac
  case "$args" in *" --file document.html "*) ;; *) exit 8 ;; esac
  case "$args" in *" --name article-v2.html "*) ;; *) exit 8 ;; esac
  case "$args" in *" --folder-token fld_target "*) ;; *) exit 8 ;; esac
  case "$args" in *" --as user "*) ;; *) exit 8 ;; esac
  [ "$(cat document.html)" = '<h1>title</h1>' ] || exit 9
  printf '%s' "$PWD" > '{}'
  printf '{{"ok":true,"identity":"user","data":{{"file_token":"html-file"}}}}\n'
fi
"##,
            captured_working_directory.display()
        );
        let (_directory, executable) = fake_cli(&script);
        let client = LarkCliClient::for_test(executable);

        let result = client
            .upload_html(
                "team-profile",
                "fld_target",
                "article-v2.html",
                b"<h1>title</h1>".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
            )
            .await
            .unwrap();

        assert_eq!(result.file_token, "html-file");
        let temporary_path = fs::read_to_string(captured_working_directory).unwrap();
        assert!(!Path::new(&temporary_path).exists());
    }

    #[tokio::test]
    /// 验证 HTML 上传失败后仍清理临时目录。
    async fn html_failure_still_cleans_its_private_directory() {
        let capture_directory = tempfile::tempdir().unwrap();
        let captured_working_directory = capture_directory.path().join("failed-working-directory");
        let script = format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{{"ok":true,"identity":"user"}}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '{{"ok":true,"identity":"user","data":{{"items":[]}}}}\n'
else
  printf '%s' "$PWD" > '{}'
  printf '{{"ok":false,"error":{{"type":"permission","subtype":"forbidden"}}}}\n' >&2
  exit 3
fi
"##,
            captured_working_directory.display()
        );
        let (_directory, executable) = fake_cli(&script);
        let client = LarkCliClient::for_test(executable);

        let error = client
            .upload_html(
                "team-profile",
                "fld_target",
                "article-v2.html",
                b"<h1>title</h1>".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
            )
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_PERMISSION");
        let temporary_path = fs::read_to_string(captured_working_directory).unwrap();
        assert!(!Path::new(&temporary_path).exists());
    }

    #[tokio::test]
    /// 验证机器人身份不能冒充用户 Profile。
    async fn rejects_a_profile_that_is_not_a_user_identity() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
else
  printf '{"ok":true,"identity":"bot"}\n'
fi
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let error = client
            .verify_folder("team-profile", "fld_target", TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_CLI_IDENTITY_INVALID");
    }

    #[tokio::test]
    /// 验证未经兼容确认的下一主版本被拒绝。
    async fn rejects_an_unverified_next_major_cli_version() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
printf 'lark-cli version 2.0.0\n'
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let error = client
            .ensure_supported_version(TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_CLI_VERSION_UNSUPPORTED");
    }

    #[tokio::test]
    /// 验证文件夹预检失败时绝不启动写命令。
    async fn folder_validation_failure_never_starts_the_write_command() {
        let capture_directory = tempfile::tempdir().unwrap();
        let write_marker = capture_directory.path().join("write-started");
        let script = format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{{"ok":true,"identity":"user"}}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '{{"ok":false,"error":{{"type":"folder","subtype":"not_found"}}}}\n' >&2
  exit 3
else
  : > '{}'
  printf '{{"ok":true,"identity":"user","data":{{"file_token":"unexpected"}}}}\n'
fi
"##,
            write_marker.display()
        );
        let (_directory, executable) = fake_cli(&script);
        let client = LarkCliClient::for_test(executable);

        let error = client
            .create_markdown(
                "team-profile",
                "missing-folder",
                "article-v1.md",
                b"# title".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
            )
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_FOLDER_UNAVAILABLE");
        assert!(!write_marker.exists());
    }

    #[tokio::test]
    /// 验证 CLI 风险确认契约变化时不会自动绕过。
    async fn never_bypasses_a_changed_cli_confirmation_contract() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$@" | grep -q '^whoami$'; then
  printf '{"ok":true,"identity":"user"}\n'
elif printf '%s\n' "$@" | grep -q '^list$'; then
  printf '{"ok":true,"identity":"user","data":{"items":[]}}\n'
else
  exit 10
fi
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let error = client
            .create_markdown(
                "team-profile",
                "fld_target",
                "article-v1.md",
                b"# title".to_vec(),
                TEST_OPERATION_TIMEOUT_MS,
            )
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_CLI_CONFIRMATION_REQUIRED");
    }

    #[tokio::test]
    /// 验证写命令启动后超时被判为结果未知。
    async fn timeout_after_write_starts_is_outcome_unknown() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
sleep 2
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let error = client
            .run_process(&["write"], None, None, 100, OperationRisk::Write)
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_SYNC_OUTCOME_UNKNOWN");
    }

    #[tokio::test]
    /// 验证超过字节上限的 CLI 输出被拒绝。
    async fn rejects_oversized_cli_output() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  head -c 1048577 /dev/zero | tr '\0' 'x'
fi
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let error = client
            .ensure_supported_version(TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_CLI_INVALID_RESPONSE");
    }

    #[tokio::test]
    /// 只读授权检查仅把明确的 profile 缺失映射为未配置，并优先区分登录、权限和超时。
    async fn inspection_classifies_structured_profile_errors_precisely() {
        for (error_json, expected_code) in [
            (
                r#"{"error":{"type":"authentication","subtype":"profile_login_required","message":"private"}}"#,
                "CONNECTOR_CLI_AUTH_REQUIRED",
            ),
            (
                r#"{"error":{"type":"authorization","subtype":"profile_missing_scope"}}"#,
                "CONNECTOR_PERMISSION",
            ),
            (
                r#"{"error":{"type":"config","subtype":"not_configured"}}"#,
                "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED",
            ),
            (
                r#"{"error":{"type":"config","subtype":"profile_not_found"}}"#,
                "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED",
            ),
            (
                r#"{"error":{"type":"profile","subtype":"not_found"}}"#,
                "CONNECTOR_CLI_PROFILE_NOT_CONFIGURED",
            ),
            (
                r#"{"error":{"type":"network","subtype":"timeout"}}"#,
                "CONNECTOR_TIMEOUT",
            ),
            (
                r#"{"error":{"type":"profile","subtype":"diagnostics_failed"}}"#,
                "CONNECTOR_CLI_AUTH_STATUS_FAILED",
            ),
        ] {
            let script = format!(
                r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif [ "$1" = "profile" ] && [ "$2" = "list" ]; then
  printf '[{{"name":"seo-publisher"}}]\n'
elif [ "$3" = "auth" ] && [ "$4" = "status" ]; then
  printf '%s\n' '{}' >&2
  exit 12
else
  exit 33
fi
"##,
                error_json
            );
            let (_directory, executable) = fake_cli(&script);
            let client = LarkCliClient::for_test(executable);

            let snapshot = client
                .inspect_setup("seo-publisher", TEST_OPERATION_TIMEOUT_MS)
                .await
                .unwrap();

            assert!(snapshot.selected_profile_configured, "{error_json}");
            assert!(!snapshot.selected_profile_authorized, "{error_json}");
            assert_eq!(
                snapshot.authorization_issue_code,
                Some(expected_code),
                "{error_json}"
            );
        }
    }

    #[tokio::test]
    /// 验证检查结果仅返回安全 Profile 与用户状态字段。
    async fn inspection_returns_only_safe_profile_and_user_status_fields() {
        let (_directory, executable) = fake_cli(
            r##"#!/bin/sh
args=" $* "
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif printf '%s\n' "$args" | grep -q ' profile list '; then
  printf '[{"name":"seo-publisher","appId":"cli_private","brand":"feishu","user":"测试用户","tokenStatus":"valid","error":{"message":"private"}}]\n'
elif printf '%s\n' "$args" | grep -q ' auth status '; then
  printf '{"appId":"cli_private","identity":"user","verified":true,"identities":{"user":{"status":"ready","available":true,"verified":true,"userName":"测试用户","openId":"ou_private","tokenStatus":"valid","scope":"private"}}}\n'
else
  exit 9
fi
"##,
        );
        let client = LarkCliClient::for_test(executable);

        let snapshot = client
            .inspect_setup("seo-publisher", TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap();

        assert_eq!(snapshot.version, "1.0.96");
        assert_eq!(
            snapshot.profiles,
            vec![LarkCliProfileSummary {
                name: "seo-publisher".into(),
                user_name: Some("测试用户".into()),
                token_status: Some("valid".into()),
            }]
        );
        assert!(snapshot.selected_profile_configured);
        assert!(snapshot.selected_profile_authorized);
        assert_eq!(snapshot.selected_user_name.as_deref(), Some("测试用户"));
        assert_eq!(snapshot.authorization_issue_code, None);
    }

    #[tokio::test]
    /// 检查只运行版本、Profile 列表和用户状态命令，不触发创建或登录。
    async fn inspection_uses_only_read_only_cli_arguments() {
        let capture_directory = tempfile::tempdir().unwrap();
        let arguments_file = capture_directory.path().join("inspection-arguments");
        let script = format!(
            r##"#!/bin/sh
printf '%s\n' "$*" >> '{}'
if [ "$#" -eq 1 ] && [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif [ "$#" -eq 2 ] && [ "$1" = "profile" ] && [ "$2" = "list" ]; then
  printf '[{{"name":"seo-publisher","user":"测试用户","tokenStatus":"valid"}}]\n'
elif [ "$#" -eq 6 ] && [ "$1" = "--profile" ] && [ "$2" = "seo-publisher" ] && [ "$3" = "auth" ] && [ "$4" = "status" ] && [ "$5" = "--json" ] && [ "$6" = "--verify" ]; then
  printf '{{"identities":{{"user":{{"available":true,"verified":true,"userName":"测试用户"}}}}}}\n'
else
  exit 33
fi
"##,
            arguments_file.display()
        );
        let (_directory, executable) = fake_cli(&script);
        let client = LarkCliClient::for_test(executable);

        let snapshot = client
            .inspect_setup("seo-publisher", TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap();

        assert!(snapshot.selected_profile_authorized);
        assert_eq!(
            fs::read_to_string(arguments_file)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "--version",
                "profile list",
                "--profile seo-publisher auth status --json --verify",
            ]
        );
    }
}
