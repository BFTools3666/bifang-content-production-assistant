import { createFileRoute } from "@tanstack/react-router";
import { WriterPage } from "../research/WriterPage";

/** 首页先处理单篇文章，批量工作台由页面内的独立入口打开。 */
export const Route = createFileRoute("/")({ component: WriterPage });
