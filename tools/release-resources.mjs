import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { loadDocument } from "../.agents/skills/desktop-prepare-release/scripts/release_notes.mjs";

/** 安装包构建前确认实际更新日志和发行版资源映射，拒绝缺少日志的发布包。 */
export function validateReleaseResources(projectRoot) {
  const root = resolve(projectRoot);
  const gui = join(root, "bifang_content_production_assistant_gui/src-tauri");
  const config = JSON.parse(readFileSync(join(gui, "tauri.conf.json"), "utf8"));
  const release = JSON.parse(readFileSync(join(gui, "tauri.release.conf.json"), "utf8"));
  if (release.bundle?.resources?.["../../release-notes.json"] !== "release-notes.json") {
    throw new Error("发布配置未正确包含更新日志，停止构建安装包。");
  }
  let document;
  try { document = loadDocument(join(root, "release-notes.json")); }
  catch { throw new Error("缺少有效的双语更新日志。请先按发布流程准备 release-notes.json，再构建安装包。"); }
  if (document.releases[0]?.version !== `v${config.version}`) {
    throw new Error("更新日志最新版本与安装包不一致，停止构建。");
  }
  return document;
}
