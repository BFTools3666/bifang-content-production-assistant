use super::*;

/// 平台新版搜索笔记路径与原explore资产合并，链接访问参数仍由作品自身保留。
#[test]
fn xhs_search_note_route_preserves_existing_identity() {
    let old = work_identity(ContentPlatform::Xiaohongshu, "https://www.xiaohongshu.com/explore/6a89a86d0000000033008dae?xsec_token=old").unwrap();
    let new = work_identity(ContentPlatform::Xiaohongshu, "https://www.xiaohongshu.com/search_result/6a89a86d0000000033008dae?xsec_token=new").unwrap();
    assert_eq!(old, new);
    assert_ne!(old, work_identity(ContentPlatform::Xiaohongshu, "https://www.xiaohongshu.com/search_result/?keyword=办公").unwrap());
}

/// CSDN普通地址与自定义博客域名指向同一作品，不生成两个资产。
#[test]
fn csdn_vanity_domain_deduplicates_article() {
    assert_eq!(work_identity(ContentPlatform::Csdn, "https://blog.csdn.net/example/article/details/123456").unwrap(), work_identity(ContentPlatform::Csdn, "https://example.blog.csdn.net/article/details/123456?source=profile").unwrap());
    assert!(validate_platform_url(ContentPlatform::Csdn, "https://user@example.blog.csdn.net").is_err());
}

/// 建立公开来源夹具，只用于验证领域约束，不作为平台采集或AI成功证据。
fn sample(index: usize, author: &str) -> BenchmarkWork {
    BenchmarkWork { id: String::new(), platform: ContentPlatform::Csdn, title:format!("技术教程{index}"), url:format!("https://blog.csdn.net/{author}/article/details/{index}"), author:author.into(), author_url:format!("https://blog.csdn.net/{author}"), published_at:String::new(), excerpt:String::new(), body:"公开文章的真实正文用于测试结构与长度规则。".repeat(12), metrics:WorkMetrics { views:Some(0), ..Default::default() }, collected_at_ms:123456 }
}

/// 缺失指标与真实零值在去重和序列化后仍可区分。
#[test]
fn missing_metrics_are_not_zero_and_urls_deduplicate() {
    let mut data = ResearchData::default();
    let first = sample(1,"alice");
    let mut second = first.clone(); second.url.push_str("?utm_source=other");
    merge_benchmark_works(&mut data,vec![first,second]).unwrap();
    assert_eq!(data.works.len(),1);
    assert_eq!(data.works[0].metrics.views,Some(0));
    assert_eq!(data.works[0].metrics.favorites,None);
    assert!(serde_json::to_string(&data).unwrap().contains("\"favorites\":null"));
}

/// 正文补充或页面重采不清空已观测指标；新观测的零值仍更新旧数值。
#[test]
fn recollection_preserves_known_metrics_and_accepts_observed_zero() {
    let mut data = ResearchData::default();
    let mut list = sample(1, "alice");
    list.body.clear();
    list.metrics = WorkMetrics { views: Some(59_957), likes: Some(45), favorites: Some(298), comments: Some(13), shares: None };
    merge_benchmark_works(&mut data, vec![list]).unwrap();
    let mut detail = sample(1, "alice");
    detail.metrics = WorkMetrics { views: Some(59_000), ..Default::default() };
    merge_benchmark_works(&mut data, vec![detail.clone()]).unwrap();
    assert!(!data.works[0].body.is_empty());
    assert_eq!(data.works[0].metrics.views, Some(59_000));
    assert_eq!(data.works[0].metrics.likes, Some(45));
    assert_eq!(data.works[0].metrics.favorites, Some(298));
    assert_eq!(data.works[0].metrics.comments, Some(13));
    assert_eq!(data.works[0].metrics.shares, None);
    detail.metrics.likes = Some(0);
    detail.metrics.favorites = Some(0);
    detail.metrics.comments = Some(0);
    detail.metrics.shares = Some(0);
    merge_benchmark_works(&mut data, vec![detail]).unwrap();
    assert_eq!(data.works[0].metrics.likes, Some(0));
    assert_eq!(data.works[0].metrics.favorites, Some(0));
    assert_eq!(data.works[0].metrics.comments, Some(0));
    assert_eq!(data.works[0].metrics.shares, Some(0));
}

/// 采集批次中一件无效链接会拒绝整个批次，保持原资产不变。
#[test]
fn invalid_collection_is_atomic() {
    let mut data = ResearchData::default();
    let mut invalid = sample(2,"alice"); invalid.url = "https://blog.csdn.net.evil.example/article".into();
    assert!(merge_benchmark_works(&mut data,vec![sample(1,"alice"),invalid]).is_err());
    assert!(data.works.is_empty());
    assert!(validate_platform_url(ContentPlatform::Csdn,"https://user@blog.csdn.net/a").is_err());
}

/// 二创必须使用已采集正文，不能把搜索摘要作为正文样本。
#[test]
fn rewrite_requires_collected_body() {
    let mut data = ResearchData::default();
    let mut work = sample(1,"alice"); work.body.clear();
    merge_benchmark_works(&mut data,vec![work]).unwrap();
    let input = ContentGenerationInput {platform:ContentPlatform::Csdn,keyword:"Python".into(),source_ids:vec![data.works[0].id.clone()],style_id:None,user_material:String::new(),prompt_override:String::new()};
    assert!(build_content_generation_prompt(&data,&input).unwrap_err().contains("作品正文"));
}

/// 风格必须来自同一主页至少三件正文样本，跨作者样本明确拒绝。
#[test]
fn style_sampling_checks_author_and_minimum() {
    let mut data = ResearchData::default();
    merge_benchmark_works(&mut data,vec![sample(1,"alice"),sample(2,"alice"),sample(3,"bob")]).unwrap();
    let ids = data.works.iter().map(|work|work.id.clone()).collect::<Vec<_>>();
    assert!(style_analysis_samples(&data,ContentPlatform::Csdn,"https://blog.csdn.net/alice",&ids).is_err());
    assert!(style_analysis_samples(&data,ContentPlatform::Csdn,"https://blog.csdn.net/alice",&ids[..2]).is_err());
    merge_benchmark_works(&mut data,vec![sample(4,"alice")]).unwrap();
    let ids = data.works.iter().filter(|work|work.author == "alice").map(|work|work.id.clone()).collect::<Vec<_>>();
    assert!(build_style_analysis_prompt(&data,ContentPlatform::Csdn,"https://blog.csdn.net/alice",&ids).unwrap().contains("不可信资料"));
}

/// 保存多个风格后按ID应用不同指令，修订冲突不能覆盖已有结果。
#[test]
fn multiple_styles_apply_independently_and_revision_conflicts_fail() {
    let mut data = ResearchData::default();
    merge_benchmark_works(&mut data,vec![sample(1,"alice"),sample(2,"alice"),sample(3,"alice")]).unwrap();
    let mut style = StyleProfile {id:"one".into(),name:"清晰教程".into(),platform:ContentPlatform::Csdn,author:"alice".into(),homepage_url:"https://blog.csdn.net/alice".into(),samples:data.works.clone(),analysis:"分析".into(),instructions:"短句解释步骤".into(),revision:1,created_at_ms:100,updated_at_ms:100};
    save_style_profile(&mut data,style.clone()).unwrap();
    style.id = "two".into(); style.instructions = "先提出问题再推导".into();
    save_style_profile(&mut data,style.clone()).unwrap();
    let input = ContentGenerationInput {platform:ContentPlatform::Csdn,keyword:"Python".into(),source_ids:vec![],style_id:Some("two".into()),user_material:String::new(),prompt_override:String::new()};
    let prompt = build_content_generation_prompt(&data,&input).unwrap();
    assert!(prompt.contains("先提出问题再推导")); assert!(!prompt.contains("短句解释步骤"));
    save_style_profile(&mut data,style.clone()).unwrap();
    assert!(save_style_profile(&mut data,style).is_err());
}

/// AI无效JSON、空正文和小红书超长标题均不能标为生成成功。
#[test]
fn generated_content_validation_rejects_incomplete_output() {
    assert!(parse_generated_content("not json",ContentPlatform::Csdn).is_err());
    let response = r#"{"titles":["标题一","标题二","标题三"],"body":"完整正文","tags":["AI"]}"#;
    assert_eq!(parse_generated_content(response,ContentPlatform::Xiaohongshu).unwrap().titles.len(),3);
    assert!(parse_generated_content(&response.replace("标题一",&"长".repeat(21)),ContentPlatform::Xiaohongshu).is_err());
}

/// v11旧资产升级到v12后保留原文章和连接器引用，新增指令与风格可以持久化。
#[test]
fn legacy_workspace_migration_preserves_existing_assets() {
    let mut old = crate::WorkspaceData::default(); old.schema_version = 11;
    old.connector_credential_refs.insert("retained".into(),"opaque-ref".into());
    let mut value = serde_json::to_value(&old).unwrap(); value.as_object_mut().unwrap().remove("research");
    let mut restored: crate::WorkspaceData = serde_json::from_value(value).unwrap();
    assert!(restored.migrate_to_current_schema().unwrap());
    assert_eq!(restored.schema_version,12);
    assert_eq!(restored.connector_credential_refs.get("retained").unwrap(),"opaque-ref");
    assert!(!restored.research.prompts.seo_rewrite.is_empty());
    let mut note = restored.clone(); initialize_note_workbench(&mut note);
    assert!(note.prompt_templates.iter().any(|prompt|prompt.content.contains("小红书")));
}

/// 快速接入不配置图片服务，也不会覆盖已有图片阶段的默认值。
#[test]
fn deepseek_onboarding_preserves_image_configuration() {
    let config = crate::ProviderConfig::deepseek_text_draft();
    assert_eq!(config.base_url, "https://api.deepseek.com");
    assert_eq!(config.article_model(), "deepseek-flash");
    assert!(!config.default_for_image);
    assert!(config.image_model.is_empty());
    assert!(config.validate_stage_configuration().is_ok());
}

/// 使用隔离草稿验证正文衔接，不触发网络、凭据或真实工作区。
#[test]
fn draft_handoff_keeps_body_and_is_idempotent() {
    let mut workspace = crate::WorkspaceData::default();
    let draft = ResearchDraft {
        id: "handoff-test".into(), platform: ContentPlatform::Csdn, keyword: "主题".into(),
        source_ids: vec![], style_id: None, titles: vec!["标题一".into(),"标题二".into(),"标题三".into()],
        title: "标题一".into(), body: "已检查的真实正文".into(), tags: vec!["关键词".into()],
        prompt: "本次提示词".into(), provider_id: "existing-provider".into(), model: "configured-model".into(),
        token_usage: None, created_at_ms: 1,
    };
    workspace.research.drafts.push(draft);
    let existing_count = workspace.articles.len();
    let id = prepare_research_article(&mut workspace, "handoff-test", 10).unwrap();
    assert_eq!(prepare_research_article(&mut workspace, "handoff-test", 11).unwrap(), id);
    assert_eq!(workspace.articles.len(), existing_count + 1);
    let task = workspace.articles.iter().find(|item| item.id == id).unwrap();
    assert_eq!(task.artifacts.markdown.as_deref(), Some("已检查的真实正文"));
    assert_eq!(task.stages.article.status, crate::StageStatus::Succeeded);
    assert_eq!(task.stages.summary.status, crate::StageStatus::NotStarted);
    workspace.research.drafts[0].body = "修改后的正文".into();
    assert!(prepare_research_article(&mut workspace, "handoff-test", 12).is_err());
    assert_eq!(workspace.articles.len(), existing_count + 1);
    assert_eq!(workspace.articles.iter().find(|item| item.id == id).unwrap().artifacts.markdown.as_deref(), Some("已检查的真实正文"));
    assert!(prepare_research_article(&mut workspace, "missing", 12).is_err());
    let (html, extension) = research_draft_document(&workspace.research, "handoff-test", "html").unwrap();
    assert_eq!(extension, "html");
    assert!(html.contains("修改后的正文"));
    workspace.research.drafts[0].body = "<script>alert(1)</script>".into();
    let (html, _) = research_draft_document(&workspace.research, "handoff-test", "html").unwrap();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(research_draft_document(&workspace.research, "handoff-test", "unsupported").is_err());
}
