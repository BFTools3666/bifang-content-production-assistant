import { MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import { createInstance } from "i18next";
import { createStore, Provider as JotaiProvider } from "jotai";
import type { ReactElement } from "react";
import { I18nextProvider } from "react-i18next";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import type {
  ArticleView,
  ArtifactLineage,
  StageProgress,
  WorkflowStage,
} from "./workbenchTypes";

/** 为 Mantine 测试提供不执行真实测量的观察器。 */
export class TestResizeObserver implements ResizeObserver {
  /** 测试销毁时没有真实观察资源。 */
  disconnect(): void {}
  /** jsdom 不执行真实布局测量。 */
  observe(): void {}
  /** jsdom 允许停止观察但没有持久状态。 */
  unobserve(): void {}
}

/** 创建未开始的阶段状态。 */
function emptyStage(): StageProgress {
  return {
    activeAttemptId: null,
    artifactVersion: null,
    attempts: 0,
    failure: null,
    status: "notStarted",
  };
}

/** 创建用于批量选择回归的一条真实结构草稿任务。 */
export function draftArticle(id: string, title: string): ArticleView {
  return {
    article: {
      approvedHistory: [],
      artifacts: {
        articleLineage: null,
        formatLineage: null,
        html: null,
        imageEditLineage: null,
        imageEditRevision: 0,
        imageLineage: null,
        markdown: null,
        summary: null,
        summaryLineage: null,
      },
      createdAtMs: 1_789_123_456_000,
      id,
      attemptHistory: [],
      keywords: [],
      packageHistory: [],
      packageVersion: 1,
      requirements: "",
      reviewStatus: "draft",
      rowVersion: 1,
      stages: {
        article: emptyStage(),
        format: emptyStage(),
        image: emptyStage(),
        summary: emptyStage(),
      },
      title,
      updatedAtMs: 1_789_123_456_000,
    },
    hasImage: false,
    overallStatus: "draft",
  };
}

/** 创建与一次实际阶段尝试绑定的产物血缘。 */
function lineage(stage: WorkflowStage, attemptId: string): ArtifactLineage {
  return {
    attemptId,
    model: stage === "format" ? null : "fixture-v1",
    packageVersion: 1,
    providerId: stage === "format" ? null : "provider-e2e-fixture",
    providerRevision: stage === "format" ? null : 1,
    sourceArticleVersion: stage === "article" ? null : 1,
    sourceSummaryVersion: stage === "image" ? 1 : null,
    templateId: stage === "format" ? null : `prompt-${stage}`,
    templateVersion: stage === "format" ? null : 1,
  };
}

/** 构造配图首轮失败、其他成功产物仍被保留的文章。 */
export function failedArticle(
  failureCode = "PROVIDER_TIMEOUT",
  retryable = true,
): ArticleView {
  const view = draftArticle("article-a", "企业知识库搭建指南");
  const articleLineage = lineage("article", "attempt-article-1");
  const summaryLineage = lineage("summary", "attempt-summary-1");
  view.article.artifacts = {
    articleLineage,
    formatLineage: null,
    html: null,
    imageEditLineage: null,
    imageEditRevision: 0,
    imageLineage: null,
    markdown: "# 企业知识库搭建指南",
    summary: "文章简介",
    summaryLineage,
  };
  view.article.stages.article = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 1,
    failure: null,
    status: "succeeded",
  };
  view.article.stages.summary = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 1,
    failure: null,
    status: "succeeded",
  };
  const imageFailure = {
    code: failureCode,
    message: "服务响应超时",
    retryAfterMs: 1_000,
    retryable,
  };
  view.article.stages.image = {
    activeAttemptId: null,
    artifactVersion: null,
    attempts: 1,
    failure: imageFailure,
    status: "failed",
  };
  view.article.attemptHistory = [
    {
      failure: imageFailure,
      finishedAtMs: 1_789_123_456_500,
      id: "attempt-image-1",
      lineage: null,
      packageVersion: 1,
      stage: "image",
      startedAtMs: 1_789_123_456_000,
      status: "failed",
      tokenUsage: null,
    },
  ];
  view.overallStatus = "partialFailure";
  return view;
}

/** 构造精确重试后已经满足人工审核条件的同一文章。 */
export function pendingArticle(): ArticleView {
  const view = failedArticle();
  const imageLineage = lineage("image", "attempt-image-2");
  const formatLineage = lineage("format", "attempt-format-1");
  view.article.artifacts.imageLineage = imageLineage;
  view.article.artifacts.formatLineage = formatLineage;
  view.article.artifacts.html = "<h1>企业知识库搭建指南</h1>";
  view.article.stages.image = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 2,
    failure: null,
    status: "succeeded",
  };
  view.article.stages.format = {
    activeAttemptId: null,
    artifactVersion: 1,
    attempts: 1,
    failure: null,
    status: "succeeded",
  };
  view.article.attemptHistory.push(
    {
      failure: null,
      finishedAtMs: 1_789_123_457_500,
      id: "attempt-image-2",
      lineage: imageLineage,
      packageVersion: 1,
      stage: "image",
      startedAtMs: 1_789_123_457_000,
      status: "succeeded",
      tokenUsage: {
        inputTokens: null,
        outputTokens: 50,
        totalTokens: 50,
      },
    },
    {
      failure: null,
      finishedAtMs: 1_789_123_458_000,
      id: "attempt-format-1",
      lineage: formatLineage,
      packageVersion: 1,
      stage: "format",
      startedAtMs: 1_789_123_457_600,
      status: "succeeded",
      tokenUsage: {
        inputTokens: 0,
        outputTokens: 0,
        totalTokens: 0,
      },
    },
  );
  view.article.reviewStatus = "pending";
  view.hasImage = true;
  view.overallStatus = "pendingReview";
  return view;
}

/** 在真实根级状态容器下渲染工作台页面。 */
export async function renderWorkbench(node: ReactElement): Promise<void> {
  const instance = createInstance();
  await instance.init({
    fallbackLng: "en-US",
    lng: "zh-CN",
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={instance}>
      <JotaiProvider store={createStore()}>
        <QueryClientProvider client={queryClient}>
          <MantineProvider env="test">{node}</MantineProvider>
        </QueryClientProvider>
      </JotaiProvider>
    </I18nextProvider>,
  );
}
