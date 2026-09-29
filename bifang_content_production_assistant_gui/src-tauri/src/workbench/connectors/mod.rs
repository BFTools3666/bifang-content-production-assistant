//! 云文档发布目标的命令、遗留凭据清理和官方飞书 CLI 适配器。

pub(crate) mod commands;
mod credentials;
mod lark_cli;

#[cfg(test)]
mod tests;

pub use commands::{ConnectorRuntime, reconcile_pending_connector_credential_deletions};
