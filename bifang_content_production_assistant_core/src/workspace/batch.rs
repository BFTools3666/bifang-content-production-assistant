//! 批量工作区的原子排队用例；宿主只负责时间、持久化和 worker。

use std::collections::{BTreeMap, BTreeSet};

use super::{
    PackageExecutionBinding, WorkflowStage, WorkspaceData, find_article_mut, record_queued_run,
    upsert_execution_binding,
};
use crate::{BatchPreparation, prepare_retry_batch, prepare_selected_batch};

fn bind_current_package(
    workspace: &mut WorkspaceData,
    task_id: &str,
    template: &PackageExecutionBinding,
) -> Result<(), String> {
    let package_version = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .map(|article| article.package_version)
        .ok_or_else(|| "找不到指定文章".to_string())?;
    let mut binding = template.clone();
    binding.task_id = task_id.to_string();
    binding.package_version = package_version;
    upsert_execution_binding(workspace, binding);
    Ok(())
}

/// 将选中行、各行冻结绑定和运行历史一起写入同一工作区事务。
pub fn queue_selected_batch_in_workspace(
    workspace: &mut WorkspaceData,
    task_ids: Vec<String>,
    binding_template: &PackageExecutionBinding,
    batch_id: String,
    now_ms: i64,
    fixture_enabled: bool,
) -> Result<BatchPreparation, String> {
    let preparation = prepare_selected_batch(&mut workspace.articles, task_ids, now_ms);
    for task_id in &preparation.queued_task_ids {
        bind_current_package(workspace, task_id, binding_template)?;
    }
    if !preparation.queued_task_ids.is_empty() {
        record_queued_run(
            workspace,
            batch_id,
            preparation.queued_task_ids.clone(),
            now_ms,
            fixture_enabled,
        );
    }
    Ok(preparation)
}

/// 将单行最早失败阶段及历史绑定重新排队，不漂移到当前默认服务。
pub fn queue_article_retry_in_workspace(
    workspace: &mut WorkspaceData,
    task_id: &str,
    binding: &PackageExecutionBinding,
    batch_id: String,
    now_ms: i64,
    fixture_enabled: bool,
) -> Result<(), String> {
    find_article_mut(workspace, task_id)?
        .retry_failed(now_ms)
        .map_err(|error| error.to_string())?;
    bind_current_package(workspace, task_id, binding)?;
    record_queued_run(
        workspace,
        batch_id,
        vec![task_id.to_string()],
        now_ms,
        fixture_enabled,
    );
    Ok(())
}

/// 批量重试以预检的可运行集为乐观锁；所有绑定和历史在同一事务提交。
pub fn queue_retry_selected_in_workspace(
    workspace: &mut WorkspaceData,
    task_ids: Vec<String>,
    expected_queued_ids: &[String],
    bindings: &BTreeMap<String, PackageExecutionBinding>,
    batch_id: String,
    now_ms: i64,
    fixture_enabled: bool,
) -> Result<BatchPreparation, String> {
    // 只克隆选中行；Core 直接调用失败时也不应留下部分排队状态。
    let selected_ids = task_ids.iter().collect::<BTreeSet<_>>();
    let mut selected_articles = workspace
        .articles
        .iter()
        .filter(|article| selected_ids.contains(&article.id))
        .cloned()
        .collect::<Vec<_>>();
    let preparation = prepare_retry_batch(&mut selected_articles, task_ids, now_ms);
    if preparation.queued_task_ids != expected_queued_ids {
        return Err("文章状态已变化，请刷新后重试".into());
    }
    if expected_queued_ids
        .iter()
        .any(|task_id| !bindings.contains_key(task_id))
    {
        return Err("批次缺少文章包执行绑定".into());
    }
    let queued_ids = preparation.queued_task_ids.iter().collect::<BTreeSet<_>>();
    let mut queued_articles = selected_articles
        .into_iter()
        .filter(|article| queued_ids.contains(&article.id))
        .map(|article| (article.id.clone(), article))
        .collect::<BTreeMap<_, _>>();
    for article in &mut workspace.articles {
        if let Some(updated) = queued_articles.remove(&article.id) {
            *article = updated;
        }
    }
    for task_id in &preparation.queued_task_ids {
        let binding = bindings
            .get(task_id)
            .ok_or_else(|| "批次缺少文章包执行绑定".to_string())?;
        bind_current_package(workspace, task_id, binding)?;
    }
    if !preparation.queued_task_ids.is_empty() {
        record_queued_run(
            workspace,
            batch_id,
            preparation.queued_task_ids.clone(),
            now_ms,
            fixture_enabled,
        );
    }
    Ok(preparation)
}

/// 重新生成正文时创建新包并冻结当前模板和 Provider。
pub fn queue_article_regeneration_in_workspace(
    workspace: &mut WorkspaceData,
    task_id: &str,
    binding_template: &PackageExecutionBinding,
    batch_id: String,
    now_ms: i64,
    fixture_enabled: bool,
) -> Result<(), String> {
    find_article_mut(workspace, task_id)?
        .regenerate_stage(WorkflowStage::Article, now_ms)
        .map_err(|error| error.to_string())?;
    bind_current_package(workspace, task_id, binding_template)?;
    record_queued_run(
        workspace,
        batch_id,
        vec![task_id.to_string()],
        now_ms,
        fixture_enabled,
    );
    Ok(())
}
