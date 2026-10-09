import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { validateReleaseResources } from "./release-resources.mjs";

/** 本项目配置门禁保留固定日志映射和明确的许可证映射，不放宽业务读 IPC。 */
const root = fileURLToPath(new URL("../", import.meta.url));
const document = validateReleaseResources(root);
const bytes = readFileSync(new URL("../release-notes.json", import.meta.url));
console.log(JSON.stringify({ version: document.releases[0].version, releaseNotesSha256: createHash("sha256").update(bytes).digest("hex"), resources: "validated" }));
