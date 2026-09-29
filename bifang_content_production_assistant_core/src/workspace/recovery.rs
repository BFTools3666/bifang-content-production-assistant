//! 打开持久工作区时恢复失去 worker 所有权的文章和批次。

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// Ark 运行中配图因无幂等保证保持不可普通重试，其余阶段按旧策略恢复。
pub fn recover_interrupted_articles(data: &mut WorkspaceData, now_ms: i64) -> usize {
    let ambiguous_ark_images = data
        .execution_bindings
        .iter()
        .filter(|binding| {
            binding.image_request_contract == Some(ImageRequestContract::VolcengineArkUrlV1)
        })
        .map(|binding| (binding.task_id.clone(), binding.package_version))
        .collect::<BTreeSet<_>>();
    let single_stage_scopes = data
        .run_history
        .iter()
        .filter(|run| {
            matches!(
                run.status,
                RunStatus::Queued | RunStatus::Running | RunStatus::Paused
            )
        })
        .filter_map(|run| run.target_stage.map(|stage| (&run.task_ids, stage)))
        .flat_map(|(task_ids, stage)| {
            task_ids
                .iter()
                .cloned()
                .map(move |task_id| (task_id, stage))
        })
        .collect::<BTreeMap<_, _>>();

    data.articles
        .iter_mut()
        .map(|article| {
            let ark_image_has_no_idempotency =
                ambiguous_ark_images.contains(&(article.id.clone(), article.package_version));
            usize::from(
                if let Some(stage) = single_stage_scopes.get(&article.id).copied() {
                    article.recover_interrupted_stage_with_policy(
                        stage,
                        now_ms,
                        stage != WorkflowStage::Image || !ark_image_has_no_idempotency,
                    )
                } else {
                    article.recover_interrupted_with_policy(now_ms, |stage| {
                        stage != WorkflowStage::Image || !ark_image_has_no_idempotency
                    })
                },
            )
        })
        .sum()
}

/// 终结失去 worker 所有权的批次记录，按现有文章终态保存计数。
pub fn recover_abandoned_runs(data: &mut WorkspaceData, now_ms: i64) -> usize {
    let mut recovered = 0;
    for run in &mut data.run_history {
        if !matches!(
            run.status,
            RunStatus::Queued | RunStatus::Running | RunStatus::Paused
        ) {
            continue;
        }
        let statuses = run
            .task_ids
            .iter()
            .map(|task_id| {
                data.articles
                    .iter()
                    .find(|article| article.id == *task_id)
                    .map(|article| article.overall_status())
            })
            .collect::<Vec<_>>();
        if let Some(stage) = run.target_stage {
            let target_status = run
                .task_ids
                .first()
                .and_then(|task_id| data.articles.iter().find(|article| article.id == *task_id))
                .map(|article| article.stages.get(stage).status);
            run.completed_count = usize::from(target_status == Some(StageStatus::Succeeded));
            run.failed_count = usize::from(matches!(
                target_status,
                Some(StageStatus::Failed | StageStatus::Interrupted | StageStatus::Stale)
            ));
            run.status = target_status
                .and_then(|status| terminal_stage_run_status(status).ok())
                .unwrap_or(RunStatus::PartialFailure);
            run.finished_at_ms = Some(now_ms);
            recovered += 1;
            continue;
        }
        run.completed_count = statuses
            .iter()
            .filter(|status| {
                matches!(
                    status,
                    Some(OverallStatus::PendingReview | OverallStatus::Approved)
                )
            })
            .count();
        run.failed_count = statuses
            .iter()
            .filter(|status| {
                matches!(
                    status,
                    Some(OverallStatus::PartialFailure | OverallStatus::Interrupted)
                )
            })
            .count();
        run.status = if statuses.iter().all(Option::is_some) {
            terminal_run_status(&statuses.iter().flatten().copied().collect::<Vec<_>>())
                .unwrap_or(RunStatus::PartialFailure)
        } else {
            RunStatus::PartialFailure
        };
        run.finished_at_ms = Some(now_ms);
        recovered += 1;
    }
    recovered
}
