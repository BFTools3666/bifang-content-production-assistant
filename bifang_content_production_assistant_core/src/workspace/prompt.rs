//! Prompt 版本选择和原子修订用例。

use std::collections::BTreeMap;

use super::{PromptTemplate, WorkspaceData};
use crate::TemplateError;

/// 工作区 Prompt 修订的稳定业务拒绝。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptWorkspaceError {
    /// 稳定 ID 不存在。
    NotFound,
    /// 同一稳定 ID 不能切换工作流阶段。
    KindMismatch,
    /// 模板内容或版本无效。
    Template(TemplateError),
}

impl PromptWorkspaceError {
    /// 稳定机器错误码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "PROMPT_NOT_FOUND",
            Self::KindMismatch => "PROMPT_KIND_MISMATCH",
            Self::Template(error) => error.code(),
        }
    }
}

/// 从同一 ID 的历史版本中选出最新版本，按稳定 ID 排列。
pub fn latest_prompt_templates(templates: &[PromptTemplate]) -> Vec<PromptTemplate> {
    let mut latest = BTreeMap::<String, PromptTemplate>::new();
    for template in templates {
        let replace = latest
            .get(&template.id)
            .is_none_or(|current| current.version < template.version);
        if replace {
            latest.insert(template.id.clone(), template.clone());
        }
    }
    latest.into_values().collect()
}

/// 校验并在同一工作区快照追加一份 Prompt 新版本。
pub fn append_prompt_template_revision(
    workspace: &mut WorkspaceData,
    requested: &PromptTemplate,
) -> Result<PromptTemplate, PromptWorkspaceError> {
    let latest = workspace
        .prompt_templates
        .iter()
        .filter(|candidate| candidate.id == requested.id)
        .max_by_key(|candidate| candidate.version)
        .cloned()
        .ok_or(PromptWorkspaceError::NotFound)?;
    if latest.kind != requested.kind {
        return Err(PromptWorkspaceError::KindMismatch);
    }
    let revised = latest
        .revised_with_name(requested.name.clone(), requested.content.clone())
        .map_err(PromptWorkspaceError::Template)?;
    workspace.prompt_templates.push(revised.clone());
    Ok(revised)
}
