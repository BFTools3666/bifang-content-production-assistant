import { createFileRoute } from "@tanstack/react-router";
import { ArticleWorkbenchPage } from "../workbench/ArticleWorkbenchPage";
/** 首页与命名一致，直接展示标题、正文和成品的批量生产工作台。 */
export const Route = createFileRoute("/")({ component: ArticleWorkbenchPage });
