import { createFileRoute } from "@tanstack/react-router";
import { ResearchDraftsPage } from "../research/ResearchDraftsPage";

/** 查看、编辑和导出真实二创草稿的固定本地路由。 */
export const Route = createFileRoute("/drafts")({ component: ResearchDraftsPage });
