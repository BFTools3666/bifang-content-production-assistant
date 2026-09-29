import { createFileRoute } from "@tanstack/react-router";

import { AiServicesPage } from "../workbench/AiServicesPage";

/** 渲染不回填密钥的 AI 服务配置页面。 */
export const Route = createFileRoute("/providers")({
  component: AiServicesPage,
});
