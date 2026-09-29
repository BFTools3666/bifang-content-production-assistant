use super::*;

/// 由用户显式触发一次最小文本请求，验证当前 Provider 配置与凭据。
#[tauri::command]
pub async fn verify_provider_connection(
    provider_id: String,
    stage: WorkflowStage,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ProviderConnectionResult, CommandError> {
    let _start_guard = worker.acquire_start().await;
    let workspace = repository.snapshot().await;
    let plan = match select_text_probe(&workspace, &provider_id, stage, fixture_mode_enabled())
        .map_err(provider_probe_command_error)?
    {
        TextProbeSelection::Fixture => {
            return Ok(ProviderConnectionResult {
                ok: true,
                message: "E2E 测试夹具可用；这不是一次真实 AI 验证".into(),
            });
        }
        TextProbeSelection::Request(plan) => plan,
    };
    let vault = CredentialVault;
    let api_key = vault
        .get_api_key(&plan.credential_ref)
        .map_err(provider_command_error)?;
    drop(_start_guard);
    let client = OpenAiCompatibleClient::new()
        .map_err(|message| CommandError::new("PROVIDER_CLIENT_FAILED", message))?;
    worker.acquire_provider_slot(&plan.provider.id).await;
    let result = client
        .generate_text(
            &plan.provider,
            &api_key,
            &plan.request_with_nonce(&uuid::Uuid::new_v4().to_string()),
        )
        .await;
    if let Err(error) = result {
        if let Some(retry_after_ms) = error.retry_after_ms {
            worker
                .defer_provider_slot(&plan.provider.id, retry_after_ms)
                .await;
        }
        return Err(provider_command_error(error));
    }
    Ok(ProviderConnectionResult {
        ok: true,
        message: "AI 文本服务连接成功".into(),
    })
}

/// 由用户确认可能计费后生成并校验一张图片，不保存测试图或工作区状态。
#[tauri::command]
pub async fn test_provider_image(
    provider_id: String,
    expected_revision: u32,
    confirmed: bool,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ProviderImageTestResult, CommandError> {
    require_image_probe_confirmation(confirmed).map_err(provider_probe_command_error)?;
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    let workspace = repository.snapshot().await;
    let plan = select_image_probe(&workspace, &provider_id, expected_revision, confirmed)
        .map_err(provider_probe_command_error)?;
    validate_image_test_endpoint(&plan.provider)?;
    let vault = CredentialVault;
    let api_key = vault
        .get_api_key(&plan.credential_ref)
        .map_err(provider_command_error)?;
    let client = OpenAiCompatibleClient::new()
        .map_err(|message| CommandError::new("PROVIDER_CLIENT_FAILED", message))?;
    worker.acquire_provider_slot(&plan.provider.id).await;
    let result = client
        .generate_image_with_contract(
            &plan.provider,
            &api_key,
            &plan.request_with_nonce(&uuid::Uuid::new_v4().to_string()),
            ImageRequestContract::for_provider_kind(plan.provider.kind),
        )
        .await
        .and_then(validated_image_generation_preview);
    let preview = match result {
        Ok(preview) => preview,
        Err(error) => {
            if let Some(retry_after_ms) = error.retry_after_ms {
                worker
                    .defer_provider_slot(&plan.provider.id, retry_after_ms)
                    .await;
            }
            return Err(image_test_command_error(error));
        }
    };
    Ok(ProviderImageTestResult {
        provider_id: plan.provider.id,
        provider_revision: plan.provider.revision,
        mime_type: preview.mime_type.into(),
        byte_length: u64::try_from(preview.byte_length).unwrap_or(u64::MAX),
        image_data_url: preview.data_url,
    })
}

/// 宿主适配器只验证真实请求使用的 HTTPS 端点协议。
pub(super) fn validate_image_test_endpoint(provider: &ProviderConfig) -> Result<(), CommandError> {
    let endpoint = provider_endpoint(&provider.base_url, "images/generations")
        .map_err(provider_command_error)?;
    if provider.kind == ProviderKind::VolcengineArkImage {
        validate_ark_generation_endpoint(&endpoint).map_err(provider_command_error)?;
    }
    Ok(())
}

/// 把 Core 连接测试拒绝原因映射为 GUI 错误，不重复维护业务码。
pub(super) fn provider_probe_command_error(error: ProviderProbeError) -> CommandError {
    CommandError::new(error.code(), error.message())
}

/// 图片请求的超时、传输中断或无效响应可能发生在计费生成之后，不能暗示安全重试。
pub(super) fn image_test_command_error(
    error: bifang_content_production_assistant_core::ProviderError,
) -> CommandError {
    if matches!(
        error.kind,
        ProviderErrorKind::Timeout
            | ProviderErrorKind::Transport
            | ProviderErrorKind::Server
            | ProviderErrorKind::InvalidResponse
            | ProviderErrorKind::UnknownOutcome
    ) {
        return CommandError::new(
            "PROVIDER_IMAGE_TEST_RESULT_UNCERTAIN",
            "图片测试结果无法确认，可能已经产生费用；请先在服务商控制台核对",
        );
    }
    provider_command_error(error)
}

/// 把 Provider 错误映射为稳定 GUI 错误。
pub(super) fn provider_command_error(
    error: bifang_content_production_assistant_core::ProviderError,
) -> CommandError {
    CommandError::new(
        match error.kind {
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
        },
        error.safe_message,
    )
}
