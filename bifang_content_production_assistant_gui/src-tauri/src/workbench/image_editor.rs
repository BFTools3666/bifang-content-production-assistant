//! 配图编辑器的受限读取、像素变换、新文件保存与失败清理。

use std::{io::Cursor, path::PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bifang_content_production_assistant_core::{
    ArticleTask, IMAGE_EDIT_COORDINATE_SCALE, ImageEditBlockedReason, ImageEditError,
    ImageEditLock, ImageEditSpec,
};
use image::DynamicImage;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use super::{
    models::fixture_mode_enabled,
    store::current_time_ms,
    worker::{
        BatchWorker, MAX_GRID_DECODED_BYTES, MAX_GRID_SOURCE_DIMENSION, MAX_STORED_IMAGE_BYTES,
        validated_image_format,
    },
};

/// 只读编辑器返回的稳定保存禁用原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageEditBlockedView {
    /// 前端可分支处理的稳定原因码。
    pub code: String,
    /// 不含路径或内部错误的用户可见说明。
    pub message: String,
}

impl From<ImageEditBlockedReason> for ImageEditBlockedView {
    /// 把领域层禁用原因转换为前端可分支处理的只读视图。
    fn from(reason: ImageEditBlockedReason) -> Self {
        Self {
            code: reason.code().into(),
            message: reason.message().into(),
        }
    }
}

/// WebView 可使用的图片编辑器只读视图，不包含任何本地引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArticleImageEditorView {
    /// 由 Rust 按任务授权并完整解码过的 AI 原图 data URL。
    pub image_data_url: String,
    /// AI 原图像素宽度。
    pub source_width: u32,
    /// AI 原图像素高度。
    pub source_height: u32,
    /// 当前包已经保存的规范化编辑参数。
    pub current_spec: ImageEditSpec,
    /// 当前行版本。
    pub row_version: u64,
    /// 当前文章包版本。
    pub package_version: u32,
    /// 当前真实 AI 图片尝试；缺失时只能查看。
    pub image_attempt_id: Option<String>,
    /// 当前人工编辑修订。
    pub edit_revision: u64,
    /// 当前状态是否允许保存。
    pub can_save: bool,
    /// 保存被禁止时的明确原因。
    pub save_blocked_reason: Option<ImageEditBlockedView>,
}

/// 本地图片编辑适配器的稳定失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageEditorError {
    /// 供 command 映射的稳定错误码。
    pub code: String,
    /// 不泄露文件引用或内部解析详情的说明。
    pub message: String,
}

impl ImageEditorError {
    /// 创建一条稳定失败。
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// 保留 core 的稳定分类和安全说明。
    fn from_core(error: ImageEditError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

/// 已完整写入、但尚未提交工作区引用的新文件。
#[derive(Debug)]
struct WrittenImage {
    path: PathBuf,
    reference: String,
}

/// 按文章 ID 读取 AI 原图和保存锁；查看不会改变工作区或写入文件。
pub(crate) async fn load_article_image_editor(
    worker: &BatchWorker,
    task_id: &str,
) -> Result<ArticleImageEditorView, ImageEditorError> {
    let workspace = worker.repository().snapshot().await;
    let article = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .ok_or_else(|| ImageEditorError::new("ARTICLE_NOT_FOUND", "找不到指定文章"))?;
    let context = article
        .image_edit_context()
        .map_err(ImageEditorError::from_core)?;
    let lock = context.lock.clone();
    let mut blocked_reason = context.blocked_reason.map(ImageEditBlockedView::from);

    if context.original_image_ref.starts_with("data:image/") && fixture_mode_enabled() {
        blocked_reason.get_or_insert_with(|| ImageEditBlockedView {
            code: "IMAGE_EDIT_UNSUPPORTED_SOURCE".into(),
            message: "测试夹具配图只支持查看，不能保存人工编辑".into(),
        });
        return Ok(ArticleImageEditorView {
            image_data_url: context.original_image_ref,
            source_width: 1_200,
            source_height: 675,
            current_spec: context.spec,
            row_version: article.row_version,
            package_version: article.package_version,
            image_attempt_id: lock.as_ref().map(|lock| lock.image_attempt_id.clone()),
            edit_revision: article.artifacts.image_edit_revision,
            can_save: false,
            save_blocked_reason: blocked_reason,
        });
    }

    let (mime_type, bytes) = worker
        .read_managed_article_image(&context.original_image_ref)
        .await
        .map_err(|message| ImageEditorError::new("IMAGE_EDIT_SOURCE_INVALID", message))?;
    let decoded = tauri::async_runtime::spawn_blocking({
        let bytes = bytes.clone();
        move || decode_source_image(&bytes)
    })
    .await
    .map_err(|_| {
        ImageEditorError::new("IMAGE_EDIT_DECODE_FAILED", "文章配图无法在受限解码器中打开")
    })?
    .map_err(|message| ImageEditorError::new("IMAGE_EDIT_DECODE_FAILED", message))?;
    let image_attempt_id = lock.as_ref().map(|lock| lock.image_attempt_id.clone());
    Ok(ArticleImageEditorView {
        image_data_url: format!("data:{mime_type};base64,{}", BASE64.encode(bytes)),
        source_width: decoded.width(),
        source_height: decoded.height(),
        current_spec: context.spec,
        row_version: article.row_version,
        package_version: article.package_version,
        image_attempt_id,
        edit_revision: article.artifacts.image_edit_revision,
        can_save: blocked_reason.is_none() && lock.is_some(),
        save_blocked_reason: blocked_reason,
    })
}

/// 从 AI 原图重算像素并保存新的 UUID 文件，成功后才切换工作区引用。
pub(crate) async fn save_article_image_edit(
    worker: &BatchWorker,
    task_id: &str,
    lock: ImageEditLock,
    spec: ImageEditSpec,
) -> Result<ArticleTask, ImageEditorError> {
    let workspace = worker.repository().snapshot().await;
    let article = workspace
        .articles
        .iter()
        .find(|article| article.id == task_id)
        .ok_or_else(|| ImageEditorError::new("ARTICLE_NOT_FOUND", "找不到指定文章"))?;
    let context = article
        .prepare_image_edit(&lock, &spec)
        .map_err(ImageEditorError::from_core)?;
    if context.original_image_ref.starts_with("data:image/") {
        return Err(ImageEditorError::new(
            "IMAGE_EDIT_UNSUPPORTED_SOURCE",
            "当前配图格式只支持查看，不能保存人工编辑",
        ));
    }
    let (_, source_bytes) = worker
        .read_managed_article_image(&context.original_image_ref)
        .await
        .map_err(|message| ImageEditorError::new("IMAGE_EDIT_SOURCE_INVALID", message))?;
    let transformed = tauri::async_runtime::spawn_blocking({
        let spec = spec.clone();
        move || transform_source_image(&source_bytes, &spec)
    })
    .await
    .map_err(|_| {
        ImageEditorError::new(
            "IMAGE_EDIT_TRANSFORM_FAILED",
            "文章配图无法完成本地像素处理",
        )
    })??;
    let edit_id = uuid::Uuid::new_v4().to_string();
    let written = write_new_derived_image(worker, &edit_id, &transformed).await?;
    commit_written_image(worker, task_id, lock, spec, edit_id, written).await
}

/// 对魔数已验证的源文件执行完整受限解码。
pub(crate) fn decode_source_image(bytes: &[u8]) -> Result<DynamicImage, String> {
    let format = match validated_image_format(bytes) {
        Some(("image/png", _)) => image::ImageFormat::Png,
        Some(("image/jpeg", _)) => image::ImageFormat::Jpeg,
        Some(("image/webp", _)) => image::ImageFormat::WebP,
        _ => return Err("文章配图格式无效".into()),
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_GRID_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_GRID_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_GRID_DECODED_BYTES);
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| "文章配图无法安全解码".to_string())
}

/// 严格按“裁剪→顺时针旋转→水平翻转→垂直翻转”处理原图并编码 PNG。
fn transform_source_image(
    source_bytes: &[u8],
    spec: &ImageEditSpec,
) -> Result<Vec<u8>, ImageEditorError> {
    spec.validate().map_err(ImageEditorError::from_core)?;
    let source = decode_source_image(source_bytes)
        .map_err(|message| ImageEditorError::new("IMAGE_EDIT_DECODE_FAILED", message))?;
    let (left, top, width, height) = pixel_crop_bounds(source.width(), source.height(), spec)?;
    let mut edited = source.crop_imm(left, top, width, height);
    edited = match spec.rotation_quarter_turns {
        0 => edited,
        1 => edited.rotate90(),
        2 => edited.rotate180(),
        3 => edited.rotate270(),
        _ => {
            return Err(ImageEditorError::from_core(ImageEditError::InvalidRotation));
        }
    };
    if spec.flip_horizontal {
        edited = edited.fliph();
    }
    if spec.flip_vertical {
        edited = edited.flipv();
    }
    let mut encoded = Cursor::new(Vec::new());
    edited
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|_| {
            ImageEditorError::new("IMAGE_EDIT_ENCODE_FAILED", "文章配图无法编码为受管 PNG")
        })?;
    let encoded = encoded.into_inner();
    if encoded.is_empty() || encoded.len() > MAX_STORED_IMAGE_BYTES {
        return Err(ImageEditorError::new(
            "IMAGE_EDIT_OUTPUT_TOO_LARGE",
            "编辑后的文章配图超过 25 MiB 限制",
        ));
    }
    Ok(encoded)
}

/// 将规范化裁剪边界确定性映射为覆盖对应区域的整数像素矩形。
fn pixel_crop_bounds(
    source_width: u32,
    source_height: u32,
    spec: &ImageEditSpec,
) -> Result<(u32, u32, u32, u32), ImageEditorError> {
    let scale = u64::from(IMAGE_EDIT_COORDINATE_SCALE);
    let crop = spec.crop;
    let left = u64::from(crop.x) * u64::from(source_width) / scale;
    let top = u64::from(crop.y) * u64::from(source_height) / scale;
    let right_numerator = u64::from(crop.x + crop.width) * u64::from(source_width);
    let bottom_numerator = u64::from(crop.y + crop.height) * u64::from(source_height);
    let right = right_numerator.div_ceil(scale);
    let bottom = bottom_numerator.div_ceil(scale);
    if right <= left
        || bottom <= top
        || right > u64::from(source_width)
        || bottom > u64::from(source_height)
    {
        return Err(ImageEditorError::from_core(ImageEditError::InvalidCrop));
    }
    Ok((
        u32::try_from(left)
            .map_err(|_| ImageEditorError::from_core(ImageEditError::InvalidCrop))?,
        u32::try_from(top).map_err(|_| ImageEditorError::from_core(ImageEditError::InvalidCrop))?,
        u32::try_from(right - left)
            .map_err(|_| ImageEditorError::from_core(ImageEditError::InvalidCrop))?,
        u32::try_from(bottom - top)
            .map_err(|_| ImageEditorError::from_core(ImageEditError::InvalidCrop))?,
    ))
}

/// 以 `create_new` 独占创建 UUID 文件，完整同步后才允许工作区引用。
async fn write_new_derived_image(
    worker: &BatchWorker,
    edit_id: &str,
    bytes: &[u8],
) -> Result<WrittenImage, ImageEditorError> {
    let normalized_edit_id = uuid::Uuid::parse_str(edit_id)
        .map_err(|_| ImageEditorError::new("IMAGE_EDIT_STORE_FAILED", "新的配图文件标识无效"))?;
    tokio::fs::create_dir_all(worker.asset_dir())
        .await
        .map_err(|_| ImageEditorError::new("IMAGE_EDIT_STORE_FAILED", "无法创建配图受管目录"))?;
    let root = tokio::fs::canonicalize(worker.asset_dir())
        .await
        .map_err(|_| ImageEditorError::new("IMAGE_EDIT_STORE_FAILED", "无法访问配图受管目录"))?;
    let final_path = root.join(format!("{normalized_edit_id}.png"));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&final_path)
        .await
        .map_err(|_| ImageEditorError::new("IMAGE_EDIT_STORE_FAILED", "无法创建新的配图文件"))?;
    let write_result = async {
        file.write_all(bytes).await?;
        file.sync_all().await
    }
    .await;
    drop(file);
    if write_result.is_err() {
        let _ = tokio::fs::remove_file(&final_path).await;
        return Err(ImageEditorError::new(
            "IMAGE_EDIT_STORE_FAILED",
            "无法完整保存新的配图文件",
        ));
    }
    let canonical = match tokio::fs::canonicalize(&final_path).await {
        Ok(path) if path.starts_with(&root) => path,
        _ => {
            let _ = tokio::fs::remove_file(&final_path).await;
            return Err(ImageEditorError::new(
                "IMAGE_EDIT_STORE_FAILED",
                "新的配图文件未落在受管目录中",
            ));
        }
    };
    Ok(WrittenImage {
        reference: canonical.to_string_lossy().into_owned(),
        path: canonical,
    })
}

/// 提交工作区引用；任何提交失败只清理本次新文件，绝不触碰原图或旧派生图。
async fn commit_written_image(
    worker: &BatchWorker,
    task_id: &str,
    lock: ImageEditLock,
    spec: ImageEditSpec,
    edit_id: String,
    written: WrittenImage,
) -> Result<ArticleTask, ImageEditorError> {
    let reference = written.reference.clone();
    let update = worker
        .repository()
        .update(|workspace| {
            let article = workspace
                .articles
                .iter_mut()
                .find(|article| article.id == task_id)
                .ok_or_else(|| "ARTICLE_NOT_FOUND\u{1f}找不到指定文章".to_string())?;
            article
                .commit_image_edit(&lock, spec, reference, edit_id, current_time_ms())
                .map_err(|error| format!("{}\u{1f}{error}", error.code()))?;
            Ok(article.clone())
        })
        .await;
    match update {
        Ok(article) => Ok(article),
        Err(message) => {
            if tokio::fs::remove_file(&written.path).await.is_err() {
                return Err(ImageEditorError::new(
                    "IMAGE_EDIT_CLEANUP_FAILED",
                    "配图保存失败，且本次新文件未能自动清理；AI 原图和旧版本未改变",
                ));
            }
            let (code, safe_message) = message.split_once('\u{1f}').map_or(
                ("IMAGE_EDIT_SAVE_FAILED", "无法保存配图编辑"),
                |parts| parts,
            );
            Err(ImageEditorError::new(code, safe_message))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, sync::Arc};

    use bifang_content_production_assistant_core::{
        ArticleTask, ArtifactLineage, ArtifactOutput, ImageCrop, ImageEditLock, ImageEditSpec,
        WorkflowStage,
    };
    use image::{GenericImageView, ImageBuffer, Rgba};

    use super::*;
    use crate::workbench::{WorkspaceRepository, worker::BatchWorker};

    /// 把固定像素写成测试 PNG。
    fn pattern_png() -> Vec<u8> {
        let pixels = [
            Rgba([255, 0, 0, 255]),
            Rgba([0, 255, 0, 255]),
            Rgba([0, 0, 255, 255]),
            Rgba([0, 255, 255, 255]),
            Rgba([255, 0, 255, 255]),
            Rgba([255, 255, 0, 255]),
        ];
        let image = ImageBuffer::from_fn(3, 2, |x, y| pixels[(y * 3 + x) as usize]);
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        encoded.into_inner()
    }

    /// 创建指定像素尺寸的低熵 PNG，便于验证解码维度门禁。
    fn sized_png(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        encoded.into_inner()
    }

    /// 为完整文章建立真实 AI 配图血缘。
    fn complete_article(reference: String) -> ArticleTask {
        let mut task = ArticleTask::new("task", "标题", 0);
        task.queue_initial(1).unwrap();
        for output in [
            ArtifactOutput::Article("# 正文".into()),
            ArtifactOutput::Summary("简介".into()),
            ArtifactOutput::Image(reference),
            ArtifactOutput::Format("<h1>正文</h1>".into()),
        ] {
            let stage = output.stage();
            let attempt_id = task.start_stage(stage, 2).unwrap();
            let lineage = ArtifactLineage {
                package_version: task.package_version,
                source_article_version: (stage != WorkflowStage::Article)
                    .then_some(task.stages.article.artifact_version)
                    .flatten(),
                source_summary_version: (stage == WorkflowStage::Image)
                    .then_some(task.stages.summary.artifact_version)
                    .flatten(),
                template_id: (stage != WorkflowStage::Format).then(|| "template".into()),
                template_version: (stage != WorkflowStage::Format).then_some(1),
                provider_id: (stage != WorkflowStage::Format).then(|| "provider".into()),
                provider_revision: (stage != WorkflowStage::Format).then_some(1),
                model: (stage != WorkflowStage::Format).then(|| "model".into()),
                attempt_id,
            };
            task.complete_stage(stage, output, lineage, 3).unwrap();
        }
        task
    }

    /// 裁剪、旋转与翻转遵守固定次序，并保持角点像素精确。
    #[test]
    fn transform_order_preserves_expected_corner_pixels() {
        let spec = ImageEditSpec {
            rotation_quarter_turns: 1,
            flip_horizontal: true,
            ..ImageEditSpec::default()
        };
        let encoded = transform_source_image(&pattern_png(), &spec).unwrap();
        let result = image::load_from_memory(&encoded).unwrap();

        assert_eq!(result.dimensions(), (2, 3));
        assert_eq!(result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
        assert_eq!(result.get_pixel(1, 0), Rgba([0, 255, 255, 255]));
        assert_eq!(result.get_pixel(0, 2), Rgba([0, 0, 255, 255]));
        assert_eq!(result.get_pixel(1, 2), Rgba([255, 255, 0, 255]));
    }

    /// 亚像素相对范围使用固定向下/向上规则覆盖目标像素且不越界。
    #[test]
    fn normalized_crop_maps_to_deterministic_pixel_bounds() {
        let spec = ImageEditSpec {
            crop: ImageCrop {
                x: 333_333,
                y: 0,
                width: 333_334,
                height: 500_000,
            },
            ..ImageEditSpec::default()
        };

        assert_eq!(pixel_crop_bounds(3, 2, &spec).unwrap(), (0, 0, 3, 1));
    }

    /// 保存始终读取 AI 原图，保留原字节、历史和尝试记录且不发起任何运行。
    #[tokio::test]
    async fn save_recomputes_from_original_and_preserves_ai_artifacts() {
        let directory = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-image-editor-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let original_path = directory.join("original.png");
        let current_path = directory.join("current.png");
        let original_bytes = pattern_png();
        tokio::fs::write(&original_path, &original_bytes)
            .await
            .unwrap();
        tokio::fs::write(&current_path, [0xff, 0xd8, 0xff, 0xd9])
            .await
            .unwrap();
        let mut article = complete_article(original_path.to_string_lossy().into_owned());
        article.artifacts.image_ref = Some(current_path.to_string_lossy().into_owned());
        let attempts_before = article.attempt_history.clone();
        let repository = Arc::new(
            WorkspaceRepository::open_database_url("sqlite::memory:")
                .await
                .unwrap(),
        );
        repository
            .update(|workspace| {
                workspace.articles.push(article);
                Ok(())
            })
            .await
            .unwrap();
        let worker = BatchWorker::new(Arc::clone(&repository), directory.clone()).unwrap();
        let editor = load_article_image_editor(&worker, "task").await.unwrap();
        let lock = ImageEditLock {
            row_version: editor.row_version,
            package_version: editor.package_version,
            image_attempt_id: editor.image_attempt_id.unwrap(),
            edit_revision: editor.edit_revision,
        };
        let source_attempt_id = lock.image_attempt_id.clone();

        let saved = save_article_image_edit(
            &worker,
            "task",
            lock,
            ImageEditSpec {
                rotation_quarter_turns: 1,
                ..ImageEditSpec::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(
            tokio::fs::read(&original_path).await.unwrap(),
            original_bytes
        );
        assert_eq!(saved.package_version, 2);
        assert_eq!(saved.package_history.len(), 1);
        assert_eq!(saved.attempt_history, attempts_before);
        assert_eq!(repository.snapshot().await.run_history.len(), 0);
        let result_path = PathBuf::from(saved.artifacts.image_ref.as_ref().unwrap());
        assert_eq!(
            image::load_from_memory(&tokio::fs::read(&result_path).await.unwrap())
                .unwrap()
                .dimensions(),
            (2, 3)
        );

        let editor = load_article_image_editor(&worker, "task").await.unwrap();
        let restored = save_article_image_edit(
            &worker,
            "task",
            ImageEditLock {
                row_version: editor.row_version,
                package_version: editor.package_version,
                image_attempt_id: editor.image_attempt_id.unwrap(),
                edit_revision: editor.edit_revision,
            },
            ImageEditSpec::default(),
        )
        .await
        .unwrap();
        let restored_path = PathBuf::from(restored.artifacts.image_ref.as_ref().unwrap());
        assert_ne!(restored_path, result_path);
        assert!(result_path.exists());
        assert_eq!(restored.package_version, 3);
        assert_eq!(restored.package_history.len(), 2);
        assert_eq!(restored.attempt_history, attempts_before);
        assert_eq!(restored.artifacts.image_edit_revision, 2);
        assert_eq!(
            restored
                .artifacts
                .image_edit_lineage
                .as_ref()
                .unwrap()
                .source_attempt_id,
            source_attempt_id
        );
        assert_eq!(
            image::load_from_memory(&tokio::fs::read(restored_path).await.unwrap())
                .unwrap()
                .dimensions(),
            (3, 2)
        );
        assert_eq!(
            tokio::fs::read(&original_path).await.unwrap(),
            original_bytes
        );
        tokio::fs::remove_dir_all(directory).await.unwrap();
    }

    /// 受管目录逃逸与伪造魔数都在返回 data URL 前失败关闭。
    #[tokio::test]
    async fn load_rejects_path_escape_and_forged_magic() {
        let directory = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-image-editor-root-{}",
            uuid::Uuid::new_v4()
        ));
        let outside = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-image-editor-outside-{}.png",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        tokio::fs::write(&outside, pattern_png()).await.unwrap();
        let repository = Arc::new(
            WorkspaceRepository::open_database_url("sqlite::memory:")
                .await
                .unwrap(),
        );
        repository
            .update(|workspace| {
                workspace
                    .articles
                    .push(complete_article(outside.to_string_lossy().into_owned()));
                Ok(())
            })
            .await
            .unwrap();
        let worker = BatchWorker::new(Arc::clone(&repository), directory.clone()).unwrap();
        assert_eq!(
            load_article_image_editor(&worker, "task")
                .await
                .unwrap_err()
                .code,
            "IMAGE_EDIT_SOURCE_INVALID"
        );

        let forged = directory.join("forged.png");
        tokio::fs::write(&forged, b"\x89PNG\r\n\x1a\nnot-a-real-png")
            .await
            .unwrap();
        repository
            .update(|workspace| {
                workspace.articles[0].artifacts.image_ref =
                    Some(forged.to_string_lossy().into_owned());
                workspace.articles[0].artifacts.original_image_ref =
                    Some(forged.to_string_lossy().into_owned());
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(
            load_article_image_editor(&worker, "task")
                .await
                .unwrap_err()
                .code,
            "IMAGE_EDIT_DECODE_FAILED"
        );
        tokio::fs::remove_file(outside).await.unwrap();
        tokio::fs::remove_dir_all(directory).await.unwrap();
    }

    /// 压缩文件很小时仍按完整解码维度拒绝像素炸弹。
    #[tokio::test]
    async fn load_rejects_excessive_source_dimensions() {
        let directory = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-image-editor-dimension-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let oversized = directory.join("oversized.png");
        tokio::fs::write(&oversized, sized_png(MAX_GRID_SOURCE_DIMENSION + 1, 1))
            .await
            .unwrap();
        let repository = Arc::new(
            WorkspaceRepository::open_database_url("sqlite::memory:")
                .await
                .unwrap(),
        );
        repository
            .update(|workspace| {
                workspace
                    .articles
                    .push(complete_article(oversized.to_string_lossy().into_owned()));
                Ok(())
            })
            .await
            .unwrap();
        let worker = BatchWorker::new(repository, directory.clone()).unwrap();

        assert_eq!(
            load_article_image_editor(&worker, "task")
                .await
                .unwrap_err()
                .code,
            "IMAGE_EDIT_DECODE_FAILED"
        );
        tokio::fs::remove_dir_all(directory).await.unwrap();
    }

    /// 新输出即使遇到同名文件也失败关闭，绝不覆盖已有图片字节。
    #[tokio::test]
    async fn new_output_never_overwrites_an_existing_file() {
        let directory = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-image-editor-no-clobber-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let collision_id = "00000000-0000-4000-8000-000000000001";
        let existing = directory.join(format!("{collision_id}.png"));
        let sentinel = b"existing-image";
        tokio::fs::write(&existing, sentinel).await.unwrap();
        let repository = Arc::new(
            WorkspaceRepository::open_database_url("sqlite::memory:")
                .await
                .unwrap(),
        );
        let worker = BatchWorker::new(repository, directory.clone()).unwrap();

        let error = write_new_derived_image(&worker, collision_id, &pattern_png())
            .await
            .unwrap_err();

        assert_eq!(error.code, "IMAGE_EDIT_STORE_FAILED");
        assert_eq!(tokio::fs::read(existing).await.unwrap(), sentinel);
        tokio::fs::remove_dir_all(directory).await.unwrap();
    }

    /// 工作区提交冲突后只删除本次新文件，保留 AI 原图。
    #[tokio::test]
    async fn failed_commit_cleans_only_new_file() {
        let directory = std::env::temp_dir().join(format!(
            "bifang-content-production-assistant-image-editor-cleanup-{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let original = directory.join("original.png");
        tokio::fs::write(&original, pattern_png()).await.unwrap();
        let repository = Arc::new(
            WorkspaceRepository::open_database_url("sqlite::memory:")
                .await
                .unwrap(),
        );
        repository
            .update(|workspace| {
                workspace
                    .articles
                    .push(complete_article(original.to_string_lossy().into_owned()));
                Ok(())
            })
            .await
            .unwrap();
        let worker = BatchWorker::new(Arc::clone(&repository), directory.clone()).unwrap();
        let context = repository.snapshot().await.articles[0]
            .image_edit_context()
            .unwrap();
        let edit_id = "00000000-0000-4000-8000-000000000002";
        let written = write_new_derived_image(&worker, edit_id, &pattern_png())
            .await
            .unwrap();
        let new_path = written.path.clone();
        let mut stale_lock = context.lock.unwrap();
        stale_lock.row_version = stale_lock.row_version.saturating_sub(1);

        let error = commit_written_image(
            &worker,
            "task",
            stale_lock,
            ImageEditSpec::default(),
            edit_id.into(),
            written,
        )
        .await
        .unwrap_err();

        assert_eq!(error.code, "IMAGE_EDIT_CONFLICT");
        assert!(!new_path.exists());
        assert!(original.exists());
        tokio::fs::remove_dir_all(directory).await.unwrap();
    }
}
