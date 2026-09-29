import { createFileRoute } from "@tanstack/react-router";

import { ConnectorSettingsPage } from "../workbench/ConnectorSettingsPage";

/** 渲染不回填秘密的云文档连接器配置页面。 */
export const Route = createFileRoute("/connectors")({
  component: ConnectorSettingsPage,
});
