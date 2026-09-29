//! 飞书云空间只读验证与 Markdown/HTML 发布。

use std::{future::Future, path::Path, time::Duration};

use tokio::time::Instant;

use super::{
    process::{LarkCliClient, OperationRisk},
    protocol::{LarkCliError, LarkCliUploadResult, extract_file_token},
    setup::remaining_timeout_ms,
};

impl LarkCliClient {
    /// 顺序验证 CLI 版本、用户身份和目标文件夹只读访问。
    pub(in crate::workbench::connectors) async fn verify_folder(
        &self,
        profile: &str,
        folder_token: &str,
        timeout_ms: u64,
    ) -> Result<String, LarkCliError> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let version = self
            .ensure_supported_version(remaining_timeout_ms(deadline)?)
            .await?;
        self.ensure_user_profile(profile, remaining_timeout_ms(deadline)?)
            .await?;
        self.ensure_folder_access(profile, folder_token, remaining_timeout_ms(deadline)?)
            .await?;
        Ok(version)
    }

    /// 用最小分页只读命令确认目标文件夹可访问。
    async fn ensure_folder_access(
        &self,
        profile: &str,
        folder_token: &str,
        timeout_ms: u64,
    ) -> Result<(), LarkCliError> {
        let arguments = vec![
            "--profile".into(),
            profile.into(),
            "drive".into(),
            "files".into(),
            "list".into(),
            "--params".into(),
            serde_json::json!({ "folder_token": folder_token, "page_size": 1 }).to_string(),
            "--as".into(),
            "user".into(),
            "--format".into(),
            "json".into(),
        ];
        self.execute_json(arguments, None, None, timeout_ms, OperationRisk::Read, true)
            .await?;
        Ok(())
    }

    /// 通过标准输入创建飞书原生 Markdown 文件并返回文件身份。

    #[cfg(test)]
    pub(in crate::workbench::connectors) async fn create_markdown(
        &self,
        profile: &str,
        folder_token: &str,
        file_name: &str,
        content: Vec<u8>,
        timeout_ms: u64,
    ) -> Result<LarkCliUploadResult, LarkCliError> {
        self.create_markdown_with_check(
            profile,
            folder_token,
            file_name,
            content,
            timeout_ms,
            || async { Ok(()) },
        )
        .await
    }

    /// 远端只读预检完成后、写入前再验证工作区乐观锁。
    pub(in crate::workbench::connectors) async fn create_markdown_with_check<F, Fut, G>(
        &self,
        profile: &str,
        folder_token: &str,
        file_name: &str,
        content: Vec<u8>,
        timeout_ms: u64,
        check: F,
    ) -> Result<LarkCliUploadResult, LarkCliError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<G, LarkCliError>>,
        G: Send,
    {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        self.ensure_supported_version(remaining_timeout_ms(deadline)?)
            .await?;
        self.ensure_user_profile(profile, remaining_timeout_ms(deadline)?)
            .await?;
        self.ensure_folder_access(profile, folder_token, remaining_timeout_ms(deadline)?)
            .await?;
        let fence = check().await?;
        let arguments = vec![
            "--profile".into(),
            profile.into(),
            "markdown".into(),
            "+create".into(),
            "--name".into(),
            file_name.into(),
            "--content".into(),
            "-".into(),
            "--folder-token".into(),
            folder_token.into(),
            "--as".into(),
            "user".into(),
            "--format".into(),
            "json".into(),
        ];
        let envelope = self
            .execute_json_with_fence(
                arguments,
                Some(content),
                None,
                remaining_timeout_ms(deadline)?,
                OperationRisk::Write,
                true,
                fence,
            )
            .await?;
        extract_file_token(&envelope)
    }

    /// 在受控临时目录上传 HTML 文件并保证目录最终清理。

    #[cfg(test)]
    pub(in crate::workbench::connectors) async fn upload_html(
        &self,
        profile: &str,
        folder_token: &str,
        file_name: &str,
        content: Vec<u8>,
        timeout_ms: u64,
    ) -> Result<LarkCliUploadResult, LarkCliError> {
        self.upload_html_with_check(
            profile,
            folder_token,
            file_name,
            content,
            timeout_ms,
            || async { Ok(()) },
        )
        .await
    }

    /// HTML 临时文件就绪后、远端写入前再验证工作区乐观锁。
    pub(in crate::workbench::connectors) async fn upload_html_with_check<F, Fut, G>(
        &self,
        profile: &str,
        folder_token: &str,
        file_name: &str,
        content: Vec<u8>,
        timeout_ms: u64,
        check: F,
    ) -> Result<LarkCliUploadResult, LarkCliError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<G, LarkCliError>>,
        G: Send,
    {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        self.ensure_supported_version(remaining_timeout_ms(deadline)?)
            .await?;
        self.ensure_user_profile(profile, remaining_timeout_ms(deadline)?)
            .await?;
        self.ensure_folder_access(profile, folder_token, remaining_timeout_ms(deadline)?)
            .await?;
        let temporary_directory = tempfile::Builder::new()
            .prefix("bifang-content-production-assistant-lark-upload-")
            .tempdir()
            .map_err(|_| {
                LarkCliError::new("CONNECTOR_TEMP_FILE_FAILED", "无法建立受控的 HTML 临时文件")
            })?;
        let local_name = "document.html";
        tokio::fs::write(temporary_directory.path().join(local_name), content)
            .await
            .map_err(|_| {
                LarkCliError::new("CONNECTOR_TEMP_FILE_FAILED", "无法写入受控的 HTML 临时文件")
            })?;
        let fence = check().await?;
        let arguments = vec![
            "--profile".into(),
            profile.into(),
            "drive".into(),
            "+upload".into(),
            "--file".into(),
            local_name.into(),
            "--name".into(),
            file_name.into(),
            "--folder-token".into(),
            folder_token.into(),
            "--as".into(),
            "user".into(),
            "--format".into(),
            "json".into(),
        ];
        let result = self
            .execute_json_with_fence(
                arguments,
                None,
                Some(Path::new(temporary_directory.path())),
                remaining_timeout_ms(deadline)?,
                OperationRisk::Write,
                true,
                fence,
            )
            .await;
        let _ = tokio::task::spawn_blocking(move || temporary_directory.close()).await;
        extract_file_token(&result?)
    }
}
