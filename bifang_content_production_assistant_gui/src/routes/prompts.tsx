import { createFileRoute } from "@tanstack/react-router";

import { PromptTemplatesPage } from "../workbench/PromptTemplatesPage";

/** 渲染版本化 Prompt 模板管理页面。 */
export const Route = createFileRoute("/prompts")({
  component: PromptTemplatesPage,
});
