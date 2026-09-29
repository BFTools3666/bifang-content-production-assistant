use super::*;

/// Retry-After 会延长共享 Provider 时间窗，后续行与连接测试都复用该状态。
#[tokio::test]
async fn retry_after_extends_provider_throttle() {
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    wait_for_provider_slot(&throttle, "provider").await;
    let before = Instant::now();

    defer_provider_slot(&throttle, "provider", 2_000).await;

    let next = throttle.lock().await["provider"];
    assert!(next >= before + Duration::from_millis(1_900));
    assert!(next <= before + Duration::from_millis(2_100));
}

/// 暂停或取消在阶段领取闭包执行前生效时，不创建尝试也不开始工作。
#[tokio::test]
async fn stage_claim_rechecks_control_flags_atomically() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.queue_initial(2).unwrap();
            workspace.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    let paused = AtomicBool::new(true);
    let cancelled = AtomicBool::new(false);

    assert!(matches!(
        claim_next_stage(&repository, "task", &paused, &cancelled, None)
            .await
            .unwrap(),
        StageClaim::Paused
    ));
    let snapshot = repository.snapshot().await;
    assert_eq!(snapshot.articles[0].stages.article.attempts, 0);

    paused.store(false, Ordering::Release);
    cancelled.store(true, Ordering::Release);
    assert!(matches!(
        claim_next_stage(&repository, "task", &paused, &cancelled, None)
            .await
            .unwrap(),
        StageClaim::Cancelled
    ));
    assert_eq!(
        repository.snapshot().await.articles[0]
            .stages
            .article
            .attempts,
        0
    );
}

/// Provider 限流等待期间的取消会立即阻止后续出站边界。
#[tokio::test]
async fn controlled_provider_wait_stops_before_outbound_on_cancel() {
    let throttle = tokio::sync::Mutex::new(BTreeMap::from([(
        "provider".to_string(),
        Instant::now() + Duration::from_secs(30),
    )]));
    let paused = AtomicBool::new(false);
    let cancelled = AtomicBool::new(true);

    assert!(!wait_for_provider_slot_controlled(&throttle, "provider", &paused, &cancelled,).await);
}

/// 暂停期间新增的 Retry-After 必须在恢复后重新读取，不能沿用过期预约。
#[tokio::test]
async fn controlled_provider_wait_rechecks_deadline_after_pause() {
    let throttle = tokio::sync::Mutex::new(BTreeMap::new());
    let paused = AtomicBool::new(true);
    let cancelled = AtomicBool::new(false);
    let started = Instant::now();
    let waiter = wait_for_provider_slot_controlled(&throttle, "provider", &paused, &cancelled);
    let controller = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        defer_provider_slot(&throttle, "provider", 180).await;
        paused.store(false, Ordering::Release);
    };

    let (ready, ()) = tokio::join!(waiter, controller);

    assert!(ready);
    assert!(started.elapsed() >= Duration::from_millis(160));
}

/// 末尾取消不能覆盖已完成成功；成功与取消混合保持取消而非伪造失败。
#[test]
fn terminal_run_status_is_strict() {
    assert_eq!(
        terminal_run_status(&[OverallStatus::PendingReview, OverallStatus::Approved]).unwrap(),
        RunStatus::Succeeded
    );
    assert_eq!(
        terminal_run_status(&[OverallStatus::PendingReview, OverallStatus::PartialFailure,])
            .unwrap(),
        RunStatus::PartialFailure
    );
    assert_eq!(
        terminal_run_status(&[OverallStatus::PendingReview, OverallStatus::Cancelled]).unwrap(),
        RunStatus::Cancelled
    );
    assert_eq!(
        terminal_run_status(&[OverallStatus::Approved]).unwrap(),
        RunStatus::Succeeded
    );
    assert!(terminal_run_status(&[OverallStatus::Queued]).is_err());
}

/// 若仅最终批次记录写入失败，错误恢复必须保留所有完成行的成功终态。
#[tokio::test]
async fn internal_error_fallback_preserves_succeeded_terminal_state() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.review_status = bifang_content_production_assistant_core::ReviewStatus::Pending;
            workspace.articles.push(article);
            workspace.run_history.push(RunRecord {
                id: "batch".into(),
                task_ids: vec!["task".into()],
                target_stage: None,
                status: RunStatus::Running,
                started_at_ms: 1,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: false,
            });
            Ok(())
        })
        .await
        .unwrap();

    recover_batch_after_internal_error(&repository, "batch", &["task".into()], 2, None, true)
        .await
        .unwrap();

    let mut snapshot = repository.snapshot().await;
    let run = snapshot.run_history.remove(0);
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(run.completed_count, 1);
    assert_eq!(run.failed_count, 0);
    assert_eq!(run.finished_at_ms, Some(2));
}

/// 启动/暂停竞态不覆盖暂停或终态批次记录。
#[tokio::test]
async fn controlled_run_transitions_preserve_pause_and_terminal_states() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            workspace.run_history.push(RunRecord {
                id: "batch".into(),
                task_ids: Vec::new(),
                target_stage: None,
                status: RunStatus::Queued,
                started_at_ms: 1,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: false,
            });
            Ok(())
        })
        .await
        .unwrap();

    transition_controlled_run_status(
        &repository,
        "batch",
        RunStatus::Paused,
        &[RunStatus::Queued, RunStatus::Running],
    )
    .await
    .unwrap();
    mark_run_started(&repository, "batch").await.unwrap();
    assert_eq!(
        repository.snapshot().await.run_history[0].status,
        RunStatus::Paused
    );

    repository
        .update(|workspace| {
            workspace.run_history[0].status = RunStatus::Succeeded;
            workspace.run_history[0].finished_at_ms = Some(2);
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        transition_controlled_run_status(
            &repository,
            "batch",
            RunStatus::Running,
            &[RunStatus::Paused],
        )
        .await
        .is_err()
    );
    assert_eq!(
        repository.snapshot().await.run_history[0].status,
        RunStatus::Succeeded
    );
}

/// 并发暂停/恢复通过同一控制锁串行，持久状态与原子标志最终一致。
#[tokio::test]
async fn concurrent_pause_and_resume_keep_control_state_consistent() {
    let repository = Arc::new(
        WorkspaceRepository::open_database_url("sqlite::memory:")
            .await
            .unwrap(),
    );
    repository
        .update(|workspace| {
            workspace.run_history.push(RunRecord {
                id: "batch".into(),
                task_ids: Vec::new(),
                target_stage: None,
                status: RunStatus::Paused,
                started_at_ms: 1,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: false,
            });
            Ok(())
        })
        .await
        .unwrap();
    let worker = Arc::new(BatchWorker::new(Arc::clone(&repository), std::env::temp_dir()).unwrap());
    let paused = Arc::new(AtomicBool::new(true));
    let cancelled = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let handle = tauri::async_runtime::spawn(async {
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    *worker
        .active
        .lock()
        .expect("test worker mutex should remain available") = Some(ActiveBatch {
        id: "batch".into(),
        paused: Arc::clone(&paused),
        cancelled,
        finished,
        handle,
    });
    let resume_worker = Arc::clone(&worker);
    let pause_worker = Arc::clone(&worker);

    let (resume_result, pause_result) = tokio::join!(resume_worker.resume(), pause_worker.pause());

    assert!(resume_result.is_ok());
    assert!(pause_result.is_ok());
    let persisted_paused = repository.snapshot().await.run_history[0].status == RunStatus::Paused;
    assert_eq!(persisted_paused, paused.load(Ordering::Acquire));
    worker.shutdown();
}

/// 进度计数刷新不能用旧的原子信号覆盖已提交的暂停/恢复状态。
#[tokio::test]
async fn progress_refresh_preserves_committed_control_state() {
    let repository = WorkspaceRepository::open_database_url("sqlite::memory:")
        .await
        .unwrap();
    repository
        .update(|workspace| {
            workspace.run_history.push(RunRecord {
                id: "batch".into(),
                task_ids: Vec::new(),
                target_stage: None,
                status: RunStatus::Paused,
                started_at_ms: 1,
                finished_at_ms: None,
                completed_count: 0,
                failed_count: 0,
                is_test_fixture: false,
            });
            Ok(())
        })
        .await
        .unwrap();

    refresh_run_counts(&repository, "batch", RunStatus::Running, None)
        .await
        .unwrap();
    assert_eq!(
        repository.snapshot().await.run_history[0].status,
        RunStatus::Paused
    );
    repository
        .update(|workspace| {
            workspace.run_history[0].status = RunStatus::Running;
            Ok(())
        })
        .await
        .unwrap();
    refresh_run_counts(&repository, "batch", RunStatus::Paused, None)
        .await
        .unwrap();
    assert_eq!(
        repository.snapshot().await.run_history[0].status,
        RunStatus::Running
    );
}
