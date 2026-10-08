use serde::Serialize;

#[cfg(not(bifang_xhs))]
const APPLICATION_NAME: &str = "毕方内容生产助手";
#[cfg(bifang_xhs)]
const APPLICATION_NAME: &str = "毕方小红书笔记助手";
const CONTACT_CHANNEL: &str = "QQ";
const CONTACT_VALUE: &str = "2222980";

/// 从打包身份构造原生窗口与 WebView 共用的唯一标题。
pub fn packaged_title() -> String {
    format!(
        "{APPLICATION_NAME} v{} {CONTACT_CHANNEL}:{CONTACT_VALUE}",
        env!("CARGO_PKG_VERSION")
    )
}

/// 描述前端启动时需要的权威打包身份。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    /// 应用的已批准中文展示名称。
    pub application_name: &'static str,
    /// Cargo 工作区提供的不带展示前缀版本。
    pub version: &'static str,
    /// 联系方式类别。
    pub contact_channel: &'static str,
    /// 联系方式值。
    pub contact_value: &'static str,
    /// 原生窗口与 WebView 共用的完整动态标题。
    pub title: String,
}

/// 从打包常量与 Cargo 版本构造应用身份，避免 WebView 猜测这些事实。
#[tauri::command]
pub fn get_app_metadata() -> AppMetadata {
    let version = env!("CARGO_PKG_VERSION");
    AppMetadata {
        application_name: APPLICATION_NAME,
        version,
        contact_channel: CONTACT_CHANNEL,
        contact_value: CONTACT_VALUE,
        title: format!("{APPLICATION_NAME} v{version} {CONTACT_CHANNEL}:{CONTACT_VALUE}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 元数据始终由同一身份事实组装并只添加一个小写版本前缀。
    #[test]
    fn metadata_title_uses_packaged_identity() {
        let metadata = get_app_metadata();
        assert_eq!(metadata.application_name, APPLICATION_NAME);
        let expected = format!(
            "{} v{} {}:{}",
            metadata.application_name,
            metadata.version,
            metadata.contact_channel,
            metadata.contact_value
        );
        assert_eq!(metadata.title, expected);
        assert_eq!(packaged_title(), expected);
    }
}
