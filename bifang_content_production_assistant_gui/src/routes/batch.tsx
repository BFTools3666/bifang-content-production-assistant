import { createFileRoute } from "@tanstack/react-router";
import { ArticleWorkbenchPage } from "../workbench/ArticleWorkbenchPage";
/** 原有批量工作台完整保留，由写文章入口按需进入。 */
export const Route = createFileRoute("/batch")({ component: ArticleWorkbenchPage });
