import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/** 将前端平台与宿主身份一起固定，避免打出名字正确但平台错误的安装包。 */
const edition = process.argv[2];
if (edition !== "seo" && edition !== "xhs") {
  throw new Error("用法：node tools/build-edition.mjs seo|xhs [--windows-cross]");
}
const crossWindows = process.argv.includes("--windows-cross");
const guiDirectory = fileURLToPath(new URL("../bifang_content_production_assistant_gui/", import.meta.url));
const args = ["exec", "tauri", "build", "--no-sign", "--config", "src-tauri/tauri.release.conf.json"];
if (process.platform === "win32" || crossWindows) args.push("--bundles", "nsis", "--target", "x86_64-pc-windows-msvc");
if (crossWindows) args.push("--runner", "cargo-xwin");
if (edition === "xhs") args.push("--features", "xhs", "--config", "src-tauri/tauri.xhs.conf.json");
const result = spawnSync(process.platform === "win32" ? "pnpm.cmd" : "pnpm", args, {
  cwd: guiDirectory,
  env: { ...process.env, VITE_BIFANG_EDITION: edition },
  stdio: "inherit",
  shell: process.platform === "win32",
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
