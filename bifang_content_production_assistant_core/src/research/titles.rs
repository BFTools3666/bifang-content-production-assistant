use serde::{Deserialize, Serialize};

use super::ContentPlatform;

/// 标题生成只需要主题与真实要求，不要求预先填写文章标题。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleGenerationInput {
    /// 目标发布平台。
    pub platform: ContentPlatform,
    /// 用户提供的主题、关键词或读者问题。
    pub topic: String,
    /// 可选的受众、角度及真实业务材料。
    pub requirements: String,
    /// 本次候选数量，范围为 1 至 20。
    pub count: usize,
}

/// 有限长度的标题候选响应，不包含正文或用户凭据。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedTitles {
    /// 已规范化且互不重复的标题。
    pub titles: Vec<String>,
}

/// 在共享 core 验证标题请求，避免无效输入产生付费出站。
pub fn build_title_generation_prompt(input: &TitleGenerationInput) -> Result<String, String> {
    if input.topic.trim().is_empty() || input.topic.chars().count() > 200 {
        return Err("请填写200字以内的主题或关键词".into());
    }
    if !(1..=20).contains(&input.count) || input.requirements.chars().count() > 5_000 {
        return Err("一次生成1至20个标题，补充要求不超过5000字".into());
    }
    let limit = if input.platform == ContentPlatform::Xiaohongshu { 20 } else { 100 };
    Ok(format!(
        "你是中文内容编辑。为用户指定主题生成{}个不同角度、具体清楚的原创标题。主题可以来自任何行业或业务，不预设为技术文章。目标平台：{}。每个标题最多{limit}字，不虚构数字、效果、经历或承诺，不生成正文。\n主题：{}\n用户的受众、材料与要求：{}\n仅输出一个有效JSON对象，不要序号、代码围栏或额外解释：{{\"titles\":[\"标题\"]}}。数组必须恰有{}个互不重复的非空标题。",
        input.count, input.platform.label(), input.topic.trim(), input.requirements.trim(), input.count
    ))
}

/// 校验真实 AI 输出；空白、重复、超长或数量不足不能冒充成功。
pub fn parse_generated_titles(response: &str, input: &TitleGenerationInput) -> Result<GeneratedTitles, String> {
    build_title_generation_prompt(input)?;
    if response.len() > 32_768 {
        return Err("AI返回的标题过长，请重试".into());
    }
    let text = response.trim();
    let text = text.strip_prefix("```json").or_else(|| text.strip_prefix("```"))
        .map(|value| value.trim().strip_suffix("```").unwrap_or(value).trim()).unwrap_or(text);
    let mut result: GeneratedTitles = serde_json::from_str(text)
        .map_err(|_| "AI未返回有效的标题列表，请重试")?;
    let limit = if input.platform == ContentPlatform::Xiaohongshu { 20 } else { 100 };
    let mut seen = std::collections::HashSet::new();
    for title in &mut result.titles {
        *title = title.trim().to_string();
        if title.is_empty() || title.chars().count() > limit || title.chars().any(char::is_control)
            || !seen.insert(title.clone()) {
            return Err("AI标题存在空白、重复或长度问题，请重试".into());
        }
    }
    if result.titles.len() != input.count {
        return Err("AI返回的标题数量与本次要求不一致，请重试".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用真实业务主题验证成功路径与出站前拒绝。
    #[test]
    fn title_requests_accept_business_topics_and_reject_invalid_input() {
        let mut input = TitleGenerationInput { platform: ContentPlatform::Baijiahao, topic: "外贸客户跟进".into(), requirements: "面向运营人员".into(), count: 2 };
        let prompt = build_title_generation_prompt(&input).unwrap();
        assert!(prompt.contains("外贸客户跟进"));
        assert!(prompt.contains("不生成正文"));
        input.topic = " ".into();
        assert!(build_title_generation_prompt(&input).is_err());
        input.topic = "主题".into(); input.count = 21;
        assert!(build_title_generation_prompt(&input).is_err());
    }

    /// 解析候选而非正文，并拒绝不能安全加入多行工作台的输出。
    #[test]
    fn generated_titles_require_distinct_complete_single_line_candidates() {
        let input = TitleGenerationInput { platform: ContentPlatform::Csdn, topic: "主题".into(), requirements: String::new(), count: 2 };
        let result = parse_generated_titles("```json\n{\"titles\":[\" 标题一 \",\"标题二\"]}\n```", &input).unwrap();
        assert_eq!(result.titles, ["标题一", "标题二"]);
        for invalid in [r#"{"titles":["标题一"]}"#, r#"{"titles":["标题一","标题一"]}"#, r#"{"titles":["标题一","  "]}"#, r#"{"titles":["标题一","标题\n二"]}"#] {
            assert!(parse_generated_titles(invalid, &input).is_err());
        }
    }
}
