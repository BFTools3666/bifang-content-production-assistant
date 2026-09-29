import type { ArticleView, WorkflowStage } from "./workbenchTypes";

/** Grid 与详情共同使用的精确单步动作目标。 */
export interface StageActionTarget {
  taskId: string;
  stage: WorkflowStage;
}

/** 页面读取当前 Provider 后交给阶段动作的可用状态。 */
export type StageProviderState = Record<
  WorkflowStage,
  "loading" | "missing" | "ready"
>;

/** 单步命令失败只在对应文章与阶段附近展示。 */
export interface StageActionError extends StageActionTarget {
  message: string;
}

const WORKFLOW_STAGES: WorkflowStage[] = [
  "article",
  "summary",
  "image",
  "format",
];

/** 返回当前阶段最早缺失的固定 SOP 前置阶段；后端仍会再次权威校验。 */
export function missingStageDependency(
  view: ArticleView,
  stage: WorkflowStage,
): WorkflowStage | "title" | null {
  const task = view.article;
  if (stage === "article") return task.title.trim() ? null : "title";
  if (
    task.stages.article.status !== "succeeded" ||
    !task.artifacts.markdown ||
    task.stages.article.artifactVersion === null
  ) {
    return "article";
  }
  if (stage === "summary") return null;
  if (
    task.stages.summary.status !== "succeeded" ||
    !task.artifacts.summary ||
    task.stages.summary.artifactVersion === null
  ) {
    return "summary";
  }
  if (stage === "image") return null;
  if (
    task.stages.image.status !== "succeeded" ||
    !view.hasImage ||
    task.stages.image.artifactVersion === null
  ) {
    return "image";
  }
  return null;
}

/** 判断该文章是否已经有一个阶段归当前 worker 所有。 */
export function articleHasOwnedStage(view: ArticleView): boolean {
  return WORKFLOW_STAGES.some((stage) =>
    ["queued", "running"].includes(view.article.stages[stage].status),
  );
}
