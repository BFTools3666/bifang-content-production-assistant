/// 启动毕方内容生产助手的 Tauri 桌面应用。
fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments.get(1).and_then(|value| value.to_str()) == Some("--ark-image-probe") {
        std::process::exit(
            bifang_content_production_assistant_gui::run_ark_image_probe_cli(
                arguments.into_iter().skip(2).collect(),
            ),
        );
    }
    bifang_content_production_assistant_gui::run();
}
