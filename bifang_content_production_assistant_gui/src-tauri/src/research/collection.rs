use std::sync::Arc;

use bifang_content_production_assistant_core::research::{BenchmarkWork, ContentPlatform, merge_benchmark_works, validate_platform_url};
use reqwest::Url;
use serde::Deserialize;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::workbench::{WorkspaceRepository, models::CommandError, store::current_time_ms};
use super::{CollectionStatus, ResearchRuntime};

const COLLECTION_WINDOW: &str = "benchmark-collector";
const EXTRACTOR: &str = include_str!("page_extractor.js");

/// 回调只接受公开作品结构，不反序列化页面登录状态或任意命令。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageCollection {
    works: Vec<BenchmarkWork>,
    message: String,
    page_url: String,
    chunk_index: usize,
    chunk_count: usize,
}

/// 读取当前采集状态；没有新结果时不伪造空作品或成功。
#[tauri::command]
pub(crate) fn get_collection_status(runtime: State<'_, ResearchRuntime>) -> Result<CollectionStatus, CommandError> {
    runtime.collection.lock().map(|status| status.clone()).map_err(|_| collector_error("无法读取采集状态"))
}

/// 将平台、任务和随机会话标识安全嵌入固定自有提取脚本。
fn extraction_script(platform: ContentPlatform, session: &str) -> String {
    EXTRACTOR.replace("__BIFANG_PLATFORM__", &serde_json::to_string(&platform).expect("platform serialization"))
        .replace("__BIFANG_SESSION__", &serde_json::to_string(session).expect("session serialization"))
}

/// 解析只读的关键词搜索或用户指定公开链接，不接受任意协议。
fn collection_url(platform: ContentPlatform, keyword: &str, page_url: Option<&str>) -> Result<Url, CommandError> {
    if let Some(address) = page_url.filter(|address| !address.trim().is_empty()) {
        validate_platform_url(platform, address).map_err(|_| collector_error("请输入当前平台的HTTPS公开作品或主页链接"))?;
        return Url::parse(address).map_err(|_| collector_error("公开链接格式无效"));
    }
    if keyword.trim().is_empty() || keyword.chars().count() > 200 { return Err(collector_error("请填写200字以内的搜索关键词")); }
    let address = match platform {
        ContentPlatform::Csdn => "https://so.csdn.net/so/search",
        ContentPlatform::Baijiahao => "https://www.baidu.com/s",
        // 当前站点会把无末尾斜杠的搜索入口重定向到HTTP；直接使用可响应的HTTPS入口。
        ContentPlatform::Xiaohongshu => "https://www.xiaohongshu.com/search_result/",
    };
    let mut url = Url::parse(address).expect("static search URL");
    match platform {
        ContentPlatform::Csdn => { url.query_pairs_mut().append_pair("q", keyword).append_pair("t", "blog"); }
        ContentPlatform::Baijiahao => { url.query_pairs_mut().append_pair("wd", &format!("{keyword} site:baijiahao.baidu.com")); }
        ContentPlatform::Xiaohongshu => { url.query_pairs_mut().append_pair("keyword", keyword).append_pair("source", "web_search_result_notes"); }
    }
    Ok(url)
}

/// 打开独立可见WebView采集公开页面；登录与验证码由用户在该窗口完成。
#[tauri::command]
pub(crate) async fn start_benchmark_collection(app: AppHandle, platform: ContentPlatform, keyword: String, page_url: Option<String>, page_urls: Option<Vec<String>>, runtime: State<'_, ResearchRuntime>) -> Result<(), CommandError> {
    if cfg!(feature = "xhs") != (platform == ContentPlatform::Xiaohongshu) { return Err(collector_error("请选择当前工具支持的平台")); }
    let mut queue = std::collections::VecDeque::new();
    if let Some(addresses) = page_urls {
        if addresses.is_empty() || addresses.len() > 10 { return Err(collector_error("请选择1到10件作品采集正文")); }
        for address in addresses { queue.push_back(collection_url(platform, "", Some(&address))?); }
    }
    let url = if let Some(first) = queue.pop_front() { first } else { collection_url(platform, &keyword, page_url.as_deref())? };
    let queue = Arc::new(std::sync::Mutex::new(queue));
    let upgraded_redirects = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    let session = uuid::Uuid::new_v4().to_string();
    let script = extraction_script(platform, &session);
    let callback_app = app.clone();
    let capture_app = app.clone();
    if let Some(window) = app.get_webview_window(COLLECTION_WINDOW) { window.close().map_err(|_| collector_error("请先关闭现有采集窗口"))?; }
    *runtime.collection.lock().map_err(|_| collector_error("无法开始采集"))? = CollectionStatus { page_url: url.to_string(), message: "正在打开公开页面；平台要求登录时，请在采集窗口登录。".into(), collected_count: 0, updated_at_ms: current_time_ms() };
    WebviewWindowBuilder::new(&app, COLLECTION_WINDOW, WebviewUrl::External(url))
        .title(format!("毕方采集窗口｜{}", platform.label())).inner_size(1180.0, 820.0).min_inner_size(760.0, 600.0)
        .on_navigation(move |url| {
            if url.scheme() == "bifang-collect" {
                let query = url.query_pairs().collect::<std::collections::BTreeMap<_, _>>();
                if query.get("session").map(|value| value.as_ref()) != Some(session.as_str()) { return false; }
                let Some(payload) = query.get("payload").filter(|payload| payload.len() <= 2_000_000) else { return false; };
                let Ok(mut page) = serde_json::from_str::<PageCollection>(payload) else { return false; };
                if page.works.len() > 100 || page.chunk_count == 0 || page.chunk_count > 100 || page.chunk_index >= page.chunk_count || page.works.iter().any(|work| work.platform != platform) || !allowed_navigation(platform, &page.page_url) { return false; }
                for work in &mut page.works { work.collected_at_ms = current_time_ms(); }
                let app = callback_app.clone();
                let queue = queue.clone();
                tauri::async_runtime::spawn(async move {
                    let repository = app.state::<Arc<WorkspaceRepository>>();
                    let result = if page.works.is_empty() { Err(page.message.chars().take(300).collect::<String>()) } else { repository.update(|data| merge_benchmark_works(&mut data.research, page.works)).await };
                    let state = app.state::<ResearchRuntime>();
                    let saved = result.is_ok();
                    if let Ok(mut status) = state.collection.lock() {
                        status.page_url = page.page_url;
                        status.updated_at_ms = current_time_ms();
                        match result {
                            Ok(count) => { if page.chunk_index == 0 { status.collected_count = 0; } status.collected_count += count; status.message = format!("已保存{}件公开作品，可回到工具筛选。网页未提供的指标保持未提供。", status.collected_count); }
                            Err(message) => { status.collected_count = 0; status.message = message; }
                        }
                    }
                    if let Some(window) = app.get_webview_window(COLLECTION_WINDOW) {
                        let _ = window.eval(&format!("window.__bifangCollectionAck && window.__bifangCollectionAck({saved})"));
                        if saved && page.chunk_index + 1 == page.chunk_count {
                            if let Some(next) = queue.lock().ok().and_then(|mut queue| queue.pop_front()) { let _ = window.navigate(next); }
                        }
                    }
                });
                return false;
            }
            if let Some(secure_url) = secure_platform_redirect(platform, url.as_str()) {
                // 部分CSDN旧主页发出HTTP重定向；只加载其同址HTTPS版本。
                let first_attempt = upgraded_redirects.lock().map(|mut seen| seen.insert(secure_url.to_string())).unwrap_or(false);
                if first_attempt {
                    let app = callback_app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(window) = app.get_webview_window(COLLECTION_WINDOW) { let _ = window.navigate(secure_url); }
                    });
                } else if let Ok(mut status) = callback_app.state::<ResearchRuntime>().collection.lock() {
                    status.message = "平台主页反复跳转，请使用作者当前的HTTPS主页链接。".into();
                    status.updated_at_ms = current_time_ms();
                }
                return false;
            }
            allowed_navigation(platform, url.as_str())
        })
        .on_page_load(move |window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished && allowed_navigation(platform, payload.url().as_str()) {
                let _ = window.eval(&script);
                let state = capture_app.state::<ResearchRuntime>();
                if let Ok(mut status) = state.collection.lock() { status.page_url = payload.url().to_string(); }
            }
        })
        .build().map_err(|_| collector_error("无法打开采集窗口，请检查系统WebView组件"))?;
    Ok(())
}

/// 仅升级CSDN平台内的历史HTTP跳转，仍拒绝HTTP加载、凭据和外部域名。
fn secure_platform_redirect(platform: ContentPlatform, address: &str) -> Option<Url> {
    if platform != ContentPlatform::Csdn { return None; }
    let mut url = Url::parse(address).ok()?;
    if url.scheme() != "http" { return None; }
    url.set_scheme("https").ok()?;
    allowed_navigation(platform, url.as_str()).then_some(url)
}

/// 采集窗口只允许平台页面与该平台正常登录域名，不能调用主窗口IPC。
fn allowed_navigation(platform: ContentPlatform, address: &str) -> bool {
    let Ok(url) = Url::parse(address) else { return false; };
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() { return false; }
    let host = url.host_str().unwrap_or("");
    match platform {
        ContentPlatform::Csdn => host == "csdn.net" || host.ends_with(".csdn.net"),
        ContentPlatform::Baijiahao => host == "baidu.com" || host.ends_with(".baidu.com"),
        ContentPlatform::Xiaohongshu => host == "xiaohongshu.com" || host.ends_with(".xiaohongshu.com") || host == "xhslink.com",
    }
}

/// 返回采集协议的安全错误，不包含网络载荷或会话标识。
fn collector_error(message: &str) -> CommandError { CommandError::new("COLLECTION_FAILED", message) }

#[cfg(test)]
mod tests {
    use super::*;

    /// 搜索关键词被标准URL编码，不允许构造额外查询参数。
    #[test]
    fn search_query_is_encoded() {
        let url = collection_url(ContentPlatform::Csdn, "AI & Python", None).unwrap();
        assert_eq!(url.query_pairs().find(|(key, _)| key == "q").unwrap().1, "AI & Python");
        let note_url = collection_url(ContentPlatform::Xiaohongshu, "办公 & 效率", None).unwrap();
        assert_eq!(note_url.path(), "/search_result/");
        assert_eq!(note_url.query_pairs().find(|(key, _)| key == "keyword").unwrap().1, "办公 & 效率");
    }

    /// 意外主机、HTTP和用户名形式的伪装域名不能进入采集窗口。
    #[test]
    fn remote_page_cannot_expand_navigation_scope() {
        assert!(!allowed_navigation(ContentPlatform::Csdn, "https://blog.csdn.net.evil.example/post"));
        assert!(!allowed_navigation(ContentPlatform::Csdn, "https://user@blog.csdn.net/post"));
        assert!(!allowed_navigation(ContentPlatform::Csdn, "http://blog.csdn.net/post"));
        assert!(allowed_navigation(ContentPlatform::Xiaohongshu, "https://www.xiaohongshu.com/explore/123"));
    }

    /// CSDN实际旧主页跳转会升级到HTTPS，未扩展采集窗口的域名范围。
    #[test]
    fn old_csdn_home_redirect_stays_secure() {
        let secure = secure_platform_redirect(ContentPlatform::Csdn, "http://blog.csdn.net/gapapp").unwrap();
        assert_eq!(secure.as_str(), "https://blog.csdn.net/gapapp");
        assert!(!allowed_navigation(ContentPlatform::Csdn, "http://blog.csdn.net/gapapp"));
        assert!(secure_platform_redirect(ContentPlatform::Csdn, "http://blog.csdn.net.evil.example/gapapp").is_none());
        assert!(secure_platform_redirect(ContentPlatform::Csdn, "http://user@blog.csdn.net/gapapp").is_none());
        assert!(secure_platform_redirect(ContentPlatform::Csdn, "https://blog.csdn.net/gapapp").is_none());
        assert!(secure_platform_redirect(ContentPlatform::Baijiahao, "http://baijiahao.baidu.com/s").is_none());
    }
}
