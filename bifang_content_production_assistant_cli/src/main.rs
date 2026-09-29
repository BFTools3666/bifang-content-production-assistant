//! CLI 的最薄进程入口；SOP 事实和协议渲染分别位于核心与适配器。

mod adapter;
mod article_output;
mod article_store;

/// 建立 Tokio 运行时并把完整命令处理委托给 CLI 适配器。
#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    adapter::run().await
}
