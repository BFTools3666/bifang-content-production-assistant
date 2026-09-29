use serde::{Deserialize, Serialize};

use crate::TokenUsage;

/// 作品来源与生成目标；各平台的指标缺失状态必须保持原样。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentPlatform {
    /// 技术文章平台。
    Csdn,
    /// 百家号文章平台。
    Baijiahao,
    /// 小红书图文或视频笔记。
    Xiaohongshu,
}

impl ContentPlatform {
    /// 返回适合提示词和界面使用的平台名称。
    pub const fn label(self) -> &'static str {
        match self {
            Self::Csdn => "CSDN",
            Self::Baijiahao => "百家号",
            Self::Xiaohongshu => "小红书",
        }
    }
}

/// 平台公开展示的互动指标；None 表示未公开或本次未采集，不能当作零。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkMetrics {
    /// 公开阅读量或播放量。
    pub views: Option<u64>,
    /// 公开点赞量。
    pub likes: Option<u64>,
    /// 公开收藏量。
    pub favorites: Option<u64>,
    /// 公开评论量。
    pub comments: Option<u64>,
    /// 公开分享量。
    pub shares: Option<u64>,
}

/// 一件可追溯的公开作品；不保存 Cookie、账号资料或登录凭据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkWork {
    /// 由平台与作品链接推导的稳定本地标识。
    #[serde(default)]
    pub id: String,
    /// 作品来源平台。
    pub platform: ContentPlatform,
    /// 公开标题。
    pub title: String,
    /// 原始可打开链接，小红书保留作品访问参数。
    pub url: String,
    /// 公开作者名称。
    #[serde(default)]
    pub author: String,
    /// 可打开的作者公开主页。
    #[serde(default)]
    pub author_url: String,
    /// 平台原样提供的发表时间；不推算缺失日期。
    #[serde(default)]
    pub published_at: String,
    /// 搜索卡片的公开摘要。
    #[serde(default)]
    pub excerpt: String,
    /// 详情页公开正文；为空时不可作为完整二创或风格分析样本。
    #[serde(default)]
    pub body: String,
    /// 本次公开页面提供的指标。
    #[serde(default)]
    pub metrics: WorkMetrics,
    /// 宿主采集的 Unix 毫秒时间。
    pub collected_at_ms: i64,
}

/// 保存到风格库的抽象写作特征，来源作品仍单独可追溯。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleProfile {
    /// 本地风格 ID。
    pub id: String,
    /// 用户自定的风格名称。
    pub name: String,
    /// 采样作品的平台。
    pub platform: ContentPlatform,
    /// 对标作者公开名称。
    pub author: String,
    /// 对标主页链接。
    pub homepage_url: String,
    /// 分析时冻结的完整公开作品样本。
    pub samples: Vec<BenchmarkWork>,
    /// AI 分析出的结构、语气、节奏、标题和禁用特征。
    pub analysis: String,
    /// 用户可以二次修改的应用指令。
    pub instructions: String,
    /// 用于阻止陈旧编辑覆盖的新修订号。
    pub revision: u32,
    /// 创建时间。
    pub created_at_ms: i64,
    /// 最近修改时间。
    pub updated_at_ms: i64,
}

/// 三类内置提示词；保存后供每次生成复用，单次仍可以覆盖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchPrompts {
    /// SEO 二创和原创写作要求。
    pub seo_rewrite: String,
    /// 小红书二创和原创写作要求。
    pub xhs_rewrite: String,
    /// 从多件作品抽象写作风格的分析要求。
    pub style_analysis: String,
}

impl Default for ResearchPrompts {
    /// 建立可编辑的内置指令，并要求原创、可核实和不冒充原作者。
    fn default() -> Self {
        Self {
            seo_rewrite: "你是中文编辑。围绕指定关键词与真实读者需求，重新构建一篇有独立结构和信息价值的原创文章。先提炼参考作品的问题与事实，再用不同的表达和组织方式写作，禁止逐句同义替换。CSDN强调可操作步骤、条件、代码准确性与问题解释；百家号强调普通读者可理解的信息、清晰段落和自然关键词。不虚构实测、经历、数据或引文，无法核实的内容明确标注。给出5个不同角度的二次标题、一个完整正文和相关关键词。".into(),
            xhs_rewrite: "你是小红书笔记编辑。围绕关键词和用户提供的真实材料，写出清楚、有用、有具体场景的原创笔记。学习参考作品的组织、语气、段落节奏与信息呈现，不复制原句、不冒充作者，不编造亲身经历、效果或数据。给出5个20字以内的标题、完整笔记正文和5到8个相关话题。正文使用易读的短段落，emoji适量，禁止堆砌营销词和无依据承诺。".into(),
            style_analysis: "分析提供的同一作者公开作品，提取可迁移的写作特征：标题机制、开头方式、结构、信息密度、句长和段落节奏、语气、修辞、结尾和互动方式。每个结论给出来源作品标题和简短依据，区分共同特征与偶然特征。生成能直接应用的写作指令，不搬运具体句子、人物身份、经历、观点事实或数据。样本不足时说明边界，不承诺复制传播结果。".into(),
        }
    }
}

/// 一次已保存的真实 AI 生成结果，保留来源、风格和非秘密模型血缘。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDraft {
    /// 草稿稳定 ID。
    pub id: String,
    /// 目标平台。
    pub platform: ContentPlatform,
    /// 本次关键词。
    pub keyword: String,
    /// 实际参考的作品标识。
    pub source_ids: Vec<String>,
    /// 实际应用的风格标识。
    pub style_id: Option<String>,
    /// AI 返回的候选标题。
    pub titles: Vec<String>,
    /// 用户当前选中的标题。
    pub title: String,
    /// 可编辑的原创正文。
    pub body: String,
    /// AI 返回的关键词或话题。
    pub tags: Vec<String>,
    /// 实际发给服务商的完整提示词快照，不含密钥。
    pub prompt: String,
    /// 调用的 Provider 配置 ID。
    pub provider_id: String,
    /// 调用的实际模型。
    pub model: String,
    /// 服务商提供的 token 使用量，不估算缺失字段。
    pub token_usage: Option<TokenUsage>,
    /// 生成时间。
    pub created_at_ms: i64,
}

/// 对标功能的持久业务资产；通过原有 SQLite 事务与旧文章一起保存。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchData {
    /// 已采集并按平台作品链接去重的公开作品。
    pub works: Vec<BenchmarkWork>,
    /// 用户保存的多个风格。
    pub styles: Vec<StyleProfile>,
    /// 生成和编辑后的草稿历史。
    pub drafts: Vec<ResearchDraft>,
    /// 可编辑的内置提示词。
    pub prompts: ResearchPrompts,
}

/// 单次生成输入；参考内容作为不可信资料放在独立区域。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentGenerationInput {
    /// 生成平台。
    pub platform: ContentPlatform,
    /// 用户关键词。
    pub keyword: String,
    /// 选中的参考作品。
    pub source_ids: Vec<String>,
    /// 本次可选风格。
    pub style_id: Option<String>,
    /// 用户提供的独立事实和写作需求。
    pub user_material: String,
    /// 本次二次修改后的指令；为空时使用库内默认指令。
    pub prompt_override: String,
}
