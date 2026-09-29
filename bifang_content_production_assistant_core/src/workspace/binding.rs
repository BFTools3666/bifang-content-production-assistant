//! 文章包冻结绑定的查找、授权和原子替换规则。

use super::{ArticleTask, PackageExecutionBinding, WorkflowStage, WorkspaceData};

/// 当前 Provider 状态不允许继续执行历史冻结绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingProviderError {
    /// 冻结的 Provider 稳定 ID 已不在当前配置中。
    NotConfigured,
    /// 冻结的 Provider 当前已停用。
    Disabled,
}

impl BindingProviderError {
    /// 稳定机器错误码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotConfigured => "PROVIDER_NOT_CONFIGURED",
            Self::Disabled => "PROVIDER_DISABLED",
        }
    }

    /// 不含密钥或本机路径的用户提示。
    pub const fn message(self) -> &'static str {
        match self {
            Self::NotConfigured => "文章包绑定的 AI 服务已不存在，不能重试",
            Self::Disabled => "文章包绑定的 AI 服务已停用，请先启用后再重试",
        }
    }
}

/// 取得指定文章的可变引用。
pub fn find_article_mut<'a>(
    workspace: &'a mut WorkspaceData,
    task_id: &str,
) -> Result<&'a mut ArticleTask, String> {
    workspace
        .articles
        .iter_mut()
        .find(|article| article.id == task_id)
        .ok_or_else(|| "找不到指定文章".to_string())
}

/// 读取一篇文章指定包版本已经冻结的执行绑定。
pub fn find_execution_binding<'a>(
    workspace: &'a WorkspaceData,
    task_id: &str,
    package_version: u32,
) -> Option<&'a PackageExecutionBinding> {
    workspace
        .execution_bindings
        .iter()
        .rev()
        .find(|binding| binding.task_id == task_id && binding.package_version == package_version)
}

/// 普通重试沿用冻结配置，但必须由当前同一稳定 ID 的启用状态重新授权。
pub fn ensure_binding_providers_enabled(
    workspace: &WorkspaceData,
    binding: &PackageExecutionBinding,
) -> Result<(), BindingProviderError> {
    for bound in [
        binding.article_provider.as_ref(),
        binding.summary_provider.as_ref(),
        binding.image_provider.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let current = workspace
            .provider_configs
            .iter()
            .find(|provider| provider.id == bound.config.id)
            .ok_or(BindingProviderError::NotConfigured)?;
        if !current.enabled {
            return Err(BindingProviderError::Disabled);
        }
    }
    Ok(())
}

/// 原子替换同一文章包版本的执行绑定，其他历史版本继续保留。
pub fn upsert_execution_binding(workspace: &mut WorkspaceData, binding: PackageExecutionBinding) {
    workspace.execution_bindings.retain(|current| {
        current.task_id != binding.task_id || current.package_version != binding.package_version
    });
    workspace.execution_bindings.push(binding);
}

/// 只替换目标阶段的冻结配置，避免同包其他单步结果被最新默认配置覆盖。
pub fn merge_stage_execution_binding(
    current: &mut PackageExecutionBinding,
    proposed: &PackageExecutionBinding,
    stage: WorkflowStage,
) {
    match stage {
        WorkflowStage::Article => {
            current.article_template = proposed.article_template.clone();
            current.article_provider = proposed.article_provider.clone();
        }
        WorkflowStage::Summary => {
            current.summary_template = proposed.summary_template.clone();
            current.summary_provider = proposed.summary_provider.clone();
        }
        WorkflowStage::Image => {
            current.image_template = proposed.image_template.clone();
            current.image_provider = proposed.image_provider.clone();
            current.image_request_contract = proposed.image_request_contract;
        }
        WorkflowStage::Format => {}
    }
}

/// 新包只继承仍有效阶段；同包首次执行则保留已冻结的其他单步绑定。
pub fn single_stage_execution_binding(
    existing: Option<PackageExecutionBinding>,
    proposed: &PackageExecutionBinding,
    stage: WorkflowStage,
    reusing_current_package: bool,
) -> PackageExecutionBinding {
    let mut binding = if reusing_current_package {
        existing.unwrap_or_else(|| proposed.clone())
    } else {
        match stage {
            WorkflowStage::Article => proposed.clone(),
            WorkflowStage::Summary => {
                let mut inherited = existing.unwrap_or_else(|| proposed.clone());
                inherited.image_provider = None;
                inherited
            }
            WorkflowStage::Image | WorkflowStage::Format => {
                existing.unwrap_or_else(|| proposed.clone())
            }
        }
    };
    merge_stage_execution_binding(&mut binding, proposed, stage);
    binding
}

/// 判断文章包冻结绑定是否仍引用指定系统凭据条目。
pub fn execution_binding_uses_credential(
    binding: &PackageExecutionBinding,
    credential_ref: &str,
) -> bool {
    [
        binding.article_provider.as_ref(),
        binding.summary_provider.as_ref(),
        binding.image_provider.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|provider| provider.credential_ref.as_deref() == Some(credential_ref))
}
