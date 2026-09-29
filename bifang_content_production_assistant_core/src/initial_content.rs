//! 用户确认的初始文章输入与一次性播种决策。

use serde::{Deserialize, Serialize};

use crate::{ArticleTask, PromptTemplate, PromptTemplateKind, TemplateError};

/// 当前内置初始内容的修订号；持久层用它保证删除后不会重新出现。
pub const CURRENT_INITIAL_CONTENT_REVISION: u32 = 1;

const LEGACY_ARTICLE_PROMPT: &str = "请围绕《{{title}}》撰写结构清晰、信息完整的中文 Markdown 文章。\n关键词：{{keywords}}\n补充要求：{{requirements}}";
const LEGACY_SUMMARY_PROMPT: &str = "请将以下文章压缩为 80 到 120 字的简介，直接输出简介正文。\n标题：{{title}}\n文章：\n{{article}}";
const LEGACY_IMAGE_PROMPT: &str =
    "为主题《{{title}}》生成一张简洁、专业、无文字的横版文章封面。文章简介：{{summary}}";

const FEISHU_ARTICLE_PROMPT: &str = "请为我写一篇关于{{title}}的文章，要求字数在1000-1500字之间，用markdown格式优化阅读，段落格式表现丰富，开头不要有生成文章的引导说明，结尾不要出现任何参考文献连接，直接输出正文内容。输入结果不要出现参考文献引用符号，不要出现任何形式的广告和链接。【文章的开头不要出现{{title}}的重复文字】\n关键词：{{keywords}}\n内容要求：{{requirements}}";
const FEISHU_IMAGE_PROMPT_V1: &str = "生成一张【不带任何文字或者类似文字的符号图片,不要人物形象,不要红绿底图配色，不要左右样式图片】,能表达{{title}}标题意义的配图。2D样式,互联网风格\n文章简介：{{summary}}";
const FEISHU_IMAGE_PROMPT: &str = "生成一张【不带任何文字或者类似文字的符号图片,不要人物形象,不要红绿底图配色，不要左右样式图片】,能表达{{title}}标题意义的配图。2D样式,互联网风格\n文章正文：{{article}}\n文章简介：{{summary}}";

/// 一条经用户确认、可稳定映射到文章任务的飞书初始输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitialArticleInput {
    /// 跨启动保持不变的本地任务 ID。
    pub stable_id: &'static str,
    /// 飞书“文章标题”字段原文。
    pub title: &'static str,
    /// 飞书“标签”字段原文；不臆造未提供的关键词。
    pub keyword: &'static str,
    /// 飞书“文章类型”字段原文。
    pub requirement: &'static str,
}

/// 一次性初始数据账本应执行的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialContentDecision {
    /// 内容为空，写入当前内置数据并标记修订。
    Apply,
    /// 已有业务内容，不写入但仍标记修订，避免日后删空时突然出现。
    SkipAndMarkProcessed,
    /// 当前修订已经处理，无需再次改变。
    AlreadyProcessed,
}

/// 初始数据账本包含当前程序不能安全解释的修订。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedInitialContentRevision(pub u32);

impl std::fmt::Display for UnsupportedInitialContentRevision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("本地初始数据修订不受支持")
    }
}

impl std::error::Error for UnsupportedInitialContentRevision {}

/// 根据持久修订和业务内容是否存在，决定本次启动能否写入内置数据。
pub fn decide_initial_content(
    current_revision: u32,
    has_business_content: bool,
) -> Result<InitialContentDecision, UnsupportedInitialContentRevision> {
    match current_revision {
        CURRENT_INITIAL_CONTENT_REVISION => Ok(InitialContentDecision::AlreadyProcessed),
        0 if has_business_content => Ok(InitialContentDecision::SkipAndMarkProcessed),
        0 => Ok(InitialContentDecision::Apply),
        unsupported => Err(UnsupportedInitialContentRevision(unsupported)),
    }
}

/// 返回用户飞书表 `QYSEO文章v1.3.0` 当前确认的 14 条真实输入。
pub const fn initial_article_inputs() -> &'static [InitialArticleInput] {
    &[
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-001",
            title: "跨境社媒运营做到后面 真正难的不是起号而是稳号",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-002",
            title: "账号真正值钱的是稳定释放同一种信号",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-003",
            title: "运营越做越重 很多时候不是内容不够而是内容没有层次",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-004",
            title: "平台规则持续收紧之后跨境卖家如何降低日常运营返工率",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-005",
            title: "欧洲小包成本改写之后跨境卖家如何重做多国发货方案",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-006",
            title: "附加费用增加时期跨境卖家如何优化高频商品履约路径",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-007",
            title: "清关节点变慢之后跨境卖家如何安排前置备货节奏",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-008",
            title: "站点投入分散带来压力跨境卖家如何提高资源使用效率",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-009",
            title: "订单利润越来越薄跨境卖家如何建立新的选品判断框架",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-010",
            title: "多国税费差异扩大之后跨境卖家如何重排市场重心",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-011",
            title: "成本波动传导加快阶段跨境卖家如何避免库存决策失误",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-012",
            title: "规则变化越来越密集跨境团队如何形成稳定执行流程",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-013",
            title: "履约结构被重新定价之后跨境卖家如何做好阶段性取舍",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
        InitialArticleInput {
            stable_id: "seed-qyseo-v1-014",
            title: "经营变量不断增加背景下跨境卖家如何提升整体抗风险能力",
            keyword: "跨境电商/营销策略",
            requirement: "原创",
        },
    ]
}

/// 把确认输入建立为尚未选中、尚未运行的领域任务。
pub fn initial_article_tasks(updated_at_ms: i64) -> Vec<ArticleTask> {
    initial_article_inputs()
        .iter()
        .map(|input| {
            let mut article = ArticleTask::new(input.stable_id, input.title, updated_at_ms);
            article.keywords = vec![input.keyword.to_string()];
            article.requirements = input.requirement.to_string();
            article
        })
        .collect()
}

/// 返回新工作区使用的三类 Prompt；正文和配图继承已确认的飞书生成约束。
pub fn initial_prompt_templates() -> Vec<PromptTemplate> {
    vec![
        PromptTemplate {
            id: "prompt-article-default".into(),
            name: "SEO 正文".into(),
            kind: PromptTemplateKind::Article,
            content: FEISHU_ARTICLE_PROMPT.into(),
            version: 1,
        },
        PromptTemplate {
            id: "prompt-summary-default".into(),
            name: "文章简介".into(),
            kind: PromptTemplateKind::Summary,
            content: LEGACY_SUMMARY_PROMPT.into(),
            version: 1,
        },
        PromptTemplate {
            id: "prompt-image-default".into(),
            name: "文章配图".into(),
            kind: PromptTemplateKind::Image,
            content: FEISHU_IMAGE_PROMPT.into(),
            version: 1,
        },
    ]
}

/// 仅当用户从未修改旧默认正文/配图时追加飞书约束的新版本。
pub fn upgrade_legacy_default_prompts(
    templates: &mut Vec<PromptTemplate>,
) -> Result<bool, TemplateError> {
    let replacements = [
        (
            "prompt-article-default",
            1,
            LEGACY_ARTICLE_PROMPT,
            FEISHU_ARTICLE_PROMPT,
        ),
        (
            "prompt-image-default",
            1,
            LEGACY_IMAGE_PROMPT,
            FEISHU_IMAGE_PROMPT,
        ),
        (
            "prompt-image-default",
            2,
            FEISHU_IMAGE_PROMPT_V1,
            FEISHU_IMAGE_PROMPT,
        ),
    ];
    let mut revised = Vec::new();
    for (id, legacy_version, legacy, current) in replacements {
        let Some(latest) = templates
            .iter()
            .filter(|template| template.id == id)
            .max_by_key(|template| template.version)
        else {
            continue;
        };
        if latest.version == legacy_version && latest.content == legacy {
            revised.push(latest.revised(current)?);
        }
    }
    let changed = !revised.is_empty();
    templates.extend(revised);
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 飞书初始输入必须保持固定数量、顺序、字段原文与稳定身份。
    #[test]
    fn preserves_confirmed_feishu_initial_inputs() {
        let tasks = initial_article_tasks(42);
        assert_eq!(tasks.len(), 14);
        assert_eq!(tasks[0].id, "seed-qyseo-v1-001");
        assert_eq!(
            tasks[0].title,
            "跨境社媒运营做到后面 真正难的不是起号而是稳号"
        );
        assert_eq!(tasks[13].id, "seed-qyseo-v1-014");
        assert_eq!(
            tasks[13].title,
            "经营变量不断增加背景下跨境卖家如何提升整体抗风险能力"
        );
        assert!(
            tasks
                .iter()
                .all(|task| task.keywords == ["跨境电商/营销策略"])
        );
        assert!(tasks.iter().all(|task| task.requirements == "原创"));
        assert!(tasks.iter().all(|task| task.updated_at_ms == 42));
    }

    /// revision 账本只允许首次空白工作区注入，并在已有内容时永久跳过。
    #[test]
    fn decides_initial_content_once_without_reappearing() {
        assert_eq!(
            decide_initial_content(0, false),
            Ok(InitialContentDecision::Apply)
        );
        assert_eq!(
            decide_initial_content(0, true),
            Ok(InitialContentDecision::SkipAndMarkProcessed)
        );
        assert_eq!(
            decide_initial_content(1, false),
            Ok(InitialContentDecision::AlreadyProcessed)
        );
        assert_eq!(
            decide_initial_content(2, false),
            Err(UnsupportedInitialContentRevision(2))
        );
    }

    /// 三类初始 Prompt 均满足 core 变量契约，且正文/配图包含飞书约束。
    #[test]
    fn validates_confirmed_initial_prompts() {
        let templates = initial_prompt_templates();
        assert_eq!(templates.len(), 3);
        assert!(templates.iter().all(|template| template.validate().is_ok()));
        assert!(templates[0].content.contains("1000-1500"));
        assert!(templates[0].content.contains("{{keywords}}"));
        assert!(templates[2].content.contains("不要人物形象"));
        assert!(templates[2].content.contains("{{article}}"));
        assert!(templates[2].content.contains("{{summary}}"));
    }

    /// 只升级完全未修改的旧默认值，用户修订过的模板保持原样。
    #[test]
    fn upgrades_only_untouched_legacy_default_prompts() {
        let mut templates = vec![
            PromptTemplate {
                id: "prompt-article-default".into(),
                name: "SEO 正文".into(),
                kind: PromptTemplateKind::Article,
                content: LEGACY_ARTICLE_PROMPT.into(),
                version: 1,
            },
            PromptTemplate {
                id: "prompt-summary-default".into(),
                name: "文章简介".into(),
                kind: PromptTemplateKind::Summary,
                content: LEGACY_SUMMARY_PROMPT.into(),
                version: 1,
            },
            PromptTemplate {
                id: "prompt-image-default".into(),
                name: "文章配图".into(),
                kind: PromptTemplateKind::Image,
                content: "用户自定义 {{title}} / {{summary}}".into(),
                version: 2,
            },
        ];
        assert!(upgrade_legacy_default_prompts(&mut templates).unwrap());
        assert_eq!(templates.len(), 4);
        assert_eq!(templates.last().unwrap().version, 2);
        assert!(templates.last().unwrap().content.contains("1000-1500"));
        assert_eq!(
            templates
                .iter()
                .filter(|template| template.id == "prompt-image-default")
                .count(),
            1
        );
        assert!(!upgrade_legacy_default_prompts(&mut templates).unwrap());
    }

    /// 已启动过旧版初始迁移的工作区只在模板仍未经用户修改时补入正文上下文。
    #[test]
    fn upgrades_untouched_first_feishu_image_prompt() {
        let mut templates = vec![PromptTemplate {
            id: "prompt-image-default".into(),
            name: "文章配图".into(),
            kind: PromptTemplateKind::Image,
            content: FEISHU_IMAGE_PROMPT_V1.into(),
            version: 2,
        }];

        assert!(upgrade_legacy_default_prompts(&mut templates).unwrap());
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[1].version, 3);
        assert!(templates[1].content.contains("{{article}}"));
        assert!(!upgrade_legacy_default_prompts(&mut templates).unwrap());
    }
}
