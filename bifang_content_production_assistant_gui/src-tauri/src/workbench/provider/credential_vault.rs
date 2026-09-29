use super::*;

/// 只在 Rust 侧读写 API 密钥的系统凭据库适配器。
#[derive(Debug, Clone, Default)]
pub struct CredentialVault;

impl CredentialVault {
    /// 将一个非空 API 密钥写入当前用户的系统凭据库。
    pub fn set_api_key(&self, provider_id: &str, api_key: &str) -> Result<(), String> {
        if api_key.trim().is_empty() {
            return Err("API 密钥不能为空".to_string());
        }
        let entry = keyring::Entry::new(KEYRING_SERVICE, provider_id)
            .map_err(|_| "无法打开系统凭据库".to_string())?;
        entry
            .set_password(api_key)
            .map_err(|_| "无法保存 API 密钥到系统凭据库".to_string())
    }

    /// 从系统凭据库读取密钥；错误不会返回凭据或底层载荷。
    pub fn get_api_key(&self, provider_id: &str) -> Result<String, ProviderError> {
        let entry =
            keyring::Entry::new(KEYRING_SERVICE, provider_id).map_err(|_| ProviderError {
                kind: ProviderErrorKind::NotConfigured,
                safe_message: "无法访问系统凭据库中的 AI 密钥".into(),
                retry_after_ms: None,
            })?;
        entry.get_password().map_err(|_| ProviderError {
            kind: ProviderErrorKind::NotConfigured,
            safe_message: "尚未为该 AI 服务保存 API 密钥".into(),
            retry_after_ms: None,
        })
    }

    /// 区分“确实不存在”和“系统凭据库不可访问”，供地址轮换门禁失败关闭。
    pub fn api_key_presence(&self, credential_ref: &str) -> Result<bool, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, credential_ref)
            .map_err(|_| "无法打开系统凭据库".to_string())?;
        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(_) => Err("无法检查系统凭据库中的 AI 密钥".to_string()),
        }
    }

    /// 删除尚未由持久配置引用的凭据条目；错误只返回安全分类。
    pub fn delete_api_key(&self, credential_ref: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, credential_ref)
            .map_err(|_| "无法打开系统凭据库".to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法清理未使用的系统凭据".to_string()),
        }
    }
}
