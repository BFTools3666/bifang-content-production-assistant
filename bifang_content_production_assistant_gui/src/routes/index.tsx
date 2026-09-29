import { createFileRoute } from "@tanstack/react-router";
import { WriterPage } from "../research/WriterPage";
/** 首页直接进入运营人员的文章写作流程。 */
export const Route = createFileRoute("/")({ component: WriterPage });
