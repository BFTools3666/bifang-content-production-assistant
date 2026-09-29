//! 安全可执行文件发现、受控子进程和有界标准流。

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::ExitStatus,
    time::Duration,
};

#[cfg(test)]
use std::sync::Arc;

use serde_json::Value;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{ChildStdin, Command},
    time::timeout,
};

use super::protocol::{LarkCliError, map_cli_error, parse_json_envelope, unverifiable_result};

const MAX_CLI_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 区分只读调用与结果可能不确定的写入调用。
pub(in crate::workbench::connectors) enum OperationRisk {
    Read,
    Write,
}

#[derive(Debug)]
/// 保存受控子进程的退出状态、有限输出和标准输入写入结果。
pub(in crate::workbench::connectors) struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub input_written: bool,
}

#[derive(Debug)]
/// 记录按字节上限读取的输出以及溢出或读取失败状态。
pub(in crate::workbench::connectors) struct LimitedOutput {
    pub bytes: Vec<u8>,
    pub overflowed: bool,
    pub read_failed: bool,
}

#[derive(Clone)]
/// 描述生产环境动态发现和测试环境显式注入的可执行文件来源。
enum LarkCliExecutable {
    DiscoverEachCall,
    #[cfg(test)]
    Fixed(PathBuf),
    #[cfg(test)]
    Resolver(Arc<dyn Fn() -> Result<PathBuf, LarkCliError> + Send + Sync>),
}

/// 每次真实调用前重新发现官方 CLI，避免应用启动时的缺失状态被永久缓存。
#[derive(Clone)]
pub(in crate::workbench::connectors) struct LarkCliClient {
    executable: LarkCliExecutable,
}

impl LarkCliClient {
    /// 建立每次执行前重新发现官方 CLI 的生产客户端。
    pub(in crate::workbench::connectors) fn new() -> Self {
        Self {
            executable: LarkCliExecutable::DiscoverEachCall,
        }
    }

    #[cfg(test)]
    /// 建立固定使用隔离假 CLI 路径的测试客户端。
    pub(in crate::workbench::connectors) fn for_test(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: LarkCliExecutable::Fixed(executable.into()),
        }
    }

    #[cfg(test)]
    /// 建立由隔离解析器控制每次发现结果的测试客户端。
    pub(in crate::workbench::connectors) fn for_test_discovery(
        resolver: impl Fn() -> Result<PathBuf, LarkCliError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            executable: LarkCliExecutable::Resolver(Arc::new(resolver)),
        }
    }

    /// 解析本次调用允许执行的绝对 CLI 路径。

    fn executable(&self) -> Result<PathBuf, LarkCliError> {
        match &self.executable {
            LarkCliExecutable::DiscoverEachCall => discover_lark_cli_executable(),
            #[cfg(test)]
            LarkCliExecutable::Fixed(path) => Ok(path.clone()),
            #[cfg(test)]
            LarkCliExecutable::Resolver(resolver) => resolver(),
        }
    }

    /// 执行 JSON 协议命令并按身份和风险要求验证响应信封。

    pub(in crate::workbench::connectors) async fn execute_json(
        &self,
        arguments: Vec<String>,
        stdin: Option<Vec<u8>>,
        current_dir: Option<&Path>,
        timeout_ms: u64,
        risk: OperationRisk,
        require_user_identity: bool,
    ) -> Result<Value, LarkCliError> {
        self.execute_json_with_fence(
            arguments,
            stdin,
            current_dir,
            timeout_ms,
            risk,
            require_user_identity,
            (),
        )
        .await
    }

    /// 最终状态栅栏保留到写子进程启动完成，不覆盖后续远端等待。
    #[allow(clippy::too_many_arguments)]
    pub(in crate::workbench::connectors) async fn execute_json_with_fence<G: Send>(
        &self,
        arguments: Vec<String>,
        stdin: Option<Vec<u8>>,
        current_dir: Option<&Path>,
        timeout_ms: u64,
        risk: OperationRisk,
        require_user_identity: bool,
        fence: G,
    ) -> Result<Value, LarkCliError> {
        let output = self
            .run_process_with_fence(&arguments, stdin, current_dir, timeout_ms, risk, fence)
            .await?;
        if output.status.code() == Some(10) {
            return Err(LarkCliError::new(
                "CONNECTOR_CLI_CONFIRMATION_REQUIRED",
                "飞书 CLI 的风险确认契约已变化，应用不会自动绕过确认",
            ));
        }
        if !output.status.success() {
            let Some(envelope) = parse_json_envelope(&output.stderr) else {
                return Err(unverifiable_result(risk));
            };
            return Err(
                if envelope.get("ok").and_then(Value::as_bool) == Some(false) {
                    map_cli_error(&envelope, risk)
                } else {
                    unverifiable_result(risk)
                },
            );
        }
        if !output.input_written {
            return Err(unverifiable_result(risk));
        }
        let Some(envelope) = parse_json_envelope(&output.stdout) else {
            return Err(unverifiable_result(risk));
        };
        match envelope.get("ok").and_then(Value::as_bool) {
            Some(true)
                if !require_user_identity
                    || envelope
                        .get("identity")
                        .or_else(|| envelope.pointer("/data/identity"))
                        .and_then(Value::as_str)
                        == Some("user") =>
            {
                Ok(envelope)
            }
            Some(true) => Err(unverifiable_result(risk)),
            Some(false) => Err(map_cli_error(&envelope, risk)),
            None => Err(unverifiable_result(risk)),
        }
    }

    /// 运行有界 CLI 子进程。

    pub(in crate::workbench::connectors) async fn run_process<S: AsRef<OsStr>>(
        &self,
        arguments: &[S],
        stdin: Option<Vec<u8>>,
        current_dir: Option<&Path>,
        timeout_ms: u64,
        risk: OperationRisk,
    ) -> Result<ProcessOutput, LarkCliError> {
        self.run_process_with_fence(arguments, stdin, current_dir, timeout_ms, risk, ())
            .await
    }

    async fn run_process_with_fence<S: AsRef<OsStr>, G: Send>(
        &self,
        arguments: &[S],
        stdin: Option<Vec<u8>>,
        current_dir: Option<&Path>,
        timeout_ms: u64,
        risk: OperationRisk,
        fence: G,
    ) -> Result<ProcessOutput, LarkCliError> {
        let mut command = Command::new(self.executable()?);
        command
            .args(arguments)
            .env("LARKSUITE_CLI_NO_UPDATE_NOTIFIER", "1")
            .env("LARKSUITE_CLI_NO_SKILLS_NOTIFIER", "1")
            .kill_on_drop(true)
            .stdin(if stdin.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(directory) = current_dir {
            command.current_dir(directory);
        }
        let mut child = command.spawn().map_err(map_spawn_error)?;
        drop(fence);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| unverifiable_result(risk))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| unverifiable_result(risk))?;
        let stdin_pipe = child.stdin.take();

        let completed = timeout(Duration::from_millis(timeout_ms), async {
            tokio::join!(
                child.wait(),
                read_limited(stdout),
                read_limited(stderr),
                write_stdin(stdin_pipe, stdin),
            )
        })
        .await;
        let (status, stdout, stderr, input_written) = match completed {
            Ok(result) => result,
            Err(_) => {
                let _ = child.start_kill();
                let _ = timeout(Duration::from_secs(2), child.wait()).await;
                return Err(match risk {
                    OperationRisk::Read => {
                        LarkCliError::new("CONNECTOR_TIMEOUT", "飞书 CLI 调用超时")
                    }
                    OperationRisk::Write => LarkCliError::new(
                        "CONNECTOR_SYNC_OUTCOME_UNKNOWN",
                        "发布命令已经开始，但结果无法确认；请先在飞书检查文件",
                    ),
                });
            }
        };
        let status = status.map_err(|_| unverifiable_result(risk))?;
        if stdout.overflowed || stderr.overflowed || stdout.read_failed || stderr.read_failed {
            return Err(unverifiable_result(risk));
        }
        Ok(ProcessOutput {
            status,
            stdout: stdout.bytes,
            stderr: stderr.bytes,
            input_written,
        })
    }
}

/// 把子进程启动失败映射为不泄露路径的稳定错误。

fn map_spawn_error(error: std::io::Error) -> LarkCliError {
    if error.kind() == std::io::ErrorKind::NotFound {
        LarkCliError::new(
            "CONNECTOR_CLI_NOT_INSTALLED",
            "未找到飞书 CLI，请先安装 lark-cli",
        )
    } else {
        LarkCliError::new("CONNECTOR_CLI_EXECUTION_FAILED", "无法启动飞书 CLI")
    }
}

/// 向子进程完整写入可选内容并关闭标准输入。

async fn write_stdin(mut writer: Option<ChildStdin>, input: Option<Vec<u8>>) -> bool {
    let Some(writer) = writer.as_mut() else {
        return input.is_none();
    };
    let written = if let Some(bytes) = input {
        writer.write_all(&bytes).await.is_ok()
    } else {
        true
    };
    written && writer.shutdown().await.is_ok()
}

/// 只接受明确的绝对候选；无候选时绝不退回裸命令名或当前工作目录。
fn discover_lark_cli_executable() -> Result<PathBuf, LarkCliError> {
    let executable_name = if cfg!(windows) {
        "lark-cli.exe"
    } else {
        "lark-cli"
    };
    let mut candidates = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(executable_name))
        .collect::<Vec<_>>();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        candidates.push(home.join(".local/bin").join(executable_name));
    }
    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/opt/homebrew/bin").join(executable_name));
        candidates.push(PathBuf::from("/usr/local/bin").join(executable_name));
    }
    if cfg!(windows)
        && let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from)
    {
        candidates.push(
            app_data
                .join("npm/node_modules/@larksuite/cli/bin")
                .join(executable_name),
        );
    }
    resolve_lark_cli_candidates(candidates)
}

/// 从绝对候选中选择可执行文件，无合法候选时稳定拒绝。

pub(in crate::workbench::connectors) fn resolve_lark_cli_candidates(
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<PathBuf, LarkCliError> {
    for candidate in candidates {
        if !candidate.is_absolute() {
            continue;
        }
        let Ok(resolved) = std::fs::canonicalize(&candidate) else {
            continue;
        };
        if !resolved.is_absolute() || !resolved.is_file() {
            continue;
        }
        if !cfg!(windows)
            && resolved.file_name() == Some(OsStr::new("run.js"))
            && let Some(script_directory) = resolved.parent()
        {
            let native = script_directory.join("../bin/lark-cli");
            if let Ok(native) = std::fs::canonicalize(native)
                && native.is_absolute()
                && native.is_file()
            {
                return Ok(native);
            }
            continue;
        }
        return Ok(resolved);
    }
    Err(LarkCliError::new(
        "CONNECTOR_CLI_NOT_INSTALLED",
        "未找到飞书 CLI，请先安装 lark-cli",
    ))
}

/// 读取标准流并在固定字节上限处标记溢出。

async fn read_limited<R>(mut reader: R) -> LimitedOutput
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut overflowed = false;
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(read) => {
                let remaining = MAX_CLI_OUTPUT_BYTES.saturating_sub(bytes.len());
                let retained = remaining.min(read);
                bytes.extend_from_slice(&buffer[..retained]);
                overflowed |= retained < read;
            }
            Err(_) => {
                return LimitedOutput {
                    bytes,
                    overflowed,
                    read_failed: true,
                };
            }
        }
    }
    LimitedOutput {
        bytes,
        overflowed,
        read_failed: false,
    }
}
