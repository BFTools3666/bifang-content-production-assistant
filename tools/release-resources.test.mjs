import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { validateReleaseResources } from "./release-resources.mjs";

/** 用隔离发布输入验证缺失、版本不符和资源映射错误，避免再次发布无日志的安装包。 */
test("release resources reject missing or mismatched notes", () => {
  const root = mkdtempSync(join(tmpdir(), "bifang-release-resources-"));
  const gui = join(root, "bifang_content_production_assistant_gui/src-tauri");
  mkdirSync(gui, { recursive: true });
  const config = { bundle: { resources: { "../../release-notes.json": "release-notes.json" } } };
  try {
    writeFileSync(join(gui, "tauri.conf.json"), JSON.stringify({ version: "0.2.28" }));
    writeFileSync(join(gui, "tauri.release.conf.json"), JSON.stringify(config));
    assert.throws(() => validateReleaseResources(root), /缺少有效/);
    const document = { schemaVersion: 2, releases: [{ releaseDate: "2026-10-09", version: "v0.2.28", featureOptimizations: [{ "zh-CN": "内部单元测试记录", "en-US": "Unit test entry" }], bugFixes: [] }] };
    writeFileSync(join(root, "release-notes.json"), JSON.stringify(document));
    assert.equal(validateReleaseResources(root).releases[0].version, "v0.2.28");
    document.releases[0].version = "v0.2.25";
    writeFileSync(join(root, "release-notes.json"), JSON.stringify(document));
    assert.throws(() => validateReleaseResources(root), /版本与安装包不一致/);
    config.bundle.resources = {};
    writeFileSync(join(gui, "tauri.release.conf.json"), JSON.stringify(config));
    assert.throws(() => validateReleaseResources(root), /未正确包含/);
  } finally { rmSync(root, { recursive: true }); }
});
