use super::*;

/// 返回 Grid、最近批次和当前活动批次的完整视图。
#[tauri::command]
pub async fn get_workbench(
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<WorkbenchView, CommandError> {
    let mut workspace = repository.snapshot().await;
    workspace.articles.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    let active_run = worker.active_batch_id().and_then(|batch_id| {
        workspace
            .run_history
            .iter()
            .find(|run| run.id == batch_id)
            .cloned()
    });
    Ok(WorkbenchView {
        articles: workspace
            .articles
            .into_iter()
            .map(ArticleView::from)
            .collect(),
        run_history: workspace.run_history,
        active_run,
    })
}

/// 只返回指定文章已经生成的受管图片数据，不接受任意路径参数。
#[tauri::command]
pub async fn load_article_image(
    task_id: String,
    worker: State<'_, BatchWorker>,
) -> Result<Option<String>, CommandError> {
    worker
        .load_article_image(&task_id)
        .await
        .map_err(|message| CommandError::new("IMAGE_LOAD_FAILED", message))
}

/// 只返回 Grid 展示用的受限缩略图，不暴露原图路径或接受任意尺寸。
#[tauri::command]
pub async fn load_article_thumbnail(
    task_id: String,
    worker: State<'_, BatchWorker>,
) -> Result<Option<String>, CommandError> {
    worker
        .load_article_thumbnail(&task_id)
        .await
        .map_err(|message| CommandError::new("IMAGE_THUMBNAIL_LOAD_FAILED", message))
}

/// 返回编辑器需要的 AI 原图、像素、当前规范与保存锁，不暴露本地引用。
#[tauri::command]
pub async fn load_article_image_editor(
    task_id: String,
    worker: State<'_, BatchWorker>,
) -> Result<ArticleImageEditorView, CommandError> {
    load_image_editor(&worker, &task_id)
        .await
        .map_err(|error| CommandError::new(error.code, error.message))
}

/// 从 AI 原图执行白名单像素变换，并以新文件和新文章包保存。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn save_article_image_edit(
    task_id: String,
    expected_row_version: u64,
    expected_package_version: u32,
    expected_image_attempt_id: String,
    expected_edit_revision: u64,
    spec: ImageEditSpec,
    worker: State<'_, BatchWorker>,
) -> Result<ArticleView, CommandError> {
    let _start_guard = worker.acquire_start().await;
    let lock = ImageEditLock {
        row_version: expected_row_version,
        package_version: expected_package_version,
        image_attempt_id: expected_image_attempt_id,
        edit_revision: expected_edit_revision,
    };
    save_image_edit(&worker, &task_id, lock, spec)
        .await
        .map(ArticleView::from)
        .map_err(|error| CommandError::new(error.code, error.message))
}

/// 将多行文本解析为独立文章任务，空行忽略、重复行保留。
#[tauri::command]
pub async fn add_article_titles(
    titles: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
) -> Result<AddTitlesResult, CommandError> {
    let (non_blank, ignored_blank_count) = parse_article_titles(&titles)?;
    repository
        .update(|workspace| {
            Ok(append_article_titles(
                workspace,
                non_blank,
                ignored_blank_count,
                current_time_ms(),
            ))
        })
        .await
        .map_err(store_error)
}

/// 将 Core 标题错误映射到稳定 IPC 错误。
pub(super) fn parse_article_titles(titles: &str) -> Result<(Vec<String>, usize), CommandError> {
    bifang_content_production_assistant_core::parse_article_titles(titles)
        .map_err(|error| CommandError::new(error.code(), error.message()))
}

/// 为领域新增任务提供宿主生成的稳定 ID，并映射到 IPC 结果。
pub(super) fn append_article_titles(
    workspace: &mut WorkspaceData,
    titles: Vec<String>,
    ignored_blank_count: usize,
    now_ms: i64,
) -> AddTitlesResult {
    let result = bifang_content_production_assistant_core::append_article_titles(
        workspace,
        titles,
        ignored_blank_count,
        now_ms,
        || format!("article-{}", uuid::Uuid::new_v4()),
    );
    AddTitlesResult {
        created_count: result.created_count,
        ignored_blank_count: result.ignored_blank_count,
        duplicate_count: result.duplicate_count,
        task_ids: result.task_ids,
    }
}

/// 删除一条未在排队或运行的文章任务。
#[tauri::command]
pub async fn delete_article(
    task_id: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<(), CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    repository
        .update(|workspace| remove_article_and_collect_unused_credentials(workspace, &task_id))
        .await
        .map_err(|message| CommandError::new("DELETE_ARTICLE_FAILED", message))?;
    reconcile_pending_credential_deletions(&repository).await;
    Ok(())
}

/// 保存关键词与行级内容要求；必要时由 core 自动创建新文章包版本。
#[tauri::command]
pub async fn update_article_inputs(
    task_id: String,
    keywords: Vec<String>,
    requirements: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ArticleView, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    repository
        .update(|workspace| {
            let article = find_article_mut(workspace, &task_id)?;
            article
                .update_inputs(keywords, requirements, current_time_ms())
                .map_err(|error| error.to_string())?;
            Ok(ArticleView::from(article.clone()))
        })
        .await
        .map_err(|message| CommandError::new("ARTICLE_INPUT_FAILED", message))
}

/// 保存人工编辑的正文和/或简介，并由 core 失效依赖产物。
#[tauri::command]
pub async fn update_article_content(
    task_id: String,
    markdown: Option<String>,
    summary: Option<String>,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ArticleView, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    repository
        .update(|workspace| {
            let article = find_article_mut(workspace, &task_id)?;
            if let Some(markdown) = markdown {
                let lineage = manual_lineage(article, "manual-article-edit", false);
                article
                    .edit_article(markdown, lineage, current_time_ms())
                    .map_err(|error| error.to_string())?;
            }
            if let Some(summary) = summary {
                let lineage = manual_lineage(article, "manual-summary-edit", true);
                article
                    .edit_summary(summary, lineage, current_time_ms())
                    .map_err(|error| error.to_string())?;
            }
            Ok(ArticleView::from(article.clone()))
        })
        .await
        .map_err(|message| CommandError::new("EDIT_ARTICLE_FAILED", message))
}

/// 批准当前完整文章包并绑定不可变版本快照。
#[tauri::command]
pub async fn approve_article(
    task_id: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ArticleView, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    repository
        .update(|workspace| {
            let article = find_article_mut(workspace, &task_id)?;
            article
                .approve(current_time_ms())
                .map_err(|error| error.to_string())?;
            Ok(ArticleView::from(article.clone()))
        })
        .await
        .map_err(|message| CommandError::new("APPROVE_FAILED", message))
}

/// 将当前待审核文章包退回草稿，等待人工修改。
#[tauri::command]
pub async fn return_article_to_draft(
    task_id: String,
    repository: State<'_, Arc<WorkspaceRepository>>,
    worker: State<'_, BatchWorker>,
) -> Result<ArticleView, CommandError> {
    let _start_guard = worker.acquire_start().await;
    ensure_worker_available(&worker)?;
    repository
        .update(|workspace| {
            let article = find_article_mut(workspace, &task_id)?;
            article
                .return_to_draft(current_time_ms())
                .map_err(|error| error.to_string())?;
            Ok(ArticleView::from(article.clone()))
        })
        .await
        .map_err(|message| CommandError::new("RETURN_TO_DRAFT_FAILED", message))
}

/// 宿主生成尝试 ID，其余人工编辑血缘规则由 Core 计算。
pub(super) fn manual_lineage(
    article: &bifang_content_production_assistant_core::ArticleTask,
    action: &str,
    depends_on_article: bool,
) -> ArtifactLineage {
    bifang_content_production_assistant_core::manual_lineage(
        article,
        depends_on_article,
        format!("{action}-{}", uuid::Uuid::new_v4()),
    )
}
