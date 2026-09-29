import { describe, expect, it } from "vitest";

import { hasRetryableStage } from "./articleRetry";
import type { ArticleTask } from "./workbenchTypes";

/** 只构造重试门禁使用的最小内容阶段视图。 */
function taskWithImageFailure(retryable: boolean): ArticleTask {
  return {
    id: "article-a",
    title: "标题",
    keywords: [],
    requirements: "",
    packageVersion: 1,
    rowVersion: 1,
    reviewStatus: "draft",
    createdAtMs: 0,
    updatedAtMs: 0,
    attemptHistory: [],
    approvedHistory: [],
    packageHistory: [],
    artifacts: {
      markdown: null,
      summary: null,
      html: null,
      imageEditLineage: null,
      imageEditRevision: 0,
      articleLineage: null,
      summaryLineage: null,
      imageLineage: null,
      formatLineage: null,
    },
    stages: {
      article: {
        activeAttemptId: null,
        artifactVersion: null,
        attempts: 0,
        failure: null,
        status: "notStarted",
      },
      summary: {
        activeAttemptId: null,
        artifactVersion: null,
        attempts: 0,
        failure: null,
        status: "notStarted",
      },
      image: {
        activeAttemptId: null,
        artifactVersion: null,
        attempts: 1,
        failure: {
          code: "IMAGE_FAILED",
          message: "图片失败",
          retryAfterMs: null,
          retryable,
        },
        status: "failed",
      },
      format: {
        activeAttemptId: null,
        artifactVersion: null,
        attempts: 0,
        failure: null,
        status: "notStarted",
      },
    },
  };
}

describe("article retry eligibility", () => {
  /** 普通失败必须明确声明可恢复，才能显示或启用仅重试失败入口。 */
  it("requires_a_retryable_failure_flag", () => {
    expect(hasRetryableStage(taskWithImageFailure(true))).toBe(true);
    expect(hasRetryableStage(taskWithImageFailure(false))).toBe(false);
  });

  /** 所有中断都由应用所有权恢复产生，保持可普通重试。 */
  it("keeps_interrupted_stages_retryable", () => {
    const task = taskWithImageFailure(false);
    task.stages.image.status = "interrupted";
    if (!task.stages.image.failure) throw new Error("missing image failure");
    task.stages.image.failure.retryable = true;
    expect(hasRetryableStage(task)).toBe(true);
  });

  /** 结果未知的中断显式标记不可重试时，界面必须隐藏普通重试入口。 */
  it("blocks_a_non_retryable_interrupted_stage", () => {
    const task = taskWithImageFailure(false);
    task.stages.image.status = "interrupted";
    expect(hasRetryableStage(task)).toBe(false);
  });
});
