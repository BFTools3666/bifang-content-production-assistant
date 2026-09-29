use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use crate::workbench::models::RunRecord;
use bifang_content_production_assistant_core::{
    ImageGenerationParameters, StageStatus, TextGenerationParameters, prepare_retry_batch,
};

use super::*;

fn fixture_provider() -> ProviderConfig {
    ProviderConfig {
        id: "provider-fixture".into(),
        name: "fixture".into(),
        revision: 1,
        kind: ProviderKind::Fixture,
        enabled: true,
        base_url: String::new(),
        text_model: "fixture-text".into(),
        article_model: Some("fixture-article".into()),
        summary_model: Some("fixture-summary".into()),
        image_model: "fixture-image".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    }
}

/// 把既有完整模型配置切换到只访问本地测试服务的真实 Provider 路径。
fn local_open_ai_provider(base_url: String) -> ProviderConfig {
    let mut provider = fixture_provider();
    provider.kind = ProviderKind::OpenAiCompatible;
    provider.base_url = base_url;
    provider
}

/// 构造指定尺寸的合法 PNG，用于验证缩略图边界。
fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = ::image::DynamicImage::new_rgba8(width, height);
    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, ::image::ImageFormat::Png)
        .unwrap();
    encoded.into_inner()
}

/// 返回一次含非法 Base64 图片的本地 OpenAI-compatible 响应。
fn serve_invalid_image_response() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let body = r#"{"data":[{"b64_json":"%%% invalid image payload"}],"usage":{"output_tokens":23,"total_tokens":29}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{address}/v1")
}

/// 返回一次含部分精确 usage 的本地 OpenAI-compatible 文本响应。
fn serve_text_usage_response() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let body = r#"{"choices":[{"message":{"content":"持久化用量正文"}}],"usage":{"prompt_tokens":17,"total_tokens":31}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{address}/v1")
}

/// 在当前测试任务内启用确定性夹具，并沿真实 worker 批次路径执行。
async fn run_fixture_batch_inner(
    repository: &WorkspaceRepository,
    batch_id: &str,
    task_ids: &[String],
    executions: &BTreeMap<String, ExecutionSnapshot>,
) -> Result<(), String> {
    let provider_client = OpenAiCompatibleClient::new()?;
    let provider_throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);
    TEST_FIXTURE_EXECUTION_ENABLED
        .scope(
            true,
            run_batch_inner(
                repository,
                batch_id,
                task_ids,
                executions,
                &provider_client,
                Path::new("unused"),
                &provider_throttle,
                &paused,
                &cancelled,
                None,
            ),
        )
        .await
}

/// 在当前测试任务内沿真实 worker 路径只执行一个目标阶段。
async fn run_fixture_stage_inner(
    repository: &WorkspaceRepository,
    batch_id: &str,
    task_id: &str,
    execution: &ExecutionSnapshot,
    stage: WorkflowStage,
) -> Result<(), String> {
    let provider_client = OpenAiCompatibleClient::new()?;
    let provider_throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);
    TEST_FIXTURE_EXECUTION_ENABLED
        .scope(
            true,
            run_batch_inner(
                repository,
                batch_id,
                &[task_id.to_string()],
                &BTreeMap::from([(task_id.to_string(), execution.clone())]),
                &provider_client,
                Path::new("unused"),
                &provider_throttle,
                &paused,
                &cancelled,
                Some(stage),
            ),
        )
        .await
}

mod execution;
mod image;
mod run_state;
mod snapshot;
