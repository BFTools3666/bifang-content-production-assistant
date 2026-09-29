use super::*;

impl BatchWorker {
    /// 按文章 ID 读取已经生成的图片；WebView 不接触任意文件路径或文件权限。
    pub async fn load_article_image(&self, task_id: &str) -> Result<Option<String>, String> {
        let reference = self.repository.article_image_reference(task_id).await?;
        let Some(reference) = reference else {
            return Ok(None);
        };
        if reference.starts_with("data:image/") && fixture_mode_enabled() {
            return Ok(Some(reference));
        }
        let (mime, bytes) = self.read_managed_article_image(&reference).await?;
        crate::workbench::image_editor::decode_source_image(&bytes)?;
        Ok(Some(format!("data:{mime};base64,{}", BASE64.encode(bytes))))
    }

    /// 只为 Grid 返回固定上限的缩略图，不将 2K 原图或本地路径批量传入 WebView。
    pub async fn load_article_thumbnail(&self, task_id: &str) -> Result<Option<String>, String> {
        let reference = self.repository.article_image_reference(task_id).await?;
        let Some(reference) = reference else {
            return Ok(None);
        };
        if reference.starts_with("data:image/") && fixture_mode_enabled() {
            return Ok(Some(reference));
        }
        let _permit = self
            .thumbnail_slots
            .acquire()
            .await
            .map_err(|_| "无法读取文章配图缩略图".to_string())?;
        let (_, bytes) = self.read_managed_article_image(&reference).await?;
        tauri::async_runtime::spawn_blocking(move || render_grid_thumbnail(&bytes))
            .await
            .map_err(|_| "无法读取文章配图缩略图".to_string())?
            .map(Some)
    }

    /// 复用同一受管目录、大小和魔数校验读取原图字节。
    pub(crate) async fn read_managed_article_image(
        &self,
        reference: &str,
    ) -> Result<(&'static str, Vec<u8>), String> {
        let root = tokio::fs::canonicalize(&self.asset_dir)
            .await
            .map_err(|_| "无法读取文章配图".to_string())?;
        let path = tokio::fs::canonicalize(PathBuf::from(reference))
            .await
            .map_err(|_| "无法读取文章配图".to_string())?;
        if !path.starts_with(&root) {
            return Err("文章配图不在受管目录中".to_string());
        }
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|_| "无法读取文章配图".to_string())?;
        if !metadata.is_file()
            || metadata.len() == 0
            || metadata.len() > MAX_STORED_IMAGE_BYTES as u64
        {
            return Err("文章配图大小无效".to_string());
        }
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|_| "无法读取文章配图".to_string())?;
        let mut reader = file.take((MAX_STORED_IMAGE_BYTES + 1) as u64);
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| "无法读取文章配图".to_string())?;
        if bytes.is_empty() || bytes.len() > MAX_STORED_IMAGE_BYTES {
            return Err("文章配图大小无效".to_string());
        }
        let (mime, _) =
            validated_image_format(&bytes).ok_or_else(|| "文章配图格式无效".to_string())?;
        Ok((mime, bytes))
    }
}

/// 在受限解码器中把原图缩放并统一编码为小型 PNG data URL。
pub(super) fn render_grid_thumbnail(bytes: &[u8]) -> Result<String, String> {
    let format = match validated_image_format(bytes) {
        Some(("image/png", _)) => ::image::ImageFormat::Png,
        Some(("image/jpeg", _)) => ::image::ImageFormat::Jpeg,
        Some(("image/webp", _)) => ::image::ImageFormat::WebP,
        _ => return Err("文章配图格式无效".to_string()),
    };
    let mut limits = ::image::Limits::default();
    limits.max_image_width = Some(MAX_GRID_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_GRID_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_GRID_DECODED_BYTES);
    let mut reader = ::image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let source = reader
        .decode()
        .map_err(|_| "文章配图无法生成缩略图".to_string())?;
    let thumbnail = source.thumbnail(GRID_THUMBNAIL_WIDTH, GRID_THUMBNAIL_HEIGHT);
    let mut encoded = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut encoded, ::image::ImageFormat::Png)
        .map_err(|_| "文章配图无法生成缩略图".to_string())?;
    let encoded = encoded.into_inner();
    if encoded.is_empty() || encoded.len() > MAX_GRID_THUMBNAIL_BYTES {
        return Err("文章配图缩略图大小无效".to_string());
    }
    Ok(format!("data:image/png;base64,{}", BASE64.encode(encoded)))
}

/// 校验新批次依赖的模板、默认服务与凭据注册；凭据值只允许在正式启动时读取。
/// 将有界 Base64 图片写入应用数据目录；URL 返回在首版中失败关闭。
pub(super) async fn store_image_result(
    result: ImageGenerationResult,
    asset_dir: &Path,
) -> Result<String, ProviderError> {
    let (_, bytes) = validated_image_result_bytes(result)?;
    store_image_bytes(asset_dir, &bytes).await
}

/// 已通过正式图片边界校验、仅供当前进程临时展示的测试结果。
pub(crate) struct ValidatedImagePreview {
    /// 可安全交给 WebView 的受限图片 data URL。
    pub data_url: String,
    /// 从图片魔数识别出的 MIME。
    pub mime_type: &'static str,
    /// 解码后的真实字节数。
    pub byte_length: usize,
}

/// 验证一次图片生成结果并建立内存预览，不创建任何文件。
pub(crate) fn validated_image_generation_preview(
    result: ImageGenerationResult,
) -> Result<ValidatedImagePreview, ProviderError> {
    let (mime_type, bytes) = validated_image_result_bytes(result)?;
    Ok(ValidatedImagePreview {
        data_url: format!("data:{mime_type};base64,{}", BASE64.encode(&bytes)),
        mime_type,
        byte_length: bytes.len(),
    })
}

/// 把有界 Base64 结果解码为经过大小和图片魔数校验的字节。
pub(super) fn validated_image_result_bytes(
    result: ImageGenerationResult,
) -> Result<(&'static str, Vec<u8>), ProviderError> {
    let Some(encoded) = result.image_base64 else {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidResponse,
            safe_message: "AI 图片服务必须返回 Base64 图片数据".into(),
            retry_after_ms: None,
        });
    };
    if encoded.len() > MAX_ENCODED_IMAGE_CHARS {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidResponse,
            safe_message: "AI 图片大小无效".into(),
            retry_after_ms: None,
        });
    }
    let bytes = BASE64.decode(encoded).map_err(|_| ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: "AI 图片数据无法解码".into(),
        retry_after_ms: None,
    })?;
    if bytes.is_empty() || bytes.len() > MAX_STORED_IMAGE_BYTES {
        return Err(ProviderError {
            kind: ProviderErrorKind::InvalidResponse,
            safe_message: "AI 图片大小无效".into(),
            retry_after_ms: None,
        });
    }
    let (mime_type, _) = validated_image_format(&bytes).ok_or_else(|| ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: "AI 图片只接受 PNG、JPEG 或 WebP 格式".into(),
        retry_after_ms: None,
    })?;
    crate::workbench::image_editor::decode_source_image(&bytes).map_err(|_| ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: "AI 图片无法在 4096×4096、128 MiB 限制内完整解码".into(),
        retry_after_ms: None,
    })?;
    Ok((mime_type, bytes))
}

/// 将经过大小与来源校验的图片字节保存到应用数据目录。
pub(super) async fn store_image_bytes(
    asset_dir: &Path,
    bytes: &[u8],
) -> Result<String, ProviderError> {
    let (_, extension) = validated_image_format(bytes).ok_or_else(|| ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: "AI 图片只接受 PNG、JPEG 或 WebP 格式".into(),
        retry_after_ms: None,
    })?;
    tokio::fs::create_dir_all(asset_dir)
        .await
        .map_err(|_| image_store_error())?;
    let path = asset_dir.join(format!("{}.{extension}", uuid::Uuid::new_v4()));
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|_| image_store_error())?;
    Ok(path.to_string_lossy().to_string())
}

/// 根据文件魔数验证并返回允许展示的图片 MIME 与扩展名。
pub(crate) fn validated_image_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

/// 返回不包含文件路径的图片保存错误。
pub(super) fn image_store_error() -> ProviderError {
    ProviderError {
        kind: ProviderErrorKind::InvalidResponse,
        safe_message: "无法保存 AI 配图".into(),
        retry_after_ms: None,
    }
}
