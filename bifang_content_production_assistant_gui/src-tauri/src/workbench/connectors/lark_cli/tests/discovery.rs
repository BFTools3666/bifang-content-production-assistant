//! CLI 缺失、安装后重发现和相对路径攻击的回归测试。

use super::*;

/// 验证 CLI 缺失错误不暴露尝试路径。
#[tokio::test]
async fn reports_a_missing_cli_without_exposing_the_attempted_path() {
    let directory = tempfile::tempdir().unwrap();
    let client = LarkCliClient::for_test(directory.path().join("missing-lark-cli"));

    let error = client.ensure_supported_version(2_000).await.unwrap_err();

    assert_eq!(error.code, "CONNECTOR_CLI_NOT_INSTALLED");
    assert!(
        !error
            .message
            .contains(directory.path().to_string_lossy().as_ref())
    );
}

#[cfg(unix)]
mod unix {
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    use super::*;

    const TEST_OPERATION_TIMEOUT_MS: u64 = 10_000;

    /// 验证应用启动后安装 CLI 无需重启即可重新发现。
    #[tokio::test]
    async fn client_rediscovers_a_cli_installed_after_application_start() {
        let home = tempfile::tempdir().unwrap();
        let executable = home.path().join(".local/bin/lark-cli");
        let discovery_candidate = executable.clone();
        let client = LarkCliClient::for_test_discovery(move || {
            resolve_lark_cli_candidates([discovery_candidate.clone()])
        });

        let first_error = client
            .inspect_setup("", TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap_err();
        assert_eq!(first_error.code, "CONNECTOR_CLI_NOT_INSTALLED");

        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(
            &executable,
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'lark-cli version 1.0.96\n'
elif [ "$1" = "profile" ] && [ "$2" = "list" ]; then
  printf '[]\n'
else
  exit 9
fi
"##,
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();

        let inspection = client
            .inspect_setup("", TEST_OPERATION_TIMEOUT_MS)
            .await
            .unwrap();
        assert_eq!(inspection.version, "1.0.96");
        assert!(inspection.profiles.is_empty());
    }

    /// 验证相对发现结果不会执行当前目录恶意同名程序。
    #[tokio::test]
    async fn relative_discovery_never_executes_a_current_directory_program() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("lark-cli");
        let executed_marker = directory.path().join("executed");
        fs::write(
            &executable,
            format!("#!/bin/sh\n: > '{}'\n", executed_marker.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).unwrap();
        let client = LarkCliClient::for_test_discovery(|| {
            resolve_lark_cli_candidates([PathBuf::from("lark-cli")])
        });

        let error = client
            .run_process(
                &["--version"],
                None,
                Some(directory.path()),
                TEST_OPERATION_TIMEOUT_MS,
                OperationRisk::Read,
            )
            .await
            .unwrap_err();

        assert_eq!(error.code, "CONNECTOR_CLI_NOT_INSTALLED");
        assert!(!executed_marker.exists());
    }
}
