import { createFileRoute } from "@tanstack/react-router";
import { StyleLibraryPage } from "../research/StyleLibraryPage";

/** 保存、编辑和应用多个对标风格的固定本地路由。 */
export const Route = createFileRoute("/styles")({ component: StyleLibraryPage });
