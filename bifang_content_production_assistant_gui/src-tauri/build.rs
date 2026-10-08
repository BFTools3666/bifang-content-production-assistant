use std::path::PathBuf;

/// 从当前独立仓库的身份清单固定宿主类型，不依赖额外 feature 参数。
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let product_path = root.join("product-edition.json");
    println!("cargo:rerun-if-changed={}", product_path.display());
    println!("cargo:rustc-check-cfg=cfg(bifang_xhs)");
    let product: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&product_path).expect("无法读取产品身份清单"),
    )
    .expect("产品身份清单不是有效 JSON");
    assert_eq!(product["schemaVersion"].as_u64(), Some(1), "产品身份版本无效");
    match product["edition"].as_str() {
        Some("xhs") => println!("cargo:rustc-cfg=bifang_xhs"),
        Some("seo") => {},
        _ => panic!("产品身份必须是 seo 或 xhs"),
    }
    let tauri: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("bifang_content_production_assistant_gui/src-tauri/tauri.conf.json"))
            .expect("无法读取 Tauri 配置"),
    )
    .expect("Tauri 配置不是有效 JSON");
    assert_eq!(product["identifier"], tauri["identifier"], "安装身份不一致");
    assert_eq!(product["productName"], tauri["productName"], "软件名称不一致");
    tauri_build::build();
}
