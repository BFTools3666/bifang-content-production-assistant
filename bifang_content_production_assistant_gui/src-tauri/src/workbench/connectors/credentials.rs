//! v11 迁移后仅用于幂等清理遗留飞书 App Secret 的系统凭据库边界。

const KEYRING_SERVICE: &str = "com.arden.bifang-content-production-assistant.connector";

/// 只在 Rust 侧清理遗留飞书 App Secret 的独立凭据库适配器。
#[derive(Debug, Clone, Default)]
pub(super) struct ConnectorCredentialVault;

impl ConnectorCredentialVault {
    /// 幂等删除不再被当前连接器引用的秘密。
    pub(super) fn delete_app_secret(&self, credential_ref: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, credential_ref)
            .map_err(|_| "无法打开连接器系统凭据库".to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法清理未使用的连接器系统凭据".to_string()),
        }
    }
}
