import { existsSync, lstatSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { loadDocument } from "../.agents/skills/desktop-prepare-release/scripts/release_notes.mjs";

/** 项目既有许可证与第三方源码分发映射，不能指向用户资料或覆盖更新日志。 */
export const LEGAL_RESOURCES = Object.freeze({
  "../../LICENSE.zh-CN.md": "Legal/LICENSE.zh-CN.md",
  "../../LICENSE.en.md": "Legal/LICENSE.en.md",
  "../../legal/THIRD_PARTY_LICENSES.txt": "Legal/THIRD_PARTY_LICENSES.txt",
  "../../legal/THIRD_PARTY_NOTICE.txt": "Legal/THIRD_PARTY_NOTICE.txt",
  "../../legal/MPL-Sources/cssparser-0.36.0.crate": "Legal/MPL-Sources/cssparser-0.36.0.crate",
  "../../legal/MPL-Sources/cssparser-macros-0.6.1.crate": "Legal/MPL-Sources/cssparser-macros-0.6.1.crate",
  "../../legal/MPL-Sources/dtoa-short-0.3.5.crate": "Legal/MPL-Sources/dtoa-short-0.3.5.crate",
  "../../legal/MPL-Sources/option-ext-0.2.0.crate": "Legal/MPL-Sources/option-ext-0.2.0.crate",
  "../../legal/MPL-Sources/selectors-0.36.1.crate": "Legal/MPL-Sources/selectors-0.36.1.crate",
});

/** 安装包构建前确认实际更新日志和发行版资源映射，拒绝缺少日志的发布包。 */
export function validateReleaseResources(projectRoot) {
  const root = resolve(projectRoot);
  const gui = join(root, "bifang_content_production_assistant_gui/src-tauri");
  const config = JSON.parse(readFileSync(join(gui, "tauri.conf.json"), "utf8"));
  const release = JSON.parse(readFileSync(join(gui, "tauri.release.conf.json"), "utf8"));
  if (release.bundle?.resources?.["../../release-notes.json"] !== "release-notes.json") {
    throw new Error("发布配置未正确包含更新日志，停止构建安装包。");
  }
  for (const [source, target] of Object.entries(release.bundle.resources)) {
    if (source === "../../release-notes.json") continue;
    if (LEGAL_RESOURCES[source] !== target) {
      throw new Error("发布资源包含未批准或重定向的映射，停止构建。");
    }
    const info = lstatSync(resolve(gui, source));
    if (info.isSymbolicLink() || !info.isFile()) {
      throw new Error("许可证资源必须是普通文件，停止构建。");
    }
  }
  for (const [source, target] of Object.entries(LEGAL_RESOURCES)) {
    // 项目已存在的分发资产不能从安装包资源映射中省略。
    if (existsSync(resolve(gui, source)) && release.bundle.resources[source] !== target) {
      throw new Error("安装包缺少项目许可证资源，停止构建。");
    }
  }
  let document;
  try { document = loadDocument(join(root, "release-notes.json")); }
  catch { throw new Error("缺少有效的双语更新日志。请先按发布流程准备 release-notes.json，再构建安装包。"); }
  if (document.releases[0]?.version !== `v${config.version}`) {
    throw new Error("更新日志最新版本与安装包不一致，停止构建。");
  }
  return document;
}
