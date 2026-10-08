import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { editionBuildPlan, readProjectEdition } from "./project-edition.mjs";

/** 用隔离目录验证两个产品，不依赖真实凭据、网络或客户工作区。 */
function productFixture(edition, callback) {
  const root = mkdtempSync(join(tmpdir(), "bifang-edition-"));
  const product = { schemaVersion: 1, edition, productName: edition === "xhs" ? "毕方小红书笔记助手" : "毕方内容生产助手",
    identifier: `com.arden.bifang-${edition === "xhs" ? "xiaohongshu-assistant" : "content-production-assistant"}` };
  const config = join(root, "bifang_content_production_assistant_gui/src-tauri");
  mkdirSync(config, { recursive: true });
  writeFileSync(join(root, "product-edition.json"), JSON.stringify(product));
  writeFileSync(join(config, "tauri.conf.json"), JSON.stringify({ ...product, app: { windows: [{ label: "main", title: product.productName }] } }));
  try { callback(root, product, config); } finally { rmSync(root, { recursive: true }); }
}

for (const edition of ["seo", "xhs"]) {
  test(`${edition}_build_without_edition_argument_uses_its_own_identity`, () => {
    productFixture(edition, (root, product) => {
      const plan = editionBuildPlan([], "win32", root);
      assert.equal(plan.identifier, product.identifier);
      assert.equal(plan.edition, edition);
      assert.ok(plan.args.includes("nsis"));
      assert.ok(!plan.args.includes("--features"));
    });
  });
  test(`${edition}_build_rejects_the_other_product_before_launch`, () => {
    productFixture(edition, (root) => {
      assert.throws(() => editionBuildPlan([edition === "seo" ? "xhs" : "seo"], "win32", root), /不能构建另一个产品/);
      assert.throws(() => editionBuildPlan(["--unknown"], "win32", root), /无效参数/);
    });
  });
}

test("mismatched_installer_identity_is_rejected", () => {
  productFixture("xhs", (root, product, config) => {
    writeFileSync(join(config, "tauri.conf.json"), JSON.stringify({ ...product, identifier: "com.wrong.application", app: { windows: [] } }));
    assert.throws(() => readProjectEdition(root), /不一致/);
  });
});

test("windows_cross_build_preserves_fixed_identity", () => {
  productFixture("xhs", (root) => {
    const plan = editionBuildPlan(["--windows-cross", "--check"], "darwin", root);
    assert.equal(plan.checkOnly, true);
    assert.equal(plan.edition, "xhs");
    assert.ok(plan.args.includes("cargo-xwin"));
    assert.ok(plan.args.includes("src-tauri/tauri.release.conf.json"));
  });
});
