import type { ArticleTask } from "./workbenchTypes";

/** 与 core 重试门禁一致：中断或明确标记可重试的失败才允许普通重试。 */
export function hasRetryableStage(task: ArticleTask): boolean {
  return Object.values(task.stages).some(
    (progress) =>
      (progress.status === "failed" || progress.status === "interrupted") &&
      progress.failure?.retryable === true,
  );
}
