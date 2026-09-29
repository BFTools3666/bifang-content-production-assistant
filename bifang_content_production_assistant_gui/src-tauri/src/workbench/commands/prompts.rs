use super::*;

/// 返回每种 Prompt 的最新版本。
#[tauri::command]
pub async fn list_prompt_templates(
    repository: State<'_, Arc<WorkspaceRepository>>,
) -> Result<Vec<PromptTemplateView>, CommandError> {
    Ok(
        latest_prompt_templates(&repository.snapshot().await.prompt_templates)
            .into_iter()
            .map(PromptTemplateView::from)
            .collect(),
    )
}

/// 校验并追加一份 Prompt 新版本。
#[tauri::command]
pub async fn save_prompt_template(
    template: PromptTemplate,
    repository: State<'_, Arc<WorkspaceRepository>>,
) -> Result<PromptTemplate, CommandError> {
    repository
        .update(|workspace| {
            append_prompt_template_revision(workspace, &template)
                .map_err(|error| error.code().to_string())
        })
        .await
        .map_err(prompt_update_error)
}

/// 在本地渲染 Prompt 预览，不调用任何 Provider。
#[tauri::command]
pub async fn preview_prompt_template(
    template: PromptTemplate,
    values: BTreeMap<String, String>,
) -> Result<PromptPreview, CommandError> {
    render_prompt(&template, &values)
        .map(|rendered| PromptPreview { rendered })
        .map_err(|error| CommandError::new(prompt_template_error_code(&error), error.to_string()))
}

/// 把 core 模板错误映射为可翻译且可行动的稳定错误码。
pub(super) fn prompt_template_error_code(error: &TemplateError) -> &'static str {
    error.code()
}

/// 区分模板业务拒绝与真正的仓储失败，避免把可修复输入折叠为泛化错误。
pub(super) fn prompt_update_error(message: String) -> CommandError {
    let safe_message = match message.as_str() {
        "PROMPT_NOT_FOUND" => "找不到指定 Prompt 模板",
        "PROMPT_KIND_MISMATCH" => "Prompt 稳定 ID 不能改成其他阶段",
        "PROMPT_ID_REQUIRED" => "模板稳定 ID 不能为空",
        "PROMPT_VERSION_INVALID" => "模板版本必须从 1 开始",
        "PROMPT_VERSION_EXHAUSTED" => "模板版本已达到上限",
        "PROMPT_NAME_REQUIRED" => "模板名称不能为空",
        "PROMPT_CONTENT_REQUIRED" => "模板内容不能为空",
        "PROMPT_VARIABLE_UNCLOSED" => "模板变量缺少结束标记",
        "PROMPT_VARIABLE_INVALID" => "模板变量名称无效",
        "PROMPT_VARIABLE_UNKNOWN" => "模板包含当前阶段不支持的变量",
        "PROMPT_VARIABLE_REQUIRED" => "模板缺少当前阶段必需变量",
        "PROMPT_PREVIEW_VALUE_MISSING" => "预览缺少模板变量值",
        "PROMPT_RENDERER_UNSUPPORTED" => "Prompt 渲染算法版本不受支持",
        _ => return store_error(message),
    };
    CommandError::new(message, safe_message)
}
