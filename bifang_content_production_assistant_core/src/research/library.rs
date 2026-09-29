use super::*;

/// 限制一次采集的作品数，避免页面内容无界进入持久工作区。
pub const MAX_COLLECTION_WORKS: usize = 100;

/// 验证公开平台链接；明确拒绝凭据、非 HTTPS 和意外主机。
pub fn validate_platform_url(platform: ContentPlatform, url: &str) -> Result<(), String> {
    let rest = url.strip_prefix("https://").ok_or("作品链接必须使用 HTTPS")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase();
    let allowed = match platform {
        ContentPlatform::Csdn => matches!(host.as_str(), "blog.csdn.net" | "so.csdn.net") || (host.ends_with(".blog.csdn.net") && !host.contains(['@', ':'])),
        ContentPlatform::Baijiahao => matches!(host.as_str(), "baijiahao.baidu.com" | "author.baidu.com" | "www.baidu.com"),
        ContentPlatform::Xiaohongshu => matches!(host.as_str(), "www.xiaohongshu.com" | "xiaohongshu.com" | "xhslink.com"),
    };
    if !allowed || url.len() > 4_096 || url.chars().any(char::is_control) {
        return Err("链接不属于当前支持的公开平台".into());
    }
    Ok(())
}

/// 推导同一作品的稳定身份，访问令牌和追踪参数变化不产生重复作品。
pub fn work_identity(platform: ContentPlatform, url: &str) -> Result<String, String> {
    validate_platform_url(platform, url)?;
    let address = url.split('#').next().unwrap_or(url);
    let address = if platform == ContentPlatform::Csdn && address.contains("/article/details/") {
        let id = address.split("/article/details/").nth(1).unwrap_or("").split(['/', '?']).next().unwrap_or("");
        if id.is_empty() || !id.chars().all(|character| character.is_ascii_digit()) { return Err("CSDN文章链接缺少有效作品编号".into()); }
        format!("article:{id}")
    } else if platform == ContentPlatform::Xiaohongshu {
        let path = address.strip_prefix("https://www.xiaohongshu.com/").or_else(|| address.strip_prefix("https://xiaohongshu.com/"));
        let note_id = path.and_then(|path| path.strip_prefix("explore/").or_else(|| path.strip_prefix("search_result/")).or_else(|| path.strip_prefix("discovery/item/")))
            .map(|path| path.split(['/', '?']).next().unwrap_or(""))
            .filter(|id| (16..=40).contains(&id.len()) && id.chars().all(|character| character.is_ascii_alphanumeric()));
        if let Some(id) = note_id {
            // 保持既有explore作品的身份，搜索新路径不会生成重复资产。
            format!("https://www.xiaohongshu.com/explore/{id}")
        } else {
            address.split('?').next().unwrap_or(address).trim_end_matches('/').into()
        }
    } else if platform == ContentPlatform::Baijiahao {
        let article_id = address.split('?').nth(1).and_then(|query| query.split('&').find_map(|item| item.strip_prefix("id=").or_else(|| item.strip_prefix("app_id="))));
        if let Some(id) = article_id {
            format!("baijiahao:{id}")
        } else {
            address.split('?').next().unwrap_or(address).trim_end_matches('/').into()
        }
    } else {
        address.split('?').next().unwrap_or(address).trim_end_matches('/').into()
    };
    Ok(format!("{}:{address}", platform.label()))
}

/// 校验采集输入并按作品身份更新；单次空结果和无效项均明确失败。
pub fn merge_benchmark_works(data: &mut ResearchData, incoming: Vec<BenchmarkWork>) -> Result<usize, String> {
    if incoming.is_empty() || incoming.len() > MAX_COLLECTION_WORKS {
        return Err("本页未采集到作品，或作品数超过单次上限；请检查登录与页面加载".into());
    }
    let mut validated = Vec::with_capacity(incoming.len());
    for mut work in incoming {
        work.id = work_identity(work.platform, &work.url)?;
        if work.title.trim().is_empty() || work.title.chars().count() > 500 || work.collected_at_ms <= 0 {
            return Err("采集作品缺少有效标题或时间".into());
        }
        if !work.author_url.is_empty() {
            validate_platform_url(work.platform, &work.author_url)?;
        }
        work.body = work.body.chars().take(24_000).collect();
        work.excerpt = work.excerpt.chars().take(2_000).collect();
        work.author = work.author.chars().take(200).collect();
        work.published_at = work.published_at.chars().take(100).collect();
        validated.push(work);
    }
    let count = validated.len();
    for mut work in validated {
        if let Some(previous) = data.works.iter_mut().find(|item| item.id == work.id) {
            if work.body.is_empty() { work.body.clone_from(&previous.body); }
            if work.author.is_empty() { work.author.clone_from(&previous.author); }
            if work.author_url.is_empty() { work.author_url.clone_from(&previous.author_url); }
            // 详情和列表公开的指标可能不同；缺失表示本次未读到，不应抹掉已有观测。
            work.metrics.views = work.metrics.views.or(previous.metrics.views);
            work.metrics.likes = work.metrics.likes.or(previous.metrics.likes);
            work.metrics.favorites = work.metrics.favorites.or(previous.metrics.favorites);
            work.metrics.comments = work.metrics.comments.or(previous.metrics.comments);
            work.metrics.shares = work.metrics.shares.or(previous.metrics.shares);
            *previous = work;
        } else {
            data.works.push(work);
        }
    }
    data.works.sort_by(|left, right| right.collected_at_ms.cmp(&left.collected_at_ms));
    data.works.truncate(2_000);
    Ok(count)
}

/// 保存可编辑的指令库；拒绝空模板和异常大小。
pub fn save_research_prompts(data: &mut ResearchData, prompts: ResearchPrompts) -> Result<(), String> {
    for prompt in [&prompts.seo_rewrite, &prompts.xhs_rewrite, &prompts.style_analysis] {
        if prompt.trim().is_empty() || prompt.chars().count() > 12_000 {
            return Err("提示词不能为空，且不能超过12000字".into());
        }
    }
    data.prompts = prompts;
    Ok(())
}

/// 原子保存新风格或基于修订更新已有风格，不允许陈旧覆盖。
pub fn save_style_profile(data: &mut ResearchData, mut style: StyleProfile) -> Result<StyleProfile, String> {
    if style.id.is_empty() || style.name.trim().is_empty() || style.name.chars().count() > 100 || style.instructions.trim().is_empty() || style.instructions.chars().count() > 20_000 {
        return Err("风格需要名称和有效的写作指令".into());
    }
    validate_platform_url(style.platform, &style.homepage_url)?;
    if style.samples.len() < 3 || style.samples.len() > 10 || style.samples.iter().any(|work| work.body.chars().count() < 80) {
        return Err("请先采集同一主页至少3篇、最多10篇作品正文".into());
    }
    if let Some(previous) = data.styles.iter_mut().find(|item| item.id == style.id) {
        if previous.revision != style.revision { return Err("风格已被修改，请刷新后再保存".into()); }
        style.revision = style.revision.checked_add(1).ok_or("风格修订号已达上限")?;
        style.created_at_ms = previous.created_at_ms;
        *previous = style.clone();
    } else {
        if data.styles.len() >= 100 { return Err("风格库最多保存100个风格".into()); }
        style.revision = 1;
        data.styles.push(style.clone());
    }
    Ok(style)
}

/// 保存真实生成结果并限制历史数量；不以占位文本补全缺失输出。
pub fn save_research_draft(data: &mut ResearchData, draft: ResearchDraft) -> Result<(), String> {
    if draft.id.is_empty() || draft.title.trim().is_empty() || draft.body.trim().is_empty() || draft.body.chars().count() > 40_000 {
        return Err("草稿需要真实标题和正文，正文不能超过40000字".into());
    }
    if let Some(previous) = data.drafts.iter_mut().find(|item| item.id == draft.id) {
        *previous = draft;
    } else {
        data.drafts.insert(0, draft);
        data.drafts.truncate(500);
    }
    Ok(())
}

/// 新建小红书独立工作区时沿用原SOP，但使用笔记、摘要与封面的平台指令。
pub fn initialize_note_workbench(data: &mut crate::WorkspaceData) {
    data.articles.clear();
    for template in &mut data.prompt_templates {
        match template.kind {
            crate::PromptTemplateKind::Article => {
                template.name = "小红书原创笔记".into();
                template.content = "围绕《{{title}}》和关键词{{keywords}}，写一篇有具体读者场景的小红书原创笔记。补充材料：{{requirements}}。使用清晰短段落，信息具体有用，末尾提供5到8个话题。不虚构亲身经历、效果、数字或引用，不冒充任何作者。直接输出笔记正文。".into();
            }
            crate::PromptTemplateKind::Summary => {
                template.name = "小红书笔记摘要".into();
                template.content = "将《{{title}}》的以下笔记概括为80到120字的准确摘要，不添加未提供的事实。只输出摘要。\n{{article}}".into();
            }
            crate::PromptTemplateKind::Image => {
                template.name = "小红书笔记配图".into();
                template.content = "为《{{title}}》生成清楚、具体的小红书笔记配图，不出现文字、数字和虚构数据。笔记内容：{{article}}\n摘要：{{summary}}".into();
            }
        }
    }
}
