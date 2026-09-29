//! 批量内容工作台的 Tauri 适配器、持久化和受管执行器。

pub(crate) mod ark_probe;
pub(crate) mod commands;
pub(crate) mod connectors;
mod image_editor;
mod models;
mod provider;
pub(crate) mod provider_lifecycle;
mod store;
mod worker;

pub(crate) use models::fixture_mode_enabled;
pub use store::WorkspaceRepository;
pub use worker::BatchWorker;
