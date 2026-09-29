use super::*;

/// 阶段失败及 Provider 在失败前已经真实返回的用量。
#[derive(Debug)]
pub(super) struct StageExecutionError {
    pub(super) error: ProviderError,
    pub(super) token_usage: Option<TokenUsage>,
}

impl From<ProviderError> for StageExecutionError {
    fn from(error: ProviderError) -> Self {
        Self {
            error,
            token_usage: None,
        }
    }
}

impl From<ProviderCallError> for StageExecutionError {
    fn from(error: ProviderCallError) -> Self {
        Self {
            error: error.error,
            token_usage: error.token_usage,
        }
    }
}

/// 执行一个阶段并返回产物、冻结血缘及真实 token 用量。
pub(super) async fn execute_stage(
    task: &ArticleTask,
    stage: WorkflowStage,
    execution: &ExecutionSnapshot,
    provider_client: &OpenAiCompatibleClient,
    asset_dir: &Path,
    provider_throttle: &tokio::sync::Mutex<BTreeMap<String, Instant>>,
    attempt_id: String,
    paused: &AtomicBool,
    cancelled: &AtomicBool,
) -> Result<(ArtifactOutput, ArtifactLineage, Option<TokenUsage>), StageExecutionError> {
    if !wait_until_execution_allowed(paused, cancelled).await {
        return Err(controlled_cancelled_error().into());
    }
    let plan = StageExecutionPlan::from_task(task, stage, &execution.to_plan(), attempt_id)?;
    let (provider, model, prompt, parameters, idempotency_key, lineage) = match plan {
        StageExecutionPlan::LocalFormat { html, lineage } => {
            return Ok((
                ArtifactOutput::Format(html),
                lineage,
                Some(TokenUsage::local_processing()),
            ));
        }
        StageExecutionPlan::ProviderRequest {
            provider,
            model,
            prompt,
            parameters,
            idempotency_key,
            lineage,
        } => (
            provider,
            model,
            prompt,
            parameters,
            idempotency_key,
            lineage,
        ),
    };
    let frozen_provider = match stage {
        WorkflowStage::Article => execution.article_provider.as_ref(),
        WorkflowStage::Summary => execution.summary_provider.as_ref(),
        WorkflowStage::Image => execution.image_provider.as_ref(),
        WorkflowStage::Format => unreachable!("local format returned before Provider lookup"),
    }
    .ok_or_else(|| ProviderError {
        kind: ProviderErrorKind::NotConfigured,
        safe_message: "当前步骤尚未选择 AI 服务".into(),
        retry_after_ms: None,
    })?;

    let (output, token_usage) = if provider.kind == ProviderKind::Fixture {
        (execute_fixture(task, stage).await?, None)
    } else {
        let api_key = frozen_provider
            .api_key
            .clone()
            .ok_or_else(|| ProviderError {
                kind: ProviderErrorKind::NotConfigured,
                safe_message: "尚未为该 AI 服务保存 API 密钥".into(),
                retry_after_ms: None,
            })?;
        if !wait_for_provider_slot_controlled(provider_throttle, &provider.id, paused, cancelled)
            .await
        {
            return Err(controlled_cancelled_error().into());
        }
        let session = OpenAiProviderSession::new(
            provider_client.clone(),
            provider.clone(),
            api_key,
            execution.image_request_contract,
        );
        let result: Result<(ArtifactOutput, Option<TokenUsage>), StageExecutionError> =
            match (stage, parameters) {
                (
                    WorkflowStage::Article | WorkflowStage::Summary,
                    StageGenerationParameters::Text(parameters),
                ) => session
                    .generate_text_with_usage_error(&TextGenerationRequest {
                        model,
                        prompt,
                        parameters,
                        idempotency_key,
                    })
                    .await
                    .map(|result| {
                        let output = if stage == WorkflowStage::Article {
                            ArtifactOutput::Article(result.text)
                        } else {
                            ArtifactOutput::Summary(result.text)
                        };
                        (output, result.token_usage)
                    })
                    .map_err(StageExecutionError::from),
                (WorkflowStage::Image, StageGenerationParameters::Image(parameters)) => {
                    match session
                        .generate_image_with_usage_error(&ImageGenerationRequest {
                            model,
                            prompt,
                            parameters,
                            idempotency_key,
                        })
                        .await
                    {
                        Ok(result) => {
                            let token_usage = result.token_usage.clone();
                            store_image_result(result, asset_dir)
                                .await
                                .map(|reference| {
                                    (ArtifactOutput::Image(reference), token_usage.clone())
                                })
                                .map_err(|error| StageExecutionError { error, token_usage })
                        }
                        Err(error) => Err(error.into()),
                    }
                }
                _ => unreachable!("Core plan parameters match the target stage"),
            };
        match result {
            Ok(output) => output,
            Err(error) => {
                if let Some(retry_after_ms) = error.error.retry_after_ms {
                    defer_provider_slot(provider_throttle, &provider.id, retry_after_ms).await;
                }
                return Err(error);
            }
        }
    };
    Ok((output, lineage, token_usage))
}

/// 执行显式调试 E2E 夹具，正式构建和普通调试默认都不可用。
pub(super) async fn execute_fixture(
    task: &ArticleTask,
    stage: WorkflowStage,
) -> Result<ArtifactOutput, ProviderError> {
    if !fixture_execution_enabled() {
        return Err(ProviderError {
            kind: ProviderErrorKind::NotConfigured,
            safe_message: "测试夹具没有在当前环境启用".into(),
            retry_after_ms: None,
        });
    }
    tokio::time::sleep(Duration::from_millis(140)).await;
    if stage == WorkflowStage::Image
        && task.title.contains("失败一次")
        && task.stages.image.attempts == 1
    {
        return Err(ProviderError {
            kind: ProviderErrorKind::Server,
            safe_message: "测试夹具模拟图片服务暂时不可用".into(),
            retry_after_ms: Some(100),
        });
    }
    Ok(match stage {
        WorkflowStage::Article => ArtifactOutput::Article(format!(
            "# {}\n\n这是一篇由 E2E 测试夹具生成的示例正文，用于验证批量任务的状态、重试和审核流程。\n\n## 核心要点\n\n- 标题驱动内容生成\n- 每个阶段独立记录状态\n- 失败后只重试对应阶段\n\n## 总结\n\n{} 的内容包已经准备进入下一阶段。",
            task.title, task.title
        )),
        WorkflowStage::Summary => ArtifactOutput::Summary(format!(
            "本文围绕“{}”说明批量 AI 内容工作台如何生成正文、简介、配图和格式产物，并通过阶段状态与精确重试保护已有成果。",
            task.title
        )),
        WorkflowStage::Image => ArtifactOutput::Image(fixture_image_data_url(&task.title)),
        WorkflowStage::Format => unreachable!("format uses local renderer"),
    })
}

/// 创建无远程请求、可直接展示且明确标注测试用途的 SVG 配图。
pub(super) fn fixture_image_data_url(title: &str) -> String {
    let escaped = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1200' height='675'><defs><linearGradient id='g' x1='0' x2='1'><stop stop-color='#667eea'/><stop offset='1' stop-color='#764ba2'/></linearGradient></defs><rect width='1200' height='675' rx='36' fill='url(#g)'/><text x='72' y='100' fill='white' font-size='28' font-family='sans-serif'>E2E 测试夹具 · 非真实 AI</text><text x='72' y='340' fill='white' font-size='52' font-weight='700' font-family='sans-serif'>{escaped}</text><text x='72' y='590' fill='#eef2ff' font-size='26' font-family='sans-serif'>毕方内容生产助手</text></svg>"
    );
    format!("data:image/svg+xml;base64,{}", BASE64.encode(svg))
}
