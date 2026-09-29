import { createFileRoute } from "@tanstack/react-router";

import { ArticleWorkbenchPage } from "../workbench/ArticleWorkbenchPage";

/** 渲染批量 AI 内容工作台主页面。 */
export const Route = createFileRoute("/")({ component: ArticleWorkbenchPage });
