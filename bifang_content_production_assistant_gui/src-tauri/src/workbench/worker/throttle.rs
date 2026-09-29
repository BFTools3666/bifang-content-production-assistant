use super::*;

/// 对同一 Provider 的请求施加最小间隔，首版并发上限仍为一。
pub(super) async fn wait_for_provider_slot(
    throttle: &tokio::sync::Mutex<BTreeMap<String, Instant>>,
    provider_id: &str,
) {
    loop {
        let delay = {
            let mut next_slots = throttle.lock().await;
            let now = Instant::now();
            match next_slots.get(provider_id).copied() {
                Some(next) if next > now => Some(next - now),
                _ => {
                    next_slots.insert(provider_id.to_string(), now + Duration::from_millis(150));
                    None
                }
            }
        };
        let Some(delay) = delay else {
            return;
        };
        tokio::time::sleep(delay).await;
    }
}

/// 等待批量 Provider 时间窗时持续响应暂停/取消，返回即定义外部请求 in-flight 边界。
pub(super) async fn wait_for_provider_slot_controlled(
    throttle: &tokio::sync::Mutex<BTreeMap<String, Instant>>,
    provider_id: &str,
    paused: &AtomicBool,
    cancelled: &AtomicBool,
) -> bool {
    loop {
        if !wait_until_execution_allowed(paused, cancelled).await {
            return false;
        }
        let delay = {
            let mut next_slots = throttle.lock().await;
            let now = Instant::now();
            match next_slots.get(provider_id).copied() {
                Some(next) if next > now => Some((next - now).min(Duration::from_millis(100))),
                _ => {
                    next_slots.insert(provider_id.to_string(), now + Duration::from_millis(150));
                    None
                }
            }
        };
        match delay {
            Some(delay) => tokio::time::sleep(delay).await,
            None => {
                if cancelled.load(Ordering::Acquire) {
                    return false;
                }
                if paused.load(Ordering::Acquire) {
                    if !wait_until_execution_allowed(paused, cancelled).await {
                        return false;
                    }
                    // 暂停期间连接测试或上一请求可能延长 Retry-After；恢复后重读。
                    continue;
                }
                // 成功预约 Provider 时刻即为这次外部请求的 in-flight 边界。
                return true;
            }
        }
    }
}

/// 暂停期间不开始新工作；取消在真正出站前失败关闭。
pub(super) async fn wait_until_execution_allowed(
    paused: &AtomicBool,
    cancelled: &AtomicBool,
) -> bool {
    while paused.load(Ordering::Acquire) && !cancelled.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    !cancelled.load(Ordering::Acquire)
}

/// 构造不进入普通失败统计的受控取消结果。
pub(super) fn controlled_cancelled_error() -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::Cancelled,
        safe_message: "AI 请求已取消".into(),
        retry_after_ms: None,
    }
}

/// 延长 Provider 的下一可用时间，且不会缩短已有的更晚限制。
pub(super) async fn defer_provider_slot(
    throttle: &tokio::sync::Mutex<BTreeMap<String, Instant>>,
    provider_id: &str,
    retry_after_ms: u64,
) {
    let proposed = Instant::now() + Duration::from_millis(retry_after_ms.min(60_000));
    let mut next_slots = throttle.lock().await;
    next_slots
        .entry(provider_id.to_string())
        .and_modify(|next| {
            if *next < proposed {
                *next = proposed;
            }
        })
        .or_insert(proposed);
}
