use super::*;

/// 图片格式由魔数决定，不信任响应声明或扩展名。
#[test]
fn image_format_uses_magic_bytes() {
    assert_eq!(
        validated_image_format(b"\x89PNG\r\n\x1a\nrest"),
        Some(("image/png", "png"))
    );
    assert_eq!(
        validated_image_format(&[0xff, 0xd8, 0xff, 0x01]),
        Some(("image/jpeg", "jpg"))
    );
    assert_eq!(
        validated_image_format(b"RIFF\x04\x00\x00\x00WEBP"),
        Some(("image/webp", "webp"))
    );
    assert_eq!(validated_image_format(b"<svg></svg>"), None);
}

/// 图片测试预览复用正式大小与魔数校验，且只返回内存 data URL。
#[test]
fn image_test_preview_accepts_real_images_and_rejects_decodable_text() {
    const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let preview = validated_image_generation_preview(ImageGenerationResult {
        image_url: None,
        image_base64: Some(PNG_BASE64.into()),
        provider_request_id: Some("must-not-cross-ipc".into()),
        token_usage: None,
    })
    .unwrap();

    assert_eq!(preview.mime_type, "image/png");
    assert_eq!(
        preview.byte_length,
        BASE64.decode(PNG_BASE64).unwrap().len()
    );
    assert_eq!(
        preview.data_url,
        format!("data:image/png;base64,{PNG_BASE64}")
    );
    assert!(
        validated_image_generation_preview(ImageGenerationResult {
            image_url: None,
            image_base64: Some(BASE64.encode("not an image")),
            provider_request_id: None,
            token_usage: None,
        })
        .is_err()
    );
}

/// 真实 PNG 的 Base64 响应必须写入受管目录，并能按同一字节重新加载为数据 URL。
#[tokio::test]
async fn png_base64_round_trip_persists_and_loads_managed_image() {
    const PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-image-round-trip-{}",
        uuid::Uuid::new_v4()
    ));
    let expected_bytes = BASE64.decode(PNG_BASE64).unwrap();
    let reference = store_image_result(
        bifang_content_production_assistant_core::ImageGenerationResult {
            image_url: None,
            image_base64: Some(PNG_BASE64.into()),
            provider_request_id: Some("request-round-trip".into()),
            token_usage: None,
        },
        &directory,
    )
    .await
    .unwrap();
    let path = PathBuf::from(&reference);
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some("png")
    );
    assert_eq!(tokio::fs::read(&path).await.unwrap(), expected_bytes);

    let repository = Arc::new(
        WorkspaceRepository::open_database_url("sqlite::memory:")
            .await
            .unwrap(),
    );
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.artifacts.image_ref = Some(reference);
            workspace.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    let worker = BatchWorker::new(Arc::clone(&repository), directory.clone()).unwrap();

    assert_eq!(
        worker.load_article_image("task").await.unwrap(),
        Some(format!("data:image/png;base64,{PNG_BASE64}"))
    );
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

/// Grid 只获得有界 PNG 缩略图，原图字节和本地路径不穿过 IPC。
#[tokio::test]
async fn grid_thumbnail_is_resized_and_bounded() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-grid-thumbnail-{}",
        uuid::Uuid::new_v4()
    ));
    let reference = store_image_bytes(&directory, &png_bytes(640, 480))
        .await
        .unwrap();
    let repository = Arc::new(
        WorkspaceRepository::open_database_url("sqlite::memory:")
            .await
            .unwrap(),
    );
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.artifacts.image_ref = Some(reference);
            workspace.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    let worker = BatchWorker::new(repository, directory.clone()).unwrap();

    let data_url = worker
        .load_article_thumbnail("task")
        .await
        .unwrap()
        .unwrap();
    let encoded = data_url
        .strip_prefix("data:image/png;base64,")
        .expect("Grid 缩略图必须统一编码为 PNG");
    let bytes = BASE64.decode(encoded).unwrap();
    let thumbnail = ::image::load_from_memory(&bytes).unwrap();

    assert!(bytes.len() <= MAX_GRID_THUMBNAIL_BYTES);
    assert!(thumbnail.width() <= GRID_THUMBNAIL_WIDTH);
    assert!(thumbnail.height() <= GRID_THUMBNAIL_HEIGHT);
    assert_eq!((thumbnail.width(), thumbnail.height()), (171, 128));
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

/// 压缩后文件虽小但像素维度过大时，缩略图解码会失败关闭且不泄露路径。
#[tokio::test]
async fn grid_thumbnail_rejects_excessive_source_dimensions() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-grid-thumbnail-limit-{}",
        uuid::Uuid::new_v4()
    ));
    let reference = store_image_bytes(&directory, &png_bytes(MAX_GRID_SOURCE_DIMENSION + 1, 1))
        .await
        .unwrap();
    let repository = Arc::new(
        WorkspaceRepository::open_database_url("sqlite::memory:")
            .await
            .unwrap(),
    );
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.artifacts.image_ref = Some(reference);
            workspace.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    let worker = BatchWorker::new(repository, directory.clone()).unwrap();

    let error = worker.load_article_thumbnail("task").await.unwrap_err();

    assert_eq!(error, "文章配图无法生成缩略图");
    assert!(!error.contains(&directory.to_string_lossy().to_string()));
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

/// 被本地篡改成超限大小的受管图片在读取前即拒绝，不把整文件载入内存。
#[tokio::test]
async fn managed_image_load_rejects_oversize_before_reading() {
    let directory = std::env::temp_dir().join(format!(
        "bifang-content-production-assistant-image-read-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let path = directory.join("oversize.png");
    let file = tokio::fs::File::create(&path).await.unwrap();
    file.set_len((MAX_STORED_IMAGE_BYTES + 1) as u64)
        .await
        .unwrap();
    drop(file);
    let repository = Arc::new(
        WorkspaceRepository::open_database_url("sqlite::memory:")
            .await
            .unwrap(),
    );
    repository
        .update(|workspace| {
            let mut article = ArticleTask::new("task", "标题", 1);
            article.artifacts.image_ref = Some(path.to_string_lossy().into_owned());
            workspace.articles.push(article);
            Ok(())
        })
        .await
        .unwrap();
    let worker = BatchWorker::new(Arc::clone(&repository), directory.clone()).unwrap();

    let error = worker.load_article_image("task").await.unwrap_err();

    assert_eq!(error, "文章配图大小无效");
    tokio::fs::remove_dir_all(directory).await.unwrap();
}
