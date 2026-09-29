import { createFileRoute } from "@tanstack/react-router";

import { RunHistoryPage } from "../workbench/RunHistoryPage";

/** 渲染最近批量运行的持久记录页面。 */
export const Route = createFileRoute("/runs")({ component: RunHistoryPage });
