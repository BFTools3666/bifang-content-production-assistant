use super::*;

/// 返回由 core 定义、尚无稳定 ID 或密钥的新 Provider 表单默认值。
#[tauri::command]
pub async fn new_provider_config_draft() -> ProviderConfig {
    ProviderConfig::open_ai_compatible_draft()
}

/// 返回由 core 定义的火山方舟图片服务草稿；只包含图片能力且不触发联网。
#[tauri::command]
pub async fn new_volcengine_ark_image_provider_config_draft() -> ProviderConfig {
    ProviderConfig::volcengine_ark_image_draft()
}

/// 返回非秘密 Provider 配置以及是否已保存密钥。
#[tauri::command]
pub async fn list_provider_configs(
    repository: State<'_, Arc<WorkspaceRepository>>,
) -> Result<Vec<ProviderConfigView>, CommandError> {
    let workspace = repository.snapshot().await;
    Ok(provider_config_views(&workspace))
}

/// 从同一工作区快照建立全部非秘密 Provider 视图。
///
/// `has_api_key` 表示应用已原子登记过一个凭据引用。被动列表不解密
/// 系统凭据库，避免重签名应用的授权对话框卡住页面和批次锁。
pub(crate) fn provider_config_views(workspace: &WorkspaceData) -> Vec<ProviderConfigView> {
    workspace
        .provider_configs
        .iter()
        .filter(|config| config.kind != ProviderKind::Fixture || fixture_mode_enabled())
        .map(|config| ProviderConfigView {
            has_api_key: config.kind == ProviderKind::Fixture
                || workspace.provider_credential_refs.contains_key(&config.id),
            is_test_fixture: config.kind == ProviderKind::Fixture,
            config: config.clone(),
        })
        .collect()
}

/// 校验并保存 Provider 非秘密配置，可选将密钥写入系统凭据库。
#[tauri::command]
pub async fn save_provider_config(
    mut config: ProviderConfig,
    api_key: Option<String>,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ProviderConfigSaveResult, CommandError> {
    if fixture_mode_enabled() {
        return Err(CommandError::new(
            "FIXTURE_READ_ONLY",
            "测试夹具模式下 AI 服务配置为只读",
        ));
    }
    if config.kind == ProviderKind::Fixture {
        return Err(CommandError::new(
            "FIXTURE_DISABLED",
            "正式界面不能创建或启用测试 Provider",
        ));
    }
    config = prepare_provider_config(config, &format!("provider-{}", uuid::Uuid::new_v4()))
        .map_err(provider_config_error)?;
    let requested_endpoint = provider_primary_endpoint(&config)?;
    // 与批次的“读取工作区 + 读取系统凭据”快照处于同一临界区，避免
    // Provider 修订与密钥映射在快照中被拆成两个不同版本。
    let _start_guard = worker.acquire_start().await;
    let has_new_api_key = api_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let vault = CredentialVault;
    let workspace = repository.snapshot().await;
    if !has_new_api_key {
        if let Some(previous) = workspace
            .provider_configs
            .iter()
            .find(|provider| provider.id == config.id)
        {
            if provider_transport_changed(previous, &config, requested_endpoint.as_ref()) {
                let credential_ref = workspace
                    .provider_credential_refs
                    .get(&previous.id)
                    .map(String::as_str)
                    .unwrap_or(previous.id.as_str());
                let old_key_is_present = vault
                    .api_key_presence(credential_ref)
                    .map_err(|message| CommandError::new("CREDENTIAL_ACCESS_FAILED", message))?;
                if endpoint_change_requires_key_rotation(
                    previous,
                    &config,
                    requested_endpoint.as_ref(),
                    old_key_is_present,
                ) {
                    return Err(CommandError::new(
                        "PROVIDER_KEY_REQUIRED_FOR_ENDPOINT_CHANGE",
                        "修改已有密钥的 AI 服务地址时必须同时重新输入 API 密钥",
                    ));
                }
            }
        }
    }
    let new_credential_ref = api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|_| format!("{}:revision:{}", config.id, uuid::Uuid::new_v4()));
    if let (Some(api_key), Some(credential_ref)) = (
        api_key.as_deref().filter(|value| !value.trim().is_empty()),
        new_credential_ref.as_deref(),
    ) {
        repository
            .update(|workspace| {
                workspace
                    .pending_credential_deletions
                    .insert(credential_ref.to_string());
                Ok(())
            })
            .await
            .map_err(store_error)?;
        if let Err(message) = vault.set_api_key(credential_ref, api_key) {
            reconcile_pending_credential_deletions(&repository).await;
            return Err(CommandError::new("CREDENTIAL_SAVE_FAILED", message));
        }
    }
    let save_result = repository
        .update(|workspace| {
            apply_provider_config_update(workspace, config.clone(), new_credential_ref.as_deref())
        })
        .await;
    let (config, credential_ref, superseded_credential_ref) = match save_result {
        Ok(saved) => saved,
        Err(message) => {
            reconcile_pending_credential_deletions(&repository).await;
            return Err(provider_update_error(message));
        }
    };
    let _ = superseded_credential_ref;
    reconcile_pending_credential_deletions(&repository).await;
    let workspace = repository.snapshot().await;
    let providers = provider_config_views(&workspace);
    let saved = providers
        .iter()
        .find(|provider| provider.config.id == config.id)
        .cloned()
        .ok_or_else(|| store_error("保存后的 AI 服务未能从工作区复读".into()))?;
    debug_assert_eq!(
        saved.has_api_key,
        workspace
            .provider_credential_refs
            .contains_key(&saved.config.id),
        "saved provider view must report credential registration"
    );
    debug_assert_eq!(
        workspace
            .provider_credential_refs
            .get(&saved.config.id)
            .map(String::as_str)
            .unwrap_or(saved.config.id.as_str()),
        credential_ref,
        "saved provider view must use the active credential reference"
    );
    Ok(ProviderConfigSaveResult { saved, providers })
}

/// 返回一种真实 Provider 的主请求端点，用于保存校验和密钥轮换边界。
pub(super) fn provider_primary_endpoint(
    config: &ProviderConfig,
) -> Result<Option<reqwest::Url>, CommandError> {
    let path = match config.kind {
        ProviderKind::OpenAiCompatible => "chat/completions",
        ProviderKind::VolcengineArkImage => "images/generations",
        ProviderKind::Fixture => return Ok(None),
    };
    let endpoint = provider_endpoint(&config.base_url, path)
        .map_err(|error| CommandError::new("PROVIDER_BASE_URL_INVALID", error.safe_message))?;
    if config.kind == ProviderKind::VolcengineArkImage {
        validate_ark_generation_endpoint(&endpoint)
            .map_err(|error| CommandError::new("PROVIDER_BASE_URL_INVALID", error.safe_message))?;
    }
    Ok(Some(endpoint))
}

/// 判断请求协议或实际发送地址是否变化；旧地址无法解析时按变化处理并失败关闭。
pub(super) fn provider_transport_changed(
    previous: &ProviderConfig,
    requested: &ProviderConfig,
    requested_endpoint: Option<&reqwest::Url>,
) -> bool {
    if previous.kind != requested.kind {
        return true;
    }
    let Some(requested_endpoint) = requested_endpoint else {
        return previous.kind != ProviderKind::Fixture;
    };
    let path = match previous.kind {
        ProviderKind::OpenAiCompatible => "chat/completions",
        ProviderKind::VolcengineArkImage => "images/generations",
        ProviderKind::Fixture => return false,
    };
    provider_endpoint(&previous.base_url, path)
        .map(|previous_endpoint| previous_endpoint != *requested_endpoint)
        .unwrap_or(true)
}

/// 地址变化只有在旧配置确实持有密钥时才要求用户同步轮换。
pub(super) fn endpoint_change_requires_key_rotation(
    previous: &ProviderConfig,
    requested: &ProviderConfig,
    requested_endpoint: Option<&reqwest::Url>,
    old_key_is_present: bool,
) -> bool {
    should_rotate_provider_key(
        old_key_is_present,
        provider_transport_changed(previous, requested, requested_endpoint),
    )
}

/// 把 core 的三阶段配置拒绝映射为稳定 IPC 错误。
pub(super) fn provider_config_error(error: ProviderConfigError) -> CommandError {
    CommandError::new(error.code(), error.to_string())
}

/// 区分 Provider 修订上限与真正的仓储失败，避免覆盖不可变修订。
pub(super) fn provider_update_error(message: String) -> CommandError {
    let safe_message = match message.as_str() {
        "PROVIDER_ID_REQUIRED" => "AI 服务稳定 ID 不能为空",
        "PROVIDER_NAME_REQUIRED" => "AI 服务名称不能为空",
        "PROVIDER_STAGE_MODEL_REQUIRED" => "至少为一个阶段填写模型",
        "PROVIDER_DEFAULT_MODEL_REQUIRED" => "设为默认的阶段必须填写对应模型",
        "PROVIDER_STAGE_UNSUPPORTED" => "该 AI 服务不支持所配置的工作流阶段",
        "PROVIDER_DISABLED_DEFAULT" => "已停用的 AI 服务不能设为默认服务",
        "PROVIDER_PARAMETER_INVALID" => "AI 服务生成参数超出允许范围",
        "PROVIDER_REVISION_EXHAUSTED" => "AI 服务修订号已达到上限，无法再保存或切换默认服务",
        "PROVIDER_REVISION_CONFLICT" => "AI 服务已被其他操作更新，请刷新后重试",
        "PROVIDER_NOT_FOUND" => "找不到指定 AI 服务",
        _ => return store_error(message),
    };
    CommandError::new(message, safe_message)
}

/// 重试持久化清理队列；单项钥匙串失败只保留该项，不能丢失后续重试依据。
pub(crate) async fn reconcile_pending_credential_deletions(repository: &WorkspaceRepository) {
    let vault = CredentialVault;
    reconcile_pending_credential_deletions_with(repository, |credential_ref| {
        vault.delete_api_key(credential_ref)
    })
    .await;
}

/// 用可注入删除器执行清理，避免单元测试接触真实系统凭据库。
pub(super) async fn reconcile_pending_credential_deletions_with(
    repository: &WorkspaceRepository,
    mut delete: impl FnMut(&str) -> Result<(), String>,
) {
    let pending = repository
        .snapshot()
        .await
        .pending_credential_deletions
        .into_iter()
        .collect::<Vec<_>>();
    for credential_ref in pending {
        let snapshot = repository.snapshot().await;
        if !snapshot
            .pending_credential_deletions
            .contains(&credential_ref)
        {
            continue;
        }
        if snapshot
            .referenced_credential_refs()
            .contains(&credential_ref)
        {
            let _ = repository
                .update(|workspace| {
                    if workspace
                        .referenced_credential_refs()
                        .contains(&credential_ref)
                    {
                        workspace
                            .pending_credential_deletions
                            .remove(&credential_ref);
                    }
                    Ok(())
                })
                .await;
            continue;
        }
        if delete(&credential_ref).is_err() {
            continue;
        }
        let _ = repository
            .update(|workspace| {
                workspace
                    .pending_credential_deletions
                    .remove(&credential_ref);
                Ok(())
            })
            .await;
    }
}
