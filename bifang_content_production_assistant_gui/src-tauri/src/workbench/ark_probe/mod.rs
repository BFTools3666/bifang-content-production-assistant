//! 火山方舟图片链路的一次性、只读配置诊断入口。

use std::{
    ffi::OsString,
    fs::{OpenOptions, symlink_metadata},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bifang_content_production_assistant_core::{
    ProviderConfig, ProviderError, ProviderErrorKind, ProviderProbeError,
    ensure_ark_probe_credential_ready, require_ark_probe_paid_confirmation,
    select_ark_probe_provider,
};
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{ConnectionTrait, DbBackend, SqlxSqliteConnector, Statement};
use serde::Serialize;

use super::{
    models::{ImageRequestContract, WORKSPACE_SCHEMA_VERSION, WorkspaceData},
    provider::{
        CredentialVault, OpenAiCompatibleClient, provider_endpoint,
        validate_ark_generation_endpoint,
    },
};

const DATABASE_FILE_NAME: &str = "content-workbench.sqlite3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum ProbeMode {
    Preflight,
    Execute,
}

#[derive(Debug, PartialEq, Eq)]
struct ProbeArguments {
    mode: ProbeMode,
    app_data_dir: PathBuf,
    output_dir: Option<PathBuf>,
    provider_id: Option<String>,
    confirmed_paid_generation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeStage {
    name: &'static str,
    status: &'static str,
    detail: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeProvider {
    id: String,
    name: String,
    revision: u32,
    model: String,
    size: String,
    watermark: bool,
    timeout_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeImageEvidence {
    mime_type: &'static str,
    byte_length: usize,
    width: u32,
    height: u32,
    output_path: String,
    provider_request_id_present: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeSuccess {
    ok: bool,
    mode: ProbeMode,
    provider: ProbeProvider,
    stages: Vec<ProbeStage>,
    generation_request_count: u8,
    automatic_retry_count: u8,
    image: Option<ProbeImageEvidence>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeFailure {
    ok: bool,
    stage: &'static str,
    code: &'static str,
    message: String,
    automatic_retry_count: u8,
}

impl ProbeFailure {
    fn new(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            stage,
            code,
            message: message.into(),
            automatic_retry_count: 0,
        }
    }

    fn from_provider(error: ProviderError) -> Self {
        let stage = if error.safe_message.starts_with("火山方舟已生成图片") {
            "download"
        } else if error.kind == ProviderErrorKind::InvalidResponse {
            "generationResponse"
        } else {
            "generation"
        };
        Self::new(stage, provider_error_code(error.kind), error.safe_message)
    }

    fn from_core(error: ProviderProbeError) -> Self {
        let (stage, message) = match error {
            ProviderProbeError::ArkCredentialPendingDeletion => (
                "credential",
                "图片服务凭据正在等待安全清理，请先在应用中重新保存密钥",
            ),
            ProviderProbeError::ArkPaidConfirmationRequired => (
                "confirmation",
                "执行真实方舟图片生成前必须传入 --confirm-paid-generation",
            ),
            ProviderProbeError::ArkProviderAmbiguous => (
                "provider",
                "存在多个已启用的方舟服务，请使用 --provider-id 明确选择",
            ),
            ProviderProbeError::ArkDefaultAmbiguous => {
                ("provider", "存在多个默认方舟图片服务，请先在应用中修正配置")
            }
            _ => ("provider", error.message()),
        };
        Self::new(stage, error.code(), message)
    }
}

/// 解析并执行一次探针；所有输出均为不含密钥、签名 URL 和响应正文的 JSON。
pub(crate) fn run_cli(arguments: Vec<OsString>) -> i32 {
    let arguments = match parse_arguments(arguments) {
        Ok(arguments) => arguments,
        Err(error) => return print_failure(error, 2),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            return print_failure(
                ProbeFailure::new("runtime", "PROBE_RUNTIME_FAILED", "无法初始化诊断运行时"),
                1,
            );
        }
    };
    match runtime.block_on(run_probe(arguments)) {
        Ok(success) => {
            print_json(&success, false);
            0
        }
        Err(error) => print_failure(error, 1),
    }
}

async fn run_probe(arguments: ProbeArguments) -> Result<ProbeSuccess, ProbeFailure> {
    let (workspace, canonical_app_data_dir) =
        load_workspace_read_only(&arguments.app_data_dir).await?;
    let plan = select_ark_probe_provider(&workspace, arguments.provider_id.as_deref())
        .map_err(ProbeFailure::from_core)?;
    let provider = &plan.provider;
    validate_provider_endpoint(provider)?;
    ensure_ark_probe_credential_ready(&workspace, &plan).map_err(ProbeFailure::from_core)?;
    let credential_ref = &plan.credential_ref;
    let vault = CredentialVault;
    match vault.api_key_presence(credential_ref) {
        Ok(true) => {}
        Ok(false) => {
            return Err(ProbeFailure::new(
                "credential",
                "PROBE_CREDENTIAL_MISSING",
                "系统凭据库中没有该图片服务的 API 密钥",
            ));
        }
        Err(message) => {
            return Err(ProbeFailure::new(
                "credential",
                "PROBE_CREDENTIAL_INACCESSIBLE",
                message,
            ));
        }
    }

    let parameters = plan.parameters.clone();
    let provider_summary = ProbeProvider {
        id: provider.id.clone(),
        name: provider.name.clone(),
        revision: provider.revision,
        model: provider.image_model.clone(),
        size: parameters.size.clone(),
        watermark: parameters.watermark,
        timeout_ms: parameters.timeout_ms,
    };
    let mut stages = vec![
        ProbeStage {
            name: "workspace",
            status: "ok",
            detail: "readOnlySnapshotLoaded",
        },
        ProbeStage {
            name: "provider",
            status: "ok",
            detail: "arkContractValidated",
        },
        ProbeStage {
            name: "credential",
            status: "ok",
            detail: "present",
        },
    ];
    if arguments.mode == ProbeMode::Preflight {
        return Ok(ProbeSuccess {
            ok: true,
            mode: arguments.mode,
            provider: provider_summary,
            stages,
            generation_request_count: 0,
            automatic_retry_count: 0,
            image: None,
        });
    }
    require_ark_probe_paid_confirmation(arguments.confirmed_paid_generation)
        .map_err(ProbeFailure::from_core)?;
    let output_dir = arguments.output_dir.as_deref().ok_or_else(|| {
        ProbeFailure::new(
            "arguments",
            "PROBE_OUTPUT_DIR_REQUIRED",
            "执行模式必须提供 --output-dir",
        )
    })?;
    let output_dir = validate_output_dir(output_dir, &canonical_app_data_dir)?;
    let api_key = vault.get_api_key(credential_ref).map_err(|error| {
        ProbeFailure::new(
            "credential",
            provider_error_code(error.kind),
            error.safe_message,
        )
    })?;
    let client = OpenAiCompatibleClient::new()
        .map_err(|message| ProbeFailure::new("client", "PROBE_CLIENT_FAILED", message))?;
    let result = client
        .generate_image_with_contract(
            &provider,
            &api_key,
            &plan.request_with_nonce(&uuid::Uuid::new_v4().to_string()),
            ImageRequestContract::VolcengineArkUrlV1,
        )
        .await
        .map_err(ProbeFailure::from_provider);
    drop(api_key);
    let result = result?;
    stages.push(ProbeStage {
        name: "generation",
        status: "ok",
        detail: "oneRequestCompleted",
    });
    stages.push(ProbeStage {
        name: "download",
        status: "ok",
        detail: "signedTosImageDownloaded",
    });
    let encoded = result.image_base64.ok_or_else(|| {
        ProbeFailure::new(
            "download",
            "PROBE_IMAGE_BYTES_MISSING",
            "方舟链路没有返回可校验的图片字节",
        )
    })?;
    let bytes = BASE64.decode(encoded.as_bytes()).map_err(|_| {
        ProbeFailure::new(
            "decode",
            "PROBE_IMAGE_BASE64_INVALID",
            "方舟链路返回的图片 Base64 无效",
        )
    })?;
    let (mime_type, extension, width, height) = inspect_image(&bytes)?;
    stages.push(ProbeStage {
        name: "decode",
        status: "ok",
        detail: "magicAndPixelsValidated",
    });
    let output_path = save_image(&output_dir, extension, &bytes)?;
    stages.push(ProbeStage {
        name: "save",
        status: "ok",
        detail: "freshPrivateFileCreated",
    });
    Ok(ProbeSuccess {
        ok: true,
        mode: arguments.mode,
        provider: provider_summary,
        stages,
        generation_request_count: 1,
        automatic_retry_count: 0,
        image: Some(ProbeImageEvidence {
            mime_type,
            byte_length: bytes.len(),
            width,
            height,
            output_path: output_path.to_string_lossy().into_owned(),
            provider_request_id_present: result.provider_request_id.is_some(),
        }),
    })
}

fn parse_arguments(arguments: Vec<OsString>) -> Result<ProbeArguments, ProbeFailure> {
    let mut values = arguments.into_iter();
    let mode = match values.next().as_deref().and_then(|value| value.to_str()) {
        Some("preflight") => ProbeMode::Preflight,
        Some("execute") => ProbeMode::Execute,
        _ => {
            return Err(ProbeFailure::new(
                "arguments",
                "PROBE_MODE_REQUIRED",
                "用法：--ark-image-probe preflight|execute --app-data-dir <目录> [--provider-id <ID>] [--output-dir <目录> --confirm-paid-generation]",
            ));
        }
    };
    let mut app_data_dir = None;
    let mut output_dir = None;
    let mut provider_id = None;
    let mut confirmed_paid_generation = false;
    while let Some(flag) = values.next() {
        match flag.to_str() {
            Some("--app-data-dir") => {
                app_data_dir = Some(PathBuf::from(next_value(&mut values, "--app-data-dir")?));
            }
            Some("--output-dir") => {
                output_dir = Some(PathBuf::from(next_value(&mut values, "--output-dir")?));
            }
            Some("--provider-id") => {
                let value = next_value(&mut values, "--provider-id")?;
                let value = value
                    .to_str()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        ProbeFailure::new(
                            "arguments",
                            "PROBE_PROVIDER_ID_INVALID",
                            "--provider-id 必须是非空文本",
                        )
                    })?;
                provider_id = Some(value.to_string());
            }
            Some("--confirm-paid-generation") => confirmed_paid_generation = true,
            _ => {
                return Err(ProbeFailure::new(
                    "arguments",
                    "PROBE_ARGUMENT_UNKNOWN",
                    "方舟探针包含未知或重复参数",
                ));
            }
        }
    }
    let app_data_dir = app_data_dir.ok_or_else(|| {
        ProbeFailure::new(
            "arguments",
            "PROBE_APP_DATA_DIR_REQUIRED",
            "方舟探针必须提供 --app-data-dir",
        )
    })?;
    if mode == ProbeMode::Preflight && (output_dir.is_some() || confirmed_paid_generation) {
        return Err(ProbeFailure::new(
            "arguments",
            "PROBE_PREFLIGHT_ARGUMENT_INVALID",
            "预检模式不会生成图片，不能携带输出目录或付费确认",
        ));
    }
    Ok(ProbeArguments {
        mode,
        app_data_dir,
        output_dir,
        provider_id,
        confirmed_paid_generation,
    })
}

fn next_value(
    values: &mut impl Iterator<Item = OsString>,
    flag: &str,
) -> Result<OsString, ProbeFailure> {
    values.next().ok_or_else(|| {
        ProbeFailure::new(
            "arguments",
            "PROBE_ARGUMENT_VALUE_MISSING",
            format!("{flag} 缺少参数值"),
        )
    })
}

async fn load_workspace_read_only(
    app_data_dir: &Path,
) -> Result<(WorkspaceData, PathBuf), ProbeFailure> {
    if !app_data_dir.is_absolute() {
        return Err(ProbeFailure::new(
            "workspace",
            "PROBE_APP_DATA_DIR_INVALID",
            "应用数据目录必须是绝对路径",
        ));
    }
    let directory_metadata = symlink_metadata(app_data_dir).map_err(|_| {
        ProbeFailure::new(
            "workspace",
            "PROBE_APP_DATA_DIR_UNAVAILABLE",
            "无法读取应用数据目录",
        )
    })?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err(ProbeFailure::new(
            "workspace",
            "PROBE_APP_DATA_DIR_INVALID",
            "应用数据目录必须是非符号链接目录",
        ));
    }
    let database_path = app_data_dir.join(DATABASE_FILE_NAME);
    let database_metadata = symlink_metadata(&database_path).map_err(|_| {
        ProbeFailure::new(
            "workspace",
            "PROBE_DATABASE_UNAVAILABLE",
            "找不到当前应用的工作区数据库",
        )
    })?;
    if database_metadata.file_type().is_symlink() || !database_metadata.is_file() {
        return Err(ProbeFailure::new(
            "workspace",
            "PROBE_DATABASE_INVALID",
            "工作区数据库必须是普通文件",
        ));
    }
    let canonical_directory = app_data_dir.canonicalize().map_err(|_| {
        ProbeFailure::new(
            "workspace",
            "PROBE_APP_DATA_DIR_UNAVAILABLE",
            "无法确认应用数据目录",
        )
    })?;
    let canonical_database = database_path.canonicalize().map_err(|_| {
        ProbeFailure::new(
            "workspace",
            "PROBE_DATABASE_UNAVAILABLE",
            "无法确认工作区数据库",
        )
    })?;
    if canonical_database.parent() != Some(canonical_directory.as_path()) {
        return Err(ProbeFailure::new(
            "workspace",
            "PROBE_DATABASE_INVALID",
            "工作区数据库不在指定应用数据目录内",
        ));
    }
    let options = SqliteConnectOptions::new()
        .filename(&canonical_database)
        .read_only(true)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| {
            ProbeFailure::new(
                "workspace",
                "PROBE_DATABASE_OPEN_FAILED",
                "无法以只读方式打开工作区数据库",
            )
        })?;
    let database = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    let row = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT payload FROM workspace_snapshot WHERE id = 1".to_string(),
        ))
        .await
        .map_err(|_| {
            ProbeFailure::new(
                "workspace",
                "PROBE_DATABASE_READ_FAILED",
                "无法读取工作区快照",
            )
        })?
        .ok_or_else(|| {
            ProbeFailure::new(
                "workspace",
                "PROBE_WORKSPACE_MISSING",
                "工作区数据库尚未保存配置",
            )
        })?;
    let payload: String = row.try_get("", "payload").map_err(|_| {
        ProbeFailure::new("workspace", "PROBE_WORKSPACE_INVALID", "工作区快照无法读取")
    })?;
    let workspace = serde_json::from_str::<WorkspaceData>(&payload).map_err(|_| {
        ProbeFailure::new("workspace", "PROBE_WORKSPACE_INVALID", "工作区快照格式无效")
    })?;
    if workspace.schema_version != WORKSPACE_SCHEMA_VERSION {
        return Err(ProbeFailure::new(
            "workspace",
            "PROBE_WORKSPACE_SCHEMA_UNSUPPORTED",
            "工作区版本与当前探针不一致，请先启动一次当前桌面应用",
        ));
    }
    Ok((workspace, canonical_directory))
}

fn validate_provider_endpoint(provider: &ProviderConfig) -> Result<(), ProbeFailure> {
    let endpoint = provider_endpoint(&provider.base_url, "images/generations")
        .map_err(ProbeFailure::from_provider)?;
    validate_ark_generation_endpoint(&endpoint).map_err(ProbeFailure::from_provider)
}

fn validate_output_dir(
    output_dir: &Path,
    canonical_app_data_dir: &Path,
) -> Result<PathBuf, ProbeFailure> {
    if !output_dir.is_absolute() {
        return Err(ProbeFailure::new(
            "save",
            "PROBE_OUTPUT_DIR_INVALID",
            "输出目录必须是绝对路径",
        ));
    }
    let metadata = symlink_metadata(output_dir).map_err(|_| {
        ProbeFailure::new("save", "PROBE_OUTPUT_DIR_UNAVAILABLE", "无法读取输出目录")
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ProbeFailure::new(
            "save",
            "PROBE_OUTPUT_DIR_INVALID",
            "输出目录必须是已存在的非符号链接目录",
        ));
    }
    let canonical_output_dir = output_dir.canonicalize().map_err(|_| {
        ProbeFailure::new("save", "PROBE_OUTPUT_DIR_UNAVAILABLE", "无法确认输出目录")
    })?;
    if canonical_output_dir == canonical_app_data_dir
        || canonical_output_dir.starts_with(canonical_app_data_dir)
    {
        return Err(ProbeFailure::new(
            "save",
            "PROBE_OUTPUT_DIR_NOT_ISOLATED",
            "输出目录必须位于应用工作区之外",
        ));
    }
    Ok(canonical_output_dir)
}

fn inspect_image(bytes: &[u8]) -> Result<(&'static str, &'static str, u32, u32), ProbeFailure> {
    let (format, mime_type, extension) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (image::ImageFormat::Png, "image/png", "png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        (image::ImageFormat::Jpeg, "image/jpeg", "jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        (image::ImageFormat::WebP, "image/webp", "webp")
    } else {
        return Err(ProbeFailure::new(
            "decode",
            "PROBE_IMAGE_MAGIC_INVALID",
            "下载内容不是受支持的 PNG、JPEG 或 WebP 图片",
        ));
    };
    let decoded = image::load_from_memory_with_format(bytes, format).map_err(|_| {
        ProbeFailure::new(
            "decode",
            "PROBE_IMAGE_DECODE_FAILED",
            "图片魔数有效，但像素数据无法解码",
        )
    })?;
    Ok((mime_type, extension, decoded.width(), decoded.height()))
}

fn save_image(output_dir: &Path, extension: &str, bytes: &[u8]) -> Result<PathBuf, ProbeFailure> {
    let output_path = output_dir.join(format!(
        "ark-image-probe-{}.{}",
        uuid::Uuid::new_v4(),
        extension
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&output_path).map_err(|_| {
        ProbeFailure::new(
            "save",
            "PROBE_IMAGE_CREATE_FAILED",
            "无法创建新的探针图片文件",
        )
    })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| {
            ProbeFailure::new(
                "save",
                "PROBE_IMAGE_WRITE_FAILED",
                "无法完整写入探针图片文件",
            )
        })?;
    output_path.canonicalize().map_err(|_| {
        ProbeFailure::new(
            "save",
            "PROBE_IMAGE_VERIFY_FAILED",
            "图片已写入但无法确认输出文件",
        )
    })
}

fn provider_error_code(kind: ProviderErrorKind) -> &'static str {
    match kind {
        ProviderErrorKind::NotConfigured => "PROVIDER_NOT_CONFIGURED",
        ProviderErrorKind::Authentication => "PROVIDER_AUTHENTICATION",
        ProviderErrorKind::InvalidRequest => "PROVIDER_INVALID_REQUEST",
        ProviderErrorKind::RateLimited => "PROVIDER_RATE_LIMITED",
        ProviderErrorKind::Timeout => "PROVIDER_TIMEOUT",
        ProviderErrorKind::Transport => "PROVIDER_TRANSPORT",
        ProviderErrorKind::Server => "PROVIDER_SERVER",
        ProviderErrorKind::ContentRejected => "PROVIDER_CONTENT_REJECTED",
        ProviderErrorKind::InvalidResponse => "PROVIDER_INVALID_RESPONSE",
        ProviderErrorKind::Cancelled => "PROVIDER_CANCELLED",
        ProviderErrorKind::UnknownOutcome => "PROVIDER_UNKNOWN_OUTCOME",
    }
}

fn print_failure(error: ProbeFailure, exit_code: i32) -> i32 {
    print_json(&error, true);
    exit_code
}

fn print_json(value: &impl Serialize, stderr: bool) {
    let payload = serde_json::to_string(value).unwrap_or_else(|_| {
        "{\"ok\":false,\"stage\":\"output\",\"code\":\"PROBE_OUTPUT_FAILED\",\"message\":\"无法序列化诊断结果\",\"automaticRetryCount\":0}".to_string()
    });
    if stderr {
        eprintln!("{payload}");
    } else {
        println!("{payload}");
    }
}

#[cfg(test)]
mod tests;
