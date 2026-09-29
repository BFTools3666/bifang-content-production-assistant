import { createFileRoute } from "@tanstack/react-router";
import { WriterPage } from "../research/WriterPage";

/** 关键词、作品、风格与可编辑提示词的原创生成路由。 */
export const Route = createFileRoute("/writer")({ component: WriterPage });
