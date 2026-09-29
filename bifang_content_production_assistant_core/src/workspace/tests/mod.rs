use crate::{
    ApprovedPackage, ArticleArtifacts, ArticlePackageSnapshot, CURRENT_PROMPT_RENDERER_VERSION,
    DocumentFormat, Failure, ReviewStatus,
};

use super::*;

fn real_provider(id: &str) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        name: "正式服务".into(),
        revision: 3,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: "https://example.com/v1".into(),
        text_model: "text".into(),
        article_model: Some("article".into()),
        summary_model: Some("summary".into()),
        image_model: "image".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    }
}

fn fixture_provider(id: &str) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        name: "测试夹具".into(),
        revision: 1,
        kind: ProviderKind::Fixture,
        enabled: true,
        base_url: String::new(),
        text_model: "fixture-text".into(),
        article_model: Some("fixture-article".into()),
        summary_model: Some("fixture-summary".into()),
        image_model: "fixture-image".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters::default()),
        default_for_article: true,
        default_for_summary: true,
        default_for_image: true,
    }
}

fn legacy_volcengine_ark_image_provider(id: &str) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        name: "旧火山方舟图片".into(),
        revision: 8,
        kind: ProviderKind::OpenAiCompatible,
        enabled: true,
        base_url: LEGACY_VOLCENGINE_ARK_IMAGE_ENDPOINT.into(),
        text_model: String::new(),
        article_model: Some(String::new()),
        summary_model: Some(String::new()),
        image_model: "ep-existing-image-model".into(),
        article_parameters: Some(TextGenerationParameters::default()),
        summary_parameters: Some(TextGenerationParameters::default()),
        image_parameters: Some(ImageGenerationParameters {
            size: "1024x1024".into(),
            watermark: false,
            timeout_ms: 137_000,
        }),
        default_for_article: false,
        default_for_summary: false,
        default_for_image: true,
    }
}

mod article;
mod batch;
mod binding;
mod compatibility;
mod connector;
mod execution_plan;
mod fixtures;
mod migration;
mod prompt;
mod provider_config;
mod provider_probe;
mod recovery;
mod run;
mod stage;
mod stage_execution;
mod stage_request;
