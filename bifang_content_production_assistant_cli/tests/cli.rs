use std::{
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::{Duration, Instant},
};

use assert_cmd::Command;

/// 返回指向真实编译产物的命令，避免每个用例重复拼装二进制名。
fn cmd() -> Command {
    Command::cargo_bin("bifang_content_production_assistant_cli").unwrap()
}

/// 校验 stdout 只包含一个无 BOM、以换行结尾且可解析的 JSON 文档。
fn parse_single_json_document(output: &[u8]) -> serde_json::Value {
    assert!(!output.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert_eq!(output.last(), Some(&b'\n'));
    std::str::from_utf8(output).unwrap();
    serde_json::from_slice(output).unwrap()
}

/// 校验每个机器响应都携带稳定模式、真实逻辑命令和 UTC 时间戳。
fn assert_metadata(value: &serde_json::Value, command: &str) {
    assert_eq!(value["meta"]["schemaVersion"], "1");
    assert_eq!(value["meta"]["command"], command);
    let timestamp = value["meta"]["timestamp"].as_str().unwrap();
    assert!(timestamp.contains('T'));
    assert!(timestamp.ends_with('Z'));
}

/// 校验显式 JSON 参数错误保持统一信封、干净 stderr 与退出码 2。
fn assert_json_argument_error(output: std::process::Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value["ok"].as_bool(), Some(false));
    assert_eq!(value["data"], serde_json::Value::Null);
    assert_eq!(value["error"]["code"], "INVALID_ARGUMENT");
    assert_metadata(&value, "cli.parse");
}

/// 验证真实二进制输出 core 定义的四阶段顺序、依赖、产物和审核门禁。
#[test]
fn describes_fixed_content_sop_as_json() {
    let output = cmd().args(["sop", "describe", "--json"]).output().unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value = parse_single_json_document(&output.stdout);
    assert_eq!(value.as_object().unwrap().len(), 4);
    assert_eq!(value["ok"].as_bool(), Some(true));
    assert_eq!(
        value["data"]["stages"],
        serde_json::json!([
            {
                "id": "article",
                "order": 1,
                "dependsOn": [],
                "output": {
                    "id": "markdownArticle",
                    "semantics": "Markdown 规范正文，是简介、配图和 HTML 的规范内容来源。"
                }
            },
            {
                "id": "summary",
                "order": 2,
                "dependsOn": ["article"],
                "output": {
                    "id": "articleSummary",
                    "semantics": "基于指定 Markdown 正文版本生成的文章简介。"
                }
            },
            {
                "id": "image",
                "order": 3,
                "dependsOn": ["article", "summary"],
                "output": {
                    "id": "articleImage",
                    "semantics": "基于标题、正文摘要和图片 Prompt 生成的一张配图。"
                }
            },
            {
                "id": "format",
                "order": 4,
                "dependsOn": ["article", "summary", "image"],
                "output": {
                    "id": "htmlDocument",
                    "semantics": "从规范 Markdown 正文派生的可复制 HTML；规范正文继续保留在同一文章包中。"
                }
            }
        ])
    );
    assert_eq!(
        value["data"]["reviewGate"],
        serde_json::json!({
            "requiredStages": ["article", "summary", "image", "format"],
            "requiresCompleteArtifacts": true
        })
    );
    assert_eq!(value["error"], serde_json::Value::Null);
    assert_metadata(&value, "sop.describe");
}

/// 验证父进程持续保持 stdin 管道打开时，SOP 查询仍在有界时间内退出。
#[test]
fn sop_describe_does_not_wait_for_open_stdin() {
    let executable = env!("CARGO_BIN_EXE_bifang_content_production_assistant_cli");
    let mut child = ProcessCommand::new(executable)
        .args(["sop", "describe", "--json"])
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
            assert_eq!(value["data"]["stages"].as_array().unwrap().len(), 4);
            assert_metadata(&value, "sop.describe");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("sop describe waited for stdin for more than five seconds");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// 验证旧脚手架命令已被删除，不能作为兼容别名继续调用。
#[test]
fn rejects_removed_scaffold_status_as_json() {
    assert_json_argument_error(
        cmd()
            .args(["scaffold", "status", "--json"])
            .output()
            .unwrap(),
    );
}

/// 验证未知业务命令不会被误当作可执行能力。
#[test]
fn rejects_unknown_commands_as_json() {
    assert_json_argument_error(cmd().args(["run", "--json"]).output().unwrap());
}

/// 验证缺少 `describe` 子命令时，显式 JSON 请求立即返回机器信封。
#[test]
fn reports_missing_describe_subcommand_as_json() {
    assert_json_argument_error(cmd().args(["sop", "--json"]).output().unwrap());
}

/// 验证产品命令包含非法参数时仍保持 stdout 单一 JSON、stderr 为空。
#[test]
fn reports_invalid_describe_argument_as_json() {
    assert_json_argument_error(
        cmd()
            .args(["sop", "describe", "--unknown", "--json"])
            .output()
            .unwrap(),
    );
}

/// 验证真实二进制从 workspace 版本事实来源暴露预期版本。
#[test]
fn reports_the_workspace_version() {
    let output = cmd().arg("--version").output().unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "bifang_content_production_assistant_cli v{}\n",
            env!("CARGO_PKG_VERSION")
        )
    );
}

/// 验证人类模式按 core 描述输出固定顺序、依赖、产物与审核门禁。
#[test]
fn describes_fixed_content_sop_for_humans() {
    let output = cmd().args(["sop", "describe"]).output().unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "固定内容 SOP（4 个阶段）\n",
            "1. article | 依赖：无 | 产物：markdownArticle — Markdown 规范正文，是简介、配图和 HTML 的规范内容来源。\n",
            "2. summary | 依赖：article | 产物：articleSummary — 基于指定 Markdown 正文版本生成的文章简介。\n",
            "3. image | 依赖：article、summary | 产物：articleImage — 基于标题、正文摘要和图片 Prompt 生成的一张配图。\n",
            "4. format | 依赖：article、summary、image | 产物：htmlDocument — 从规范 Markdown 正文派生的可复制 HTML；规范正文继续保留在同一文章包中。\n",
            "审核门禁：article、summary、image、format 全部成功且必需产物与血缘完整。\n"
        )
    );
}

/// 验证真实二进制提供可发现的只读帮助入口，且不再列出脚手架命令。
#[test]
fn exposes_a_read_only_product_help_entrypoint() {
    let output = cmd().arg("--help").output().unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("article"));
    assert!(stdout.contains("sop"));
    assert!(!stdout.contains("scaffold"));
}
