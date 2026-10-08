import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { editionBuildPlan } from "./project-edition.mjs";

/** 固定构建本仓库的产品；--check 只核对身份，不启动编译或联网。 */
const plan = editionBuildPlan(process.argv.slice(2), process.platform);
if (plan.checkOnly) {
  console.log(JSON.stringify(plan, null, 2));
} else {
  const guiDirectory = fileURLToPath(new URL("../bifang_content_production_assistant_gui/", import.meta.url));
  const result = spawnSync(process.platform === "win32" ? "pnpm.cmd" : "pnpm", plan.args, {
    cwd: guiDirectory,
    env: { ...process.env, VITE_BIFANG_EDITION: plan.edition },
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}
