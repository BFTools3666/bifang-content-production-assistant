//! 对标功能的Tauri协议、可见采集窗口和原有AI服务适配。

pub(crate) mod collection;
pub(crate) mod commands;

use std::sync::Mutex;

use serde::Serialize;

/// 一次可见采集任务的非秘密进度，供主窗口轮询展示。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionStatus {
    /// 正在访问的公开页面。
    pub page_url: String,
    /// 最近一次明确结果或恢复提示。
    pub message: String,
    /// 本次已保存的作品数。
    pub collected_count: usize,
    /// 最近一次结果时间，作为界面刷新依据。
    pub updated_at_ms: i64,
}

/// 宿主拥有的采集状态与生成互斥；不持久化Cookie或密钥。
#[derive(Default)]
pub(crate) struct ResearchRuntime {
    /// 可见采集状态。
    pub collection: Mutex<CollectionStatus>,
    /// 阻止重复点击产生并发付费AI请求。
    pub generation: tokio::sync::Mutex<()>,
}
