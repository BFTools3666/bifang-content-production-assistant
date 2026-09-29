//! 有单一生命周期所有者的批量 SOP worker。

use std::{
    collections::BTreeMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bifang_content_production_assistant_core::{
    ArticleTask, ArtifactLineage, ArtifactOutput, ExecutionPlan, Failure, ImageGenerationRequest,
    ImageGenerationResult, PromptTemplate, ProviderError, ProviderErrorKind, StageClaim,
    StageExecutionPlan, StageGenerationParameters, TextGenerationRequest, TokenUsage,
    WorkflowStage, cancel_task_in_workspace, claim_next_stage_in_workspace,
    derive_terminal_run_status as derive_terminal_run_status_in_workspace, find_article_mut,
    mark_run_started_in_workspace, recover_interrupted_run_articles,
    refresh_run_counts_in_workspace, transition_controlled_run_status_in_workspace,
    validate_execution_configuration_for_kind, validate_execution_providers,
    validate_prompt_renderer_version,
};
#[cfg(test)]
use pulldown_cmark::{Options, Parser, html};
use tauri::async_runtime::JoinHandle;
use tokio::io::AsyncReadExt;
use tokio::time::Instant;

use super::{
    models::{
        CommandError, ImageRequestContract, PackageExecutionBinding, ProviderConfig, ProviderExecutionBinding,
        ProviderKind, RunStatus, WorkspaceData, fixture_mode_enabled,
    },
    provider::{CredentialVault, OpenAiCompatibleClient, OpenAiProviderSession, ProviderCallError},
    store::{WorkspaceRepository, current_time_ms},
};

#[cfg(test)]
use bifang_content_production_assistant_core::{
    CURRENT_PROMPT_RENDERER_VERSION, LEGACY_PROMPT_RENDERER_VERSION, OverallStatus,
    PromptTemplateKind, image_request_idempotency_material, source_article_version,
    terminal_run_status,
};

pub(crate) const MAX_STORED_IMAGE_BYTES: usize = 25 * 1024 * 1024;
const MAX_ENCODED_IMAGE_CHARS: usize = (MAX_STORED_IMAGE_BYTES * 4 / 3) + 8;
const GRID_THUMBNAIL_WIDTH: u32 = 192;
const GRID_THUMBNAIL_HEIGHT: u32 = 128;
const MAX_GRID_THUMBNAIL_BYTES: usize = 256 * 1024;
pub(crate) const MAX_GRID_SOURCE_DIMENSION: u32 = 4_096;
pub(crate) const MAX_GRID_DECODED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CONCURRENT_GRID_THUMBNAILS: usize = 2;

#[cfg(test)]
tokio::task_local! {
    static TEST_FIXTURE_EXECUTION_ENABLED: bool;
}

/// 生产运行只接受显式环境开关；测试可在当前异步任务内安全启用夹具。
fn fixture_execution_enabled() -> bool {
    #[cfg(test)]
    if TEST_FIXTURE_EXECUTION_ENABLED
        .try_with(|enabled| *enabled)
        .unwrap_or(false)
    {
        return true;
    }
    fixture_mode_enabled()
}

/// 当前活动批次的控制信号和受管任务句柄。
struct ActiveBatch {
    id: String,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// 在一个批次内绑定不可变配置修订和密钥值。
#[derive(Clone)]
struct FrozenProvider {
    config: ProviderConfig,
    credential_ref: Option<String>,
    api_key: Option<String>,
}

/// 冻结一次批次使用的模板和非秘密 Provider 配置。
#[derive(Clone)]
pub(crate) struct ExecutionSnapshot {
    prompt_renderer_version: u32,
    image_request_contract: ImageRequestContract,
    article_template: PromptTemplate,
    summary_template: PromptTemplate,
    image_template: PromptTemplate,
    article_provider: Option<FrozenProvider>,
    summary_provider: Option<FrozenProvider>,
    image_provider: Option<FrozenProvider>,
}

impl ExecutionSnapshot {
    /// 只把非秘密冻结配置交给 Core 选择模型、Prompt 与幂等材料。
    fn to_plan(&self) -> ExecutionPlan {
        let bind = |provider: &FrozenProvider| ProviderExecutionBinding {
            config: provider.config.clone(),
            credential_ref: provider.credential_ref.clone(),
        };
        ExecutionPlan {
            prompt_renderer_version: self.prompt_renderer_version,
            image_request_contract: self.image_request_contract,
            article_template: self.article_template.clone(),
            summary_template: self.summary_template.clone(),
            image_template: self.image_template.clone(),
            article_provider: self.article_provider.as_ref().map(bind),
            summary_provider: self.summary_provider.as_ref().map(bind),
            image_provider: self.image_provider.as_ref().map(bind),
        }
    }

    /// 新运行必须同时具备三个已启用的默认 AI 服务绑定。
    fn validate_for_new_run(&self) -> Result<(), String> {
        validate_execution_providers(
            self.article_provider.is_some(),
            self.summary_provider.is_some(),
            self.image_provider.is_some(),
            None,
        )
    }

    /// 单步运行只要求目标 AI 存在；格式转换完全在本地执行。
    fn validate_for_stage(&self, stage: WorkflowStage) -> Result<(), String> {
        validate_execution_providers(
            self.article_provider.is_some(),
            self.summary_provider.is_some(),
            self.image_provider.is_some(),
            Some(stage),
        )
    }

    /// 后续单步沿用文章包首次冻结的 renderer，避免同包历史与失败重试发生语义漂移。
    fn use_prompt_renderer_version(&mut self, version: u32) -> Result<(), String> {
        validate_prompt_renderer_version(version)?;
        self.prompt_renderer_version = version;
        Ok(())
    }

    /// 从批次开始时的工作区状态创建不可变快照。
    fn from_workspace(
        workspace: &WorkspaceData,
        credential_vault: &CredentialVault,
    ) -> Result<Self, String> {
        let allowed_kind = if fixture_mode_enabled() {
            ProviderKind::Fixture
        } else {
            ProviderKind::OpenAiCompatible
        };
        Self::from_workspace_with_loader(workspace, allowed_kind, |credential_ref| {
            credential_vault
                .get_api_key(credential_ref)
                .map_err(|error| error.safe_message)
        })
    }

    /// 用显式运行模式和凭据加载器建立快照，避免测试修改进程环境。
    fn from_workspace_with_loader(
        workspace: &WorkspaceData,
        allowed_kind: ProviderKind,
        load_credential: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::from_workspace_for_stage_with_loader(workspace, allowed_kind, None, load_credential)
    }

    /// 只冻结目标阶段需要的 AI 服务；本地格式阶段不会读取任何凭据。
    fn from_workspace_for_stage_with_loader(
        workspace: &WorkspaceData,
        allowed_kind: ProviderKind,
        target_stage: Option<WorkflowStage>,
        load_credential: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let plan = ExecutionPlan::from_workspace(workspace, allowed_kind, target_stage)?;
        Self::from_plan_with_loader(plan, load_credential)
    }

    /// 从文章包已经持久化的绑定恢复执行快照，普通重试不得读取最新配置。
    fn from_binding_with_loader(
        binding: &PackageExecutionBinding,
        allowed_kind: ProviderKind,
        load_credential: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let plan = ExecutionPlan::from_binding(binding, allowed_kind)?;
        Self::from_plan_with_loader(plan, load_credential)
    }

    /// 宿主仅按 Core 计划读取凭据，同一引用最多访问一次系统凭据库。
    fn from_plan_with_loader(
        plan: ExecutionPlan,
        mut load_credential: impl FnMut(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        let mut credential_cache = BTreeMap::<String, Result<String, String>>::new();
        let mut freeze = |bound: &ProviderExecutionBinding| -> Result<FrozenProvider, String> {
            let api_key = bound
                .credential_ref
                .as_deref()
                .map(|credential_ref| {
                    credential_cache
                        .entry(credential_ref.to_string())
                        .or_insert_with(|| load_credential(credential_ref))
                        .clone()
                })
                .transpose()?;
            Ok(FrozenProvider {
                config: bound.config.clone(),
                credential_ref: bound.credential_ref.clone(),
                api_key,
            })
        };
        Ok(Self {
            prompt_renderer_version: plan.prompt_renderer_version,
            image_request_contract: plan.image_request_contract,
            article_template: plan.article_template,
            summary_template: plan.summary_template,
            image_template: plan.image_template,
            article_provider: plan
                .article_provider
                .as_ref()
                .map(&mut freeze)
                .transpose()?,
            summary_provider: plan
                .summary_provider
                .as_ref()
                .map(&mut freeze)
                .transpose()?,
            image_provider: plan.image_provider.as_ref().map(freeze).transpose()?,
        })
    }

    /// 生成不含密钥值的持久执行绑定。
    pub(crate) fn to_binding(
        &self,
        task_id: impl Into<String>,
        package_version: u32,
    ) -> PackageExecutionBinding {
        let persist_provider = |provider: &FrozenProvider| ProviderExecutionBinding {
            config: provider.config.clone(),
            credential_ref: provider.credential_ref.clone(),
        };
        PackageExecutionBinding {
            task_id: task_id.into(),
            package_version,
            prompt_renderer_version: self.prompt_renderer_version,
            image_request_contract: Some(self.image_request_contract),
            article_template: self.article_template.clone(),
            summary_template: self.summary_template.clone(),
            image_template: self.image_template.clone(),
            article_provider: self.article_provider.as_ref().map(persist_provider),
            summary_provider: self.summary_provider.as_ref().map(persist_provider),
            image_provider: self.image_provider.as_ref().map(persist_provider),
        }
    }
}

/// 拥有至多一个活动批次，并提供暂停、恢复、取消和退出回收。
pub struct BatchWorker {
    repository: Arc<WorkspaceRepository>,
    credential_vault: CredentialVault,
    provider_client: OpenAiCompatibleClient,
    asset_dir: PathBuf,
    active: Mutex<Option<ActiveBatch>>,
    start_lock: tokio::sync::Mutex<()>,
    control_lock: tokio::sync::Mutex<()>,
    provider_throttle: Arc<tokio::sync::Mutex<BTreeMap<String, Instant>>>,
    thumbnail_slots: tokio::sync::Semaphore,
}

impl BatchWorker {
    /// 创建一个尚未启动批次的 worker。
    pub fn new(repository: Arc<WorkspaceRepository>, asset_dir: PathBuf) -> Result<Self, String> {
        Ok(Self {
            repository,
            credential_vault: CredentialVault,
            provider_client: OpenAiCompatibleClient::new()?,
            asset_dir,
            active: Mutex::new(None),
            start_lock: tokio::sync::Mutex::new(()),
            control_lock: tokio::sync::Mutex::new(()),
            provider_throttle: Arc::new(tokio::sync::Mutex::new(BTreeMap::new())),
            thumbnail_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_GRID_THUMBNAILS),
        })
    }

    /// 串行化“检查 worker→持久排队→启动”完整临界区。
    pub async fn acquire_start(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.start_lock.lock().await
    }

    /// 只向同一适配器内的本地图片服务开放权威仓储。
    pub(crate) fn repository(&self) -> &Arc<WorkspaceRepository> {
        &self.repository
    }

    /// 只向同一适配器内的本地图片服务开放受管目录。
    pub(crate) fn asset_dir(&self) -> &Path {
        &self.asset_dir
    }

    /// 与批量生成共享同一 Provider 请求时间窗。
    pub(crate) async fn acquire_provider_slot(&self, provider_id: &str) {
        wait_for_provider_slot(&self.provider_throttle, provider_id).await;
    }

    /// 收到 Provider 的 Retry-After 后推迟该 Provider 的后续请求。
    pub(crate) async fn defer_provider_slot(&self, provider_id: &str, retry_after_ms: u64) {
        defer_provider_slot(&self.provider_throttle, provider_id, retry_after_ms).await;
    }

    /// 在任何任务进入队列前冻结模板、Provider 修订和密钥。
    pub(crate) async fn prepare_execution(&self) -> Result<ExecutionSnapshot, CommandError> {
        let workspace = self.repository.snapshot().await;
        self.validate_execution_configuration(&workspace)?;
        let snapshot = ExecutionSnapshot::from_workspace(&workspace, &self.credential_vault)
            .map_err(|message| CommandError::new("BATCH_PREPARE_FAILED", message))?;
        snapshot.validate_for_new_run()
            .map_err(|message| CommandError::new("BATCH_PREPARE_FAILED", message))?;
        Ok(snapshot)
    }

    /// 从同一乐观锁快照只准备一个目标阶段的执行契约。
    pub(crate) fn prepare_stage_execution(
        &self,
        workspace: &WorkspaceData,
        stage: WorkflowStage,
        inherited_prompt_renderer_version: Option<u32>,
    ) -> Result<ExecutionSnapshot, String> {
        let allowed_kind = if fixture_mode_enabled() {
            ProviderKind::Fixture
        } else {
            ProviderKind::OpenAiCompatible
        };
        let mut snapshot = ExecutionSnapshot::from_workspace_for_stage_with_loader(
            workspace,
            allowed_kind,
            Some(stage),
            |credential_ref| {
                self.credential_vault
                    .get_api_key(credential_ref)
                    .map_err(|error| error.safe_message)
            },
        )?;
        if let Some(version) = inherited_prompt_renderer_version {
            snapshot.use_prompt_renderer_version(version)?;
        }
        snapshot.validate_for_stage(stage)?;
        Ok(snapshot)
    }

    /// 只根据当前工作区的非秘密配置预检新批次，不读取或解密系统凭据库。
    pub(crate) fn validate_execution_configuration(
        &self,
        workspace: &WorkspaceData,
    ) -> Result<(), CommandError> {
        let allowed_kind = if fixture_mode_enabled() {
            ProviderKind::Fixture
        } else {
            ProviderKind::OpenAiCompatible
        };
        validate_execution_configuration_for_kind(workspace, allowed_kind)
            .map_err(|error| CommandError::new(error.code(), error.to_string()))
    }

    /// 从指定文章包的持久绑定恢复 Prompt、Provider 修订与凭据值。
    pub(crate) fn prepare_bound_execution(
        &self,
        binding: &PackageExecutionBinding,
    ) -> Result<ExecutionSnapshot, String> {
        let allowed_kind = if fixture_mode_enabled() {
            ProviderKind::Fixture
        } else {
            ProviderKind::OpenAiCompatible
        };
        ExecutionSnapshot::from_binding_with_loader(binding, allowed_kind, |credential_ref| {
            self.credential_vault
                .get_api_key(credential_ref)
                .map_err(|error| error.safe_message)
        })
    }

    /// 判断是否已有未结束的批次。
    pub fn has_active_batch(&self) -> bool {
        self.active
            .lock()
            .expect("batch worker mutex should remain available")
            .as_ref()
            .is_some_and(|active| !active.finished.load(Ordering::Acquire))
    }

    /// 返回当前未结束批次 ID。
    pub fn active_batch_id(&self) -> Option<String> {
        self.active
            .lock()
            .expect("batch worker mutex should remain available")
            .as_ref()
            .filter(|active| !active.finished.load(Ordering::Acquire))
            .map(|active| active.id.clone())
    }

    /// 使用已经冻结的执行快照启动已持久排队的批次。
    pub(crate) fn start_prepared(
        &self,
        batch_id: String,
        task_ids: Vec<String>,
        executions: BTreeMap<String, ExecutionSnapshot>,
    ) -> Result<(), String> {
        self.start_prepared_with_scope(batch_id, task_ids, executions, None)
    }

    /// 启动只允许领取一个目标阶段的持久单步运行。
    pub(crate) fn start_prepared_stage(
        &self,
        batch_id: String,
        task_id: String,
        execution: ExecutionSnapshot,
        stage: WorkflowStage,
    ) -> Result<(), String> {
        self.start_prepared_with_scope(
            batch_id,
            vec![task_id.clone()],
            BTreeMap::from([(task_id, execution)]),
            Some(stage),
        )
    }

    fn start_prepared_with_scope(
        &self,
        batch_id: String,
        task_ids: Vec<String>,
        executions: BTreeMap<String, ExecutionSnapshot>,
        target_stage: Option<WorkflowStage>,
    ) -> Result<(), String> {
        if task_ids.is_empty() {
            return Err("批次没有可运行任务".to_string());
        }
        if task_ids
            .iter()
            .any(|task_id| !executions.contains_key(task_id))
        {
            return Err("批次缺少文章包执行绑定".to_string());
        }
        let mut active = self
            .active
            .lock()
            .expect("batch worker mutex should remain available");
        if active
            .as_ref()
            .is_some_and(|current| !current.finished.load(Ordering::Acquire))
        {
            return Err("已有批次正在运行".to_string());
        }

        let paused = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let repository = Arc::clone(&self.repository);
        let provider_client = self.provider_client.clone();
        let asset_dir = self.asset_dir.clone();
        let provider_throttle = Arc::clone(&self.provider_throttle);
        let worker_batch_id = batch_id.clone();
        let worker_paused = Arc::clone(&paused);
        let worker_cancelled = Arc::clone(&cancelled);
        let worker_finished = Arc::clone(&finished);
        let handle = tauri::async_runtime::spawn(async move {
            run_batch(
                repository,
                worker_batch_id,
                task_ids,
                executions,
                provider_client,
                asset_dir,
                provider_throttle,
                worker_paused,
                worker_cancelled,
                target_stage,
            )
            .await;
            worker_finished.store(true, Ordering::Release);
        });
        *active = Some(ActiveBatch {
            id: batch_id,
            paused,
            cancelled,
            finished,
            handle,
        });
        Ok(())
    }

    /// 暂停领取后续阶段；正在等待的外部请求会有界完成。
    pub async fn pause(&self) -> Result<String, String> {
        let _control_guard = self.control_lock.lock().await;
        let (batch_id, paused) = {
            let active = self
                .active
                .lock()
                .expect("batch worker mutex should remain available");
            let active = active
                .as_ref()
                .filter(|active| !active.finished.load(Ordering::Acquire))
                .ok_or_else(|| "当前没有运行中的批次".to_string())?;
            if active.cancelled.load(Ordering::Acquire) {
                return Err("批次已在取消中".to_string());
            }
            active.paused.store(true, Ordering::Release);
            (active.id.clone(), Arc::clone(&active.paused))
        };
        if let Err(error) = transition_controlled_run_status(
            &self.repository,
            &batch_id,
            RunStatus::Paused,
            &[RunStatus::Queued, RunStatus::Running],
        )
        .await
        {
            paused.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(batch_id)
    }

    /// 恢复已暂停批次继续领取阶段。
    pub async fn resume(&self) -> Result<String, String> {
        let _control_guard = self.control_lock.lock().await;
        let (batch_id, paused) = {
            let active = self
                .active
                .lock()
                .expect("batch worker mutex should remain available");
            let active = active
                .as_ref()
                .filter(|active| !active.finished.load(Ordering::Acquire))
                .ok_or_else(|| "当前没有可恢复的批次".to_string())?;
            if active.cancelled.load(Ordering::Acquire) {
                return Err("批次已在取消中".to_string());
            }
            (active.id.clone(), Arc::clone(&active.paused))
        };
        transition_controlled_run_status(
            &self.repository,
            &batch_id,
            RunStatus::Running,
            &[RunStatus::Paused],
        )
        .await?;
        paused.store(false, Ordering::Release);
        Ok(batch_id)
    }

    /// 请求取消活动批次，已成功产物仍被保留。
    pub async fn cancel(&self) -> Result<String, String> {
        let _control_guard = self.control_lock.lock().await;
        let batch_id = {
            let active = self
                .active
                .lock()
                .expect("batch worker mutex should remain available");
            let active = active
                .as_ref()
                .filter(|active| !active.finished.load(Ordering::Acquire))
                .ok_or_else(|| "当前没有可取消的批次".to_string())?;
            active.cancelled.store(true, Ordering::Release);
            active.paused.store(false, Ordering::Release);
            active.id.clone()
        };
        Ok(batch_id)
    }

    /// 应用退出时取消并中止仍由本 worker 拥有的任务。
    pub fn shutdown(&self) {
        if let Some(active) = self
            .active
            .lock()
            .expect("batch worker mutex should remain available")
            .take()
        {
            active.cancelled.store(true, Ordering::Release);
            active.handle.abort();
        }
    }
}

mod execution;
mod image;
mod run;
mod throttle;

#[cfg(test)]
mod tests;

use execution::*;
use image::*;
pub(crate) use image::{validated_image_format, validated_image_generation_preview};
use run::*;
use throttle::*;
