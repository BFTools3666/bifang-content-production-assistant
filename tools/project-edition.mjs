import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export const PROJECT_ROOT = fileURLToPath(new URL("../", import.meta.url));

/** 从当前仓库读取固定产品身份，拒绝混用另一个产品的安装配置。 */
export function readProjectEdition(projectRoot = PROJECT_ROOT) {
  const product = JSON.parse(readFileSync(join(projectRoot, "product-edition.json"), "utf8"));
  if (product.schemaVersion !== 1 || !["seo", "xhs"].includes(product.edition)) {
    throw new Error("产品身份无效；需要 schemaVersion 1 和固定 edition。");
  }
  const tauri = JSON.parse(readFileSync(join(projectRoot,
    "bifang_content_production_assistant_gui/src-tauri/tauri.conf.json"), "utf8"));
  if (tauri.identifier !== product.identifier || tauri.productName !== product.productName
    || tauri.app.windows.find((window) => window.label === "main")?.title !== product.productName) {
    throw new Error("产品身份与 Tauri 安装配置不一致，停止构建。");
  }
  return product;
}

/** 为当前独立项目生成构建参数；旧版 edition 参数只能指定本项目。 */
export function editionBuildPlan(argv, platform, projectRoot = PROJECT_ROOT) {
  const product = readProjectEdition(projectRoot);
  const requested = argv.filter((arg) => arg === "seo" || arg === "xhs");
  const unknown = argv.filter((arg) => !["seo", "xhs", "--windows-cross", "--check"].includes(arg));
  if (unknown.length || requested.length > 1) throw new Error("无效参数；使用 --windows-cross 或 --check。");
  if (requested.length && requested[0] !== product.edition) {
    throw new Error(`本仓库只构建${product.productName}，不能构建另一个产品。`);
  }
  const crossWindows = argv.includes("--windows-cross");
  const args = ["exec", "tauri", "build", "--no-sign", "--config", "src-tauri/tauri.release.conf.json"];
  if (platform === "win32" || crossWindows) args.push("--bundles", "nsis", "--target", "x86_64-pc-windows-msvc");
  if (crossWindows) args.push("--runner", "cargo-xwin");
  return { ...product, args, checkOnly: argv.includes("--check") };
}
