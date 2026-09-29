use std::{io::Write, path::PathBuf, process::ExitCode};

use bifang_content_production_assistant_core::{
    ArticleDetail, ArticleList, ArticleReadError, WorkflowDefinition, describe_fixed_content_sop,
    get_article, list_articles, validate_article_id,
};
use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::{
    article_output::{render_human_article_detail, render_human_article_list},
    article_store::ReadOnlySqliteArticleRepository,
};

/// 定义产品 CLI 的全局输出模式与只读命令。
#[derive(Parser)]
#[command(
    name = "bifang_content_production_assistant_cli",
    version = concat!("v", env!("CARGO_PKG_VERSION")),
    about,
    long_about = None
)]
struct Cli {
    /// 要求把调用结果渲染为稳定、可机器读取的 JSON 信封。
    #[arg(long, global = true)]
    json: bool,

    /// 选择当前获准执行的只读产品命令。
    #[command(subcommand)]
    command: Command,
}

/// 定义当前可用的产品命令组。
#[derive(Subcommand)]
enum Command {
    /// 从显式桌面应用数据目录只读查询文章，不迁移或修改工作区。
    Article(ArticleArgs),
    /// 查看固定内容生产 SOP，不执行内容生成。
    Sop(SopArgs),
}

/// 定义文章命令组内必须显式选择的只读操作。
#[derive(Args)]
struct ArticleArgs {
    /// 选择文章列表或单篇详情查询。
    #[command(subcommand)]
    command: ArticleCommand,
}

/// 定义当前可用的文章只读查询。
#[derive(Subcommand)]
enum ArticleCommand {
    /// 按产品默认顺序列出当前工作区中的全部文章。
    List(ArticleWorkspaceArgs),
    /// 按大小写敏感的稳定 ID 查询一篇文章详情。
    Get(ArticleGetArgs),
}

/// 指向桌面应用已保存工作区的显式路径参数。
#[derive(Args)]
struct ArticleWorkspaceArgs {
    /// 包含 content-workbench.sqlite3 的现有绝对应用数据目录。
    #[arg(long, value_name = "DIR")]
    app_data_dir: PathBuf,
}

/// 查询单篇文章所需的稳定 ID 与显式工作区路径。
#[derive(Args)]
struct ArticleGetArgs {
    /// 要精确查询的文章稳定 ID。
    #[arg(long, value_name = "ID")]
    id: String,
    /// 包含 content-workbench.sqlite3 的现有绝对应用数据目录。
    #[arg(long, value_name = "DIR")]
    app_data_dir: PathBuf,
}

/// 定义 SOP 命令组内必须显式选择的只读操作。
#[derive(Args)]
struct SopArgs {
    /// 选择一个只读 SOP 操作。
    #[command(subcommand)]
    command: SopCommand,
}

/// 定义当前可用的固定 SOP 查询操作。
#[derive(Subcommand)]
enum SopCommand {
    /// 描述四个固定阶段、执行依赖、产物和审核门禁。
    Describe,
}

/// 表示 CLI JSON 模式的统一顶层信封，确保成功与失败具有稳定形状。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope<T> {
    /// 标识本次命令是否完成预期只读查询。
    ok: bool,
    /// 成功时承载产品描述，失败时必须为空。
    data: Option<T>,
    /// 失败时承载稳定错误，成功时必须为空。
    error: Option<ErrorBody>,
    /// 承载便于 Agent 校验的协议元数据。
    meta: Meta,
}

/// 表示 CLI 参数错误的稳定机器接口形状。
#[derive(Serialize)]
struct ErrorBody {
    /// 供 Agent 分支处理的稳定机器错误码。
    code: &'static str,
    /// 供人类诊断的错误说明，不作为机器分支依据。
    message: String,
}

/// 表示每次 JSON 响应都必须携带的协议版本与调用上下文。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    /// 当前 JSON 信封的模式版本。
    schema_version: &'static str,
    /// 标识产生该结果的稳定产品命令或解析边界。
    command: &'static str,
    /// 记录响应生成时刻的 UTC 时间戳。
    timestamp: String,
}

/// 执行轻量 CLI 适配器的完整调用流程并返回稳定进程状态。
///
/// 参数解析失败时仍为显式 JSON 请求提供机器信封。文章命令只读取明确指定的
/// 现有 SQLite 工作区；所有命令都不读取标准输入、网络、凭据或外部进程。
pub async fn run() -> ExitCode {
    let wants_json = std::env::args_os().any(|argument| argument == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if wants_json && error.use_stderr() => {
            return write_json::<WorkflowDefinition>(
                Envelope {
                    ok: false,
                    data: None,
                    error: Some(ErrorBody {
                        code: "INVALID_ARGUMENT",
                        message: error.to_string(),
                    }),
                    meta: metadata("cli.parse"),
                },
                2,
            );
        }
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            return ExitCode::from(u8::try_from(exit_code).unwrap_or(70));
        }
    };

    match cli.command {
        Command::Article(ArticleArgs {
            command: ArticleCommand::List(arguments),
        }) => run_article_list(arguments, cli.json).await,
        Command::Article(ArticleArgs {
            command: ArticleCommand::Get(arguments),
        }) => run_article_get(arguments, cli.json).await,
        Command::Sop(SopArgs {
            command: SopCommand::Describe,
        }) => {
            let definition = describe_fixed_content_sop();
            if cli.json {
                write_json(
                    Envelope {
                        ok: true,
                        data: Some(definition),
                        error: None,
                        meta: metadata("sop.describe"),
                    },
                    0,
                )
            } else {
                println!("{}", render_human_sop(&definition));
                ExitCode::SUCCESS
            }
        }
    }
}

/// 打开严格只读工作区并执行文章列表 core 用例。
async fn run_article_list(arguments: ArticleWorkspaceArgs, json: bool) -> ExitCode {
    let result = async {
        let repository = ReadOnlySqliteArticleRepository::open(&arguments.app_data_dir).await?;
        list_articles(&repository).await
    }
    .await;
    match result {
        Ok(list) if json => write_json(
            Envelope {
                ok: true,
                data: Some(list),
                error: None,
                meta: metadata("article.list"),
            },
            0,
        ),
        Ok(list) => {
            println!("{}", render_human_article_list(&list));
            ExitCode::SUCCESS
        }
        Err(error) => write_article_error::<ArticleList>(error, json, "article.list"),
    }
}

/// 打开严格只读工作区并执行单篇文章详情 core 用例。
async fn run_article_get(arguments: ArticleGetArgs, json: bool) -> ExitCode {
    if let Err(error) = validate_article_id(&arguments.id) {
        return write_article_error::<ArticleDetail>(error, json, "article.get");
    }
    let result = async {
        let repository = ReadOnlySqliteArticleRepository::open(&arguments.app_data_dir).await?;
        get_article(&repository, &arguments.id).await
    }
    .await;
    match result {
        Ok(detail) if json => write_json(
            Envelope {
                ok: true,
                data: Some(detail),
                error: None,
                meta: metadata("article.get"),
            },
            0,
        ),
        Ok(detail) => {
            println!("{}", render_human_article_detail(&detail));
            ExitCode::SUCCESS
        }
        Err(error) => write_article_error::<ArticleDetail>(error, json, "article.get"),
    }
}

/// 把 core 只读错误稳定映射为 JSON 或人类诊断与进程退出码。
fn write_article_error<T: Serialize>(
    error: ArticleReadError,
    json: bool,
    command: &'static str,
) -> ExitCode {
    let exit_code = match error {
        ArticleReadError::WorkspacePathInvalid | ArticleReadError::ArticleIdInvalid => 2,
        ArticleReadError::WorkspaceNotFound | ArticleReadError::ArticleNotFound => 3,
        ArticleReadError::WorkspaceUnavailable => 6,
        ArticleReadError::WorkspaceInvalid | ArticleReadError::WorkspaceSchemaUnsupported => 10,
    };
    if json {
        write_json::<T>(
            Envelope {
                ok: false,
                data: None,
                error: Some(ErrorBody {
                    code: error.code(),
                    message: error.message().to_owned(),
                }),
                meta: metadata(command),
            },
            exit_code,
        )
    } else {
        eprintln!("{}: {}", error.code(), error.message());
        ExitCode::from(exit_code)
    }
}

/// 为一次 JSON 响应生成稳定的协议元数据，不读取或修改业务状态。
fn metadata(command: &'static str) -> Meta {
    Meta {
        schema_version: "1",
        command,
        timestamp: jiff::Timestamp::now().to_string(),
    }
}

/// 把 core 的固定 SOP 描述渲染为紧凑人类文本，不重建执行顺序或业务依赖。
fn render_human_sop(definition: &WorkflowDefinition) -> String {
    let mut lines = vec![format!(
        "固定内容 SOP（{} 个阶段）",
        definition.stages.len()
    )];
    for stage in &definition.stages {
        let dependencies = if stage.depends_on.is_empty() {
            "无".to_owned()
        } else {
            stage
                .depends_on
                .iter()
                .map(|dependency| dependency.stable_id())
                .collect::<Vec<_>>()
                .join("、")
        };
        lines.push(format!(
            "{}. {} | 依赖：{} | 产物：{} — {}",
            stage.order,
            stage.id.stable_id(),
            dependencies,
            stage.output.id.stable_id(),
            stage.output.semantics
        ));
    }

    let required_stages = definition
        .review_gate
        .required_stages
        .iter()
        .map(|stage| stage.stable_id())
        .collect::<Vec<_>>()
        .join("、");
    let artifact_requirement = if definition.review_gate.requires_complete_artifacts {
        "且必需产物与血缘完整"
    } else {
        ""
    };
    lines.push(format!(
        "审核门禁：{required_stages} 全部成功{artifact_requirement}。"
    ));
    lines.join("\n")
}

/// 把一个完整 JSON 信封原子地写入锁定的标准输出，并映射目标退出码。
///
/// 序列化或写入失败时不会继续输出不完整结果，而是向标准错误报告并返回内部错误退出码 70。
fn write_json<T: Serialize>(envelope: Envelope<T>, exit_code: u8) -> ExitCode {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    if serde_json::to_writer(&mut output, &envelope).is_err() || writeln!(output).is_err() {
        eprintln!("写入 JSON 结果失败");
        return ExitCode::from(70);
    }

    ExitCode::from(exit_code)
}
