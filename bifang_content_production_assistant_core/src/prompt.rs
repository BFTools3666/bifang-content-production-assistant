//! Prompt 模板变量、版本和本地预览规则。

use std::{collections::BTreeMap, error::Error, fmt};

use serde::{Deserialize, Serialize};

/// 首版 Prompt 渲染算法版本，仅用于恢复旧执行绑定。
pub const LEGACY_PROMPT_RENDERER_VERSION: u32 = 1;
/// 当前 Prompt 渲染算法版本，新执行绑定必须冻结该值。
pub const CURRENT_PROMPT_RENDERER_VERSION: u32 = 2;

/// Prompt 模板适用的 AI 生成阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromptTemplateKind {
    /// Markdown 正文生成模板。
    Article,
    /// 文章简介生成模板。
    Summary,
    /// 配图生成模板。
    Image,
}

/// 一份可版本化并在批次开始时冻结的 Prompt 模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    /// 稳定模板 ID。
    pub id: String,
    /// 用户可识别的模板名称。
    pub name: String,
    /// 模板适用阶段。
    pub kind: PromptTemplateKind,
    /// 包含 `{{variable}}` 占位符的模板正文。
    pub content: String,
    /// 模板版本，从 1 开始。
    pub version: u32,
}

impl PromptTemplate {
    /// 校验稳定身份、版本、模板内容和变量契约。
    pub fn validate(&self) -> Result<(), TemplateError> {
        if self.id.trim().is_empty() {
            return Err(TemplateError::EmptyId);
        }
        if self.version == 0 {
            return Err(TemplateError::InvalidVersion);
        }
        self.validate_content_contract()
    }

    /// 校验首版已经执行过的名称、正文和变量契约，不引入后续身份门禁。
    fn validate_content_contract(&self) -> Result<(), TemplateError> {
        if self.name.trim().is_empty() {
            return Err(TemplateError::EmptyName);
        }
        if self.content.trim().is_empty() {
            return Err(TemplateError::EmptyContent);
        }
        let variables = extract_template_variables(&self.content)?;
        for variable in &variables {
            if !self.allowed_variables().contains(&variable.as_str()) {
                return Err(TemplateError::UnknownVariable(variable.clone()));
            }
        }
        for required in self.required_variables() {
            if !variables.iter().any(|variable| variable == required) {
                return Err(TemplateError::MissingRequiredVariable((*required).into()));
            }
        }
        Ok(())
    }

    /// 校验首版冻结绑定；只豁免当时尚未要求的稳定 ID。
    fn validate_legacy_binding(&self) -> Result<(), TemplateError> {
        if self.version == 0 {
            return Err(TemplateError::InvalidVersion);
        }
        self.validate_content_contract()
    }

    /// 创建内容已更新的新模板版本，不修改旧版本。
    pub fn revised(&self, content: impl Into<String>) -> Result<Self, TemplateError> {
        self.revised_with_name(self.name.clone(), content)
    }

    /// 创建名称和内容已更新的新模板版本，同时保持稳定 ID 与阶段不变。
    pub fn revised_with_name(
        &self,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, TemplateError> {
        let candidate = Self {
            id: self.id.clone(),
            name: name.into(),
            kind: self.kind,
            content: content.into(),
            version: self
                .version
                .checked_add(1)
                .ok_or(TemplateError::VersionExhausted)?,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    /// 返回当前模板允许使用的变量。
    pub const fn allowed_variables(&self) -> &'static [&'static str] {
        match self.kind {
            PromptTemplateKind::Article => &["title", "keywords", "requirements"],
            PromptTemplateKind::Summary => &["title", "article"],
            PromptTemplateKind::Image => &["title", "article", "summary"],
        }
    }

    /// 返回当前模板必须包含的变量。
    pub const fn required_variables(&self) -> &'static [&'static str] {
        match self.kind {
            PromptTemplateKind::Article => &["title"],
            PromptTemplateKind::Summary => &["article"],
            PromptTemplateKind::Image => &["title", "summary"],
        }
    }
}

/// 模板解析、校验或预览失败时的稳定错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// 模板稳定 ID 为空。
    EmptyId,
    /// 模板版本不是从 1 开始的正整数。
    InvalidVersion,
    /// 模板版本已经达到可表示上限。
    VersionExhausted,
    /// 模板名称为空。
    EmptyName,
    /// 模板内容为空。
    EmptyContent,
    /// 出现未闭合的 `{{`。
    UnclosedVariable,
    /// 变量名为空或含非法字符。
    InvalidVariable(String),
    /// 当前模板阶段不允许该变量。
    UnknownVariable(String),
    /// 模板没有引用阶段必需变量。
    MissingRequiredVariable(String),
    /// 预览缺少某个已声明变量的值。
    MissingValue(String),
    /// 执行绑定引用了当前程序不支持的渲染算法版本。
    UnsupportedRendererVersion(u32),
}

impl TemplateError {
    /// 返回供各接口使用的稳定机器错误码。
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyId => "PROMPT_ID_REQUIRED",
            Self::InvalidVersion => "PROMPT_VERSION_INVALID",
            Self::VersionExhausted => "PROMPT_VERSION_EXHAUSTED",
            Self::EmptyName => "PROMPT_NAME_REQUIRED",
            Self::EmptyContent => "PROMPT_CONTENT_REQUIRED",
            Self::UnclosedVariable => "PROMPT_VARIABLE_UNCLOSED",
            Self::InvalidVariable(_) => "PROMPT_VARIABLE_INVALID",
            Self::UnknownVariable(_) => "PROMPT_VARIABLE_UNKNOWN",
            Self::MissingRequiredVariable(_) => "PROMPT_VARIABLE_REQUIRED",
            Self::MissingValue(_) => "PROMPT_PREVIEW_VALUE_MISSING",
            Self::UnsupportedRendererVersion(_) => "PROMPT_RENDERER_UNSUPPORTED",
        }
    }
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("模板稳定 ID 不能为空"),
            Self::InvalidVersion => formatter.write_str("模板版本必须从 1 开始"),
            Self::VersionExhausted => formatter.write_str("模板版本已达到上限"),
            Self::EmptyName => formatter.write_str("模板名称不能为空"),
            Self::EmptyContent => formatter.write_str("模板内容不能为空"),
            Self::UnclosedVariable => formatter.write_str("模板变量缺少结束标记 }}"),
            Self::InvalidVariable(name) => write!(formatter, "模板变量 {name} 的名称无效"),
            Self::UnknownVariable(name) => write!(formatter, "当前步骤不支持变量 {name}"),
            Self::MissingRequiredVariable(name) => write!(formatter, "模板缺少必需变量 {name}"),
            Self::MissingValue(name) => write!(formatter, "预览缺少变量 {name} 的值"),
            Self::UnsupportedRendererVersion(version) => {
                write!(formatter, "不支持 Prompt 渲染算法版本 {version}")
            }
        }
    }
}

impl Error for TemplateError {}

/// 提取模板中按首次出现顺序去重的变量名。
pub fn extract_template_variables(content: &str) -> Result<Vec<String>, TemplateError> {
    let mut variables = Vec::new();
    let mut remainder = content;
    while let Some(start) = remainder.find("{{") {
        remainder = &remainder[start + 2..];
        let end = remainder
            .find("}}")
            .ok_or(TemplateError::UnclosedVariable)?;
        let name = remainder[..end].trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(TemplateError::InvalidVariable(name.into()));
        }
        if !variables.iter().any(|variable| variable == name) {
            variables.push(name.into());
        }
        remainder = &remainder[end + 2..];
    }
    Ok(variables)
}

/// 使用明确变量值在本地渲染 Prompt 预览，不执行任何外部调用。
pub fn render_prompt(
    template: &PromptTemplate,
    values: &BTreeMap<String, String>,
) -> Result<String, TemplateError> {
    render_prompt_with_version(template, values, CURRENT_PROMPT_RENDERER_VERSION)
}

/// 按执行绑定冻结的算法版本渲染 Prompt，确保升级后的普通重试语义不漂移。
pub fn render_prompt_with_version(
    template: &PromptTemplate,
    values: &BTreeMap<String, String>,
    renderer_version: u32,
) -> Result<String, TemplateError> {
    match renderer_version {
        LEGACY_PROMPT_RENDERER_VERSION => render_prompt_legacy(template, values),
        CURRENT_PROMPT_RENDERER_VERSION => render_prompt_current(template, values),
        version => Err(TemplateError::UnsupportedRendererVersion(version)),
    }
}

/// 保留旧绑定的字节级渲染行为；新模板预览不得调用该函数。
fn render_prompt_legacy(
    template: &PromptTemplate,
    values: &BTreeMap<String, String>,
) -> Result<String, TemplateError> {
    template.validate_legacy_binding()?;
    let mut rendered = template.content.clone();
    for variable in extract_template_variables(&template.content)? {
        let value = values
            .get(&variable)
            .ok_or_else(|| TemplateError::MissingValue(variable.clone()))?;
        rendered = rendered.replace(&format!("{{{{{variable}}}}}"), value);
        rendered = rendered.replace(&format!("{{{{ {variable} }}}}"), value);
    }
    Ok(rendered)
}

/// 当前渲染算法接受已通过校验的内侧空白，并且不会递归改写变量值。
fn render_prompt_current(
    template: &PromptTemplate,
    values: &BTreeMap<String, String>,
) -> Result<String, TemplateError> {
    template.validate()?;
    let mut rendered = String::with_capacity(template.content.len());
    let mut remainder = template.content.as_str();
    while let Some(start) = remainder.find("{{") {
        rendered.push_str(&remainder[..start]);
        remainder = &remainder[start + 2..];
        let end = remainder
            .find("}}")
            .ok_or(TemplateError::UnclosedVariable)?;
        let variable = remainder[..end].trim();
        let value = values
            .get(variable)
            .ok_or_else(|| TemplateError::MissingValue(variable.into()))?;
        rendered.push_str(value);
        remainder = &remainder[end + 2..];
    }
    rendered.push_str(remainder);
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 正文模板只接受声明变量并要求标题存在。
    #[test]
    fn validates_allowed_and_required_variables() {
        let valid = PromptTemplate {
            id: "article".into(),
            name: "默认正文".into(),
            kind: PromptTemplateKind::Article,
            content: "以 {{title}} 为题，关键词：{{keywords}}".into(),
            version: 1,
        };
        assert_eq!(valid.validate(), Ok(()));

        let unknown = PromptTemplate {
            content: "{{title}} {{secret}}".into(),
            ..valid.clone()
        };
        assert_eq!(
            unknown.validate(),
            Err(TemplateError::UnknownVariable("secret".into()))
        );

        let missing = PromptTemplate {
            content: "只写关键词 {{keywords}}".into(),
            ..valid
        };
        assert_eq!(
            missing.validate(),
            Err(TemplateError::MissingRequiredVariable("title".into()))
        );
    }

    /// 本地预览替换所有变量且不会改变原模板版本。
    #[test]
    fn renders_preview_without_mutating_template() {
        let template = PromptTemplate {
            id: "summary".into(),
            name: "默认简介".into(),
            kind: PromptTemplateKind::Summary,
            content: "《{{title}}》：{{article}}".into(),
            version: 3,
        };
        let values = BTreeMap::from([
            ("title".into(), "标题".into()),
            ("article".into(), "正文".into()),
        ]);

        assert_eq!(render_prompt(&template, &values).unwrap(), "《标题》：正文");
        assert_eq!(template.version, 3);
    }

    /// 当前算法统一处理内侧空白且不递归替换；旧绑定仍保留旧算法的字节级结果。
    #[test]
    fn freezes_renderer_version_across_algorithm_changes() {
        let template = PromptTemplate {
            id: "summary".into(),
            name: "默认简介".into(),
            kind: PromptTemplateKind::Summary,
            content: "《{{  title  }}》：{{ article }}".into(),
            version: 1,
        };
        let values = BTreeMap::from([
            ("title".into(), "{{article}}".into()),
            ("article".into(), "正文".into()),
        ]);

        assert_eq!(
            render_prompt_with_version(&template, &values, CURRENT_PROMPT_RENDERER_VERSION)
                .unwrap(),
            "《{{article}}》：正文"
        );
        assert_eq!(
            render_prompt_with_version(&template, &values, LEGACY_PROMPT_RENDERER_VERSION).unwrap(),
            "《{{  title  }}》：正文"
        );
        assert_eq!(
            render_prompt_with_version(&template, &values, 99),
            Err(TemplateError::UnsupportedRendererVersion(99))
        );
    }

    /// 首版冻结绑定允许历史空 ID 继续重试；当前渲染和新保存仍执行严格身份校验。
    #[test]
    fn legacy_renderer_preserves_pre_identity_templates() {
        let template = PromptTemplate {
            id: String::new(),
            name: "旧版正文".into(),
            kind: PromptTemplateKind::Article,
            content: "请写 {{title}}".into(),
            version: 1,
        };
        let values = BTreeMap::from([("title".into(), "历史标题".into())]);

        assert_eq!(template.validate(), Err(TemplateError::EmptyId));
        assert_eq!(
            render_prompt_with_version(&template, &values, CURRENT_PROMPT_RENDERER_VERSION),
            Err(TemplateError::EmptyId)
        );
        assert_eq!(
            render_prompt_with_version(&template, &values, LEGACY_PROMPT_RENDERER_VERSION).unwrap(),
            "请写 历史标题"
        );
    }

    /// 修改模板会创建新版本并继续执行完整变量校验。
    #[test]
    fn revision_increments_version_and_revalidates() {
        let template = PromptTemplate {
            id: "image".into(),
            name: "默认配图".into(),
            kind: PromptTemplateKind::Image,
            content: "为 {{title}} 生成封面".into(),
            version: 1,
        };
        let revised = template
            .revised("结合 {{title}} 与 {{summary}} 生成封面")
            .unwrap();
        assert_eq!(revised.version, 2);
        assert_eq!(template.version, 1);
    }

    /// 稳定 ID、正版本以及版本上限由 core 统一失败关闭。
    #[test]
    fn validates_stable_identity_and_version_domain() {
        let valid = PromptTemplate {
            id: "article".into(),
            name: "默认正文".into(),
            kind: PromptTemplateKind::Article,
            content: "{{title}}".into(),
            version: 1,
        };
        assert_eq!(
            PromptTemplate {
                id: " ".into(),
                ..valid.clone()
            }
            .validate(),
            Err(TemplateError::EmptyId)
        );
        assert_eq!(
            PromptTemplate {
                version: 0,
                ..valid.clone()
            }
            .validate(),
            Err(TemplateError::InvalidVersion)
        );
        assert_eq!(
            PromptTemplate {
                version: u32::MAX,
                ..valid
            }
            .revised("{{title}}"),
            Err(TemplateError::VersionExhausted)
        );
    }

    /// 配图必须同时绑定标题和当前简介，避免保存脱离正文摘要的模板。
    #[test]
    fn image_template_requires_title_and_summary() {
        let missing_summary = PromptTemplate {
            id: "image".into(),
            name: "默认配图".into(),
            kind: PromptTemplateKind::Image,
            content: "为 {{title}} 生成封面".into(),
            version: 1,
        };
        assert_eq!(
            missing_summary.validate(),
            Err(TemplateError::MissingRequiredVariable("summary".into()))
        );

        let complete = PromptTemplate {
            content: "结合 {{title}} 与 {{summary}} 生成封面".into(),
            ..missing_summary
        };
        assert_eq!(complete.validate(), Ok(()));
    }
}
