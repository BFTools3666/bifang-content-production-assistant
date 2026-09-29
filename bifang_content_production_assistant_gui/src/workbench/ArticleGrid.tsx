import {
  ActionIcon,
  Box,
  Button,
  Center,
  Checkbox,
  Group,
  Loader,
  Stack,
  Table,
  Text,
  Tooltip,
  VisuallyHidden,
} from "@mantine/core";
import { IconEye, IconFilePlus, IconPlayerPlay } from "@tabler/icons-react";
import { useEffect, useState, type ReactElement, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import {
  GridImagePreview,
  GridTextPreview,
  type GridInspectableArtifact,
} from "./GridArtifactPreview";
import { StatusBadge } from "./StatusBadge";
import {
  articleHasOwnedStage,
  missingStageDependency,
  type StageActionError,
  type StageActionTarget,
  type StageProviderState,
} from "./stageGeneration";
import { localizedFailure } from "./workbenchErrors";
import type {
  ArticleView,
  Failure,
  StageAttempt,
  StageStatus,
  WorkflowStage,
} from "./workbenchTypes";
import classes from "./workbench.module.css";

export type {
  StageActionError,
  StageActionTarget,
  StageProviderState,
} from "./stageGeneration";

/** Grid 空结果必须区分工作区真空、筛选无匹配与当前页越界。 */
export type ArticleGridEmptyState = "workspace" | "filtered" | "page";

/** 找到一行最先需要用户处理的安全错误摘要。 */
function firstFailure(article: ArticleView): Failure | null {
  const stages = article.article.stages;
  return (
    stages.article.failure ??
    stages.summary.failure ??
    stages.image.failure ??
    stages.format.failure ??
    null
  );
}

/** 以当前语言格式化持久化毫秒时间。 */
function formatTimestamp(value: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
}

/** 只在运行态建立秒级时钟；读取既有产物或打开详情不会启动计时。 */
function useRunningElapsedSeconds(
  startedAtMs: number | null,
  running: boolean,
): number {
  const [nowMs, setNowMs] = useState(() => Date.now());
  useEffect(() => {
    if (!running) return undefined;
    setNowMs(Date.now());
    const interval = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [running, startedAtMs]);
  if (!running || startedAtMs === null) return 0;
  return Math.max(0, Math.floor((nowMs - startedAtMs) / 1_000));
}

/** 取得该阶段最后一次真实开始的尝试。 */
function latestStageAttempt(
  attempts: StageAttempt[],
  stage: WorkflowStage,
): StageAttempt | null {
  return attempts.findLast((attempt) => attempt.stage === stage) ?? null;
}

/** 终态耗时保留到十分之一秒，不把运行时钟伪装为持久结果。 */
function terminalDurationSeconds(attempt: StageAttempt): string | null {
  if (attempt.finishedAtMs === null) return null;
  const seconds =
    Math.max(0, attempt.finishedAtMs - attempt.startedAtMs) / 1_000;
  return Number.isInteger(seconds) ? String(seconds) : seconds.toFixed(1);
}

/** 阶段格的唯一生成动效、实时耗时和紧凑总 token 展示。 */
function StageGenerationMetrics({
  stage,
  view,
}: {
  stage: WorkflowStage;
  view: ArticleView;
}): ReactElement | null {
  const { t } = useTranslation("app");
  const status = view.article.stages[stage].status;
  const attempt = latestStageAttempt(view.article.attemptHistory, stage);
  const running = status === "running";
  const queued = status === "queued";
  const elapsedSeconds = useRunningElapsedSeconds(
    attempt?.startedAtMs ?? null,
    running,
  );
  const stageName = t(`workbench.grid.${stage}`);

  if (queued || running) {
    return (
      <Group
        data-testid={`stage-metrics-${view.article.id}-${stage}`}
        gap={5}
        role="status"
        wrap="nowrap"
      >
        <Loader
          aria-label={t(
            queued
              ? "workbench.grid.generation_queued"
              : "workbench.grid.generation_running",
            { stage: stageName },
          )}
          size="xs"
        />
        <Text c="dimmed" size="xs">
          {queued
            ? t("workbench.grid.waiting")
            : t("workbench.grid.elapsed", { seconds: elapsedSeconds })}
        </Text>
      </Group>
    );
  }

  if (status === "notStarted" || !attempt) return null;
  const duration = terminalDurationSeconds(attempt);
  const totalTokens = attempt.tokenUsage?.totalTokens;
  const tokenText =
    stage === "format"
      ? t("workbench.grid.local_tokens")
      : totalTokens === null || totalTokens === undefined
        ? t("workbench.grid.token_unavailable")
        : t("workbench.grid.total_tokens", {
            count: totalTokens,
          });
  return (
    <Stack data-testid={`stage-metrics-${view.article.id}-${stage}`} gap={1}>
      <Text c="dimmed" size="xs">
        {duration === null
          ? t("workbench.grid.duration_unavailable")
          : t("workbench.grid.duration", { seconds: duration })}
      </Text>
      <Text c="dimmed" size="xs">
        {tokenText}
      </Text>
    </Stack>
  );
}

/** 状态徽标与单步动作保持为两个独立语义控件。 */
function StageActionCell({
  actionError,
  actionLocked,
  actionTarget,
  onGenerateStage,
  preview,
  providerState,
  stage,
  view,
}: {
  actionError: StageActionError | null;
  actionLocked: boolean;
  actionTarget: StageActionTarget | null;
  onGenerateStage: (
    taskId: string,
    stage: WorkflowStage,
    expectedRowVersion: number,
    expectedPackageVersion: number,
  ) => void;
  preview?: ReactNode;
  providerState: StageProviderState;
  stage: WorkflowStage;
  view: ArticleView;
}): ReactElement {
  const { t } = useTranslation("app");
  const task = view.article;
  const status: StageStatus = task.stages[stage].status;
  const dependency = missingStageDependency(view, stage);
  const rowOwned = articleHasOwnedStage(view);
  const isTarget =
    actionTarget?.taskId === task.id && actionTarget.stage === stage;
  const isGeneratingTarget =
    isTarget && (status === "queued" || status === "running");
  const isInitial = status === "notStarted";
  const providerUnavailable = providerState[stage] !== "ready";
  const disabled =
    actionLocked || rowOwned || dependency !== null || providerUnavailable;
  const stageName = t(`workbench.grid.${stage}`);
  const actionName = isInitial
    ? t("workbench.grid.generate")
    : t("workbench.grid.regenerate");
  let hint = t("workbench.grid.single_stage_hint", { stage: stageName });
  if (dependency === "title") {
    hint = t("workbench.grid.title_required");
  } else if (dependency) {
    hint = t("workbench.grid.dependency_required", {
      stage: t(`workbench.grid.${dependency}`),
    });
  } else if (providerState[stage] === "loading") {
    hint = t("workbench.grid.provider_loading");
  } else if (providerState[stage] === "missing") {
    hint = t("workbench.grid.provider_required", { stage: stageName });
  } else if (rowOwned || actionLocked) {
    hint = t("workbench.grid.action_locked");
  }
  const visibleAction =
    disabled && !actionLocked && !rowOwned ? hint : actionName;
  const localError =
    actionError?.taskId === task.id && actionError.stage === stage
      ? actionError.message
      : null;
  const reasonId = `stage-action-reason-${task.id}-${stage}`;

  return (
    <Stack className={classes.stageCell} gap={5}>
      {preview}
      <StatusBadge scope="stage" status={status} />
      <StageGenerationMetrics stage={stage} view={view} />
      <Tooltip label={hint}>
        <span
          aria-label={disabled ? hint : undefined}
          className={classes.stageActionWrapper}
          tabIndex={disabled ? 0 : undefined}
        >
          <Button
            aria-busy={isGeneratingTarget}
            aria-describedby={disabled ? reasonId : undefined}
            aria-label={t(
              isInitial
                ? "workbench.grid.generate_stage_title"
                : "workbench.grid.regenerate_stage_title",
              { stage: stageName, title: task.title },
            )}
            classNames={{ label: classes.stageActionLabel }}
            disabled={disabled}
            fullWidth
            leftSection={<IconPlayerPlay aria-hidden="true" size={14} />}
            onClick={() =>
              onGenerateStage(
                task.id,
                stage,
                task.rowVersion,
                task.packageVersion,
              )
            }
            size="compact-sm"
            variant="subtle"
          >
            {visibleAction}
          </Button>
          {disabled ? (
            <VisuallyHidden id={reasonId}>{hint}</VisuallyHidden>
          ) : null}
        </span>
      </Tooltip>
      {localError ? (
        <Text c="red" lineClamp={3} role="alert" size="xs">
          {localError}
        </Text>
      ) : null}
    </Stack>
  );
}

/** 用固定六列承载完整文章信息和阶段动作，行本身不代理任何子控件动作。 */
export function ArticleGrid({
  articles,
  emptyState,
  loading,
  selectedIds,
  onInspect,
  onInspectArtifact,
  onGenerateStage,
  onSelectionChange,
  onClearFilters,
  onOpenAdd,
  stageActionError,
  stageActionTarget,
  stageProviderState,
  actionLocked,
}: {
  articles: ArticleView[];
  actionLocked: boolean;
  emptyState: ArticleGridEmptyState;
  loading: boolean;
  selectedIds: Set<string>;
  onInspect: (taskId: string) => void;
  onInspectArtifact: (taskId: string, stage: GridInspectableArtifact) => void;
  onGenerateStage: (
    taskId: string,
    stage: WorkflowStage,
    expectedRowVersion: number,
    expectedPackageVersion: number,
  ) => void;
  onSelectionChange: (taskId: string, selected: boolean) => void;
  onClearFilters: () => void;
  onOpenAdd: () => void;
  stageActionError: StageActionError | null;
  stageActionTarget: StageActionTarget | null;
  stageProviderState: StageProviderState;
}): ReactElement {
  const { t, i18n } = useTranslation("app");

  if (loading) {
    return (
      <Center mih={280} role="status">
        <Stack align="center" gap="sm">
          <Text c="dimmed">{t("workbench.loading")}</Text>
        </Stack>
      </Center>
    );
  }

  if (articles.length === 0) {
    const titleKey =
      emptyState === "workspace"
        ? "workbench.empty.title"
        : emptyState === "filtered"
          ? "workbench.empty.filtered_title"
          : "workbench.empty.page_title";
    const descriptionKey =
      emptyState === "workspace"
        ? "workbench.empty.description"
        : emptyState === "filtered"
          ? "workbench.empty.filtered_description"
          : "workbench.empty.page_description";
    return (
      <Center mih={320} px="lg">
        <Stack align="center" gap="sm" maw={420} ta="center">
          {emptyState === "workspace" ? (
            <IconFilePlus
              aria-hidden="true"
              color="var(--app-text-muted)"
              size={40}
            />
          ) : null}
          <Text fw={600}>{t(titleKey)}</Text>
          <Text c="dimmed" size="sm">
            {t(descriptionKey)}
          </Text>
          {emptyState === "workspace" ? (
            <button
              className={classes.emptyAction}
              onClick={onOpenAdd}
              type="button"
            >
              {t("workbench.actions.add_titles")}
            </button>
          ) : emptyState === "filtered" ? (
            <button
              className={classes.emptyAction}
              onClick={onClearFilters}
              type="button"
            >
              {t("workbench.empty.clear_filters")}
            </button>
          ) : null}
        </Stack>
      </Center>
    );
  }

  return (
    <Box
      className={classes.gridTableContainer}
      data-testid="article-grid-container"
    >
      <Table
        className={classes.articleGrid}
        data-testid="article-grid-table"
        highlightOnHover
        horizontalSpacing="xs"
        layout="fixed"
        stickyHeader
        verticalSpacing="sm"
      >
        <colgroup>
          <col className={classes.gridSelectColumn} />
          <col className={classes.gridInfoColumn} />
          <col span={4} />
        </colgroup>
        <Table.Thead>
          <Table.Tr>
            <Table.Th>{t("workbench.grid.select")}</Table.Th>
            <Table.Th>{t("workbench.grid.title")}</Table.Th>
            <Table.Th>{t("workbench.grid.article")}</Table.Th>
            <Table.Th>{t("workbench.grid.summary")}</Table.Th>
            <Table.Th>{t("workbench.grid.image")}</Table.Th>
            <Table.Th>{t("workbench.grid.format")}</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {articles.map((view) => {
            const task = view.article;
            const failure = firstFailure(view);
            return (
              <Table.Tr data-testid={`article-row-${task.id}`} key={task.id}>
                <Table.Td>
                  <Checkbox
                    aria-label={t("workbench.grid.select_title", {
                      title: task.title,
                    })}
                    checked={selectedIds.has(task.id)}
                    onChange={(event) =>
                      onSelectionChange(task.id, event.currentTarget.checked)
                    }
                  />
                </Table.Td>
                <Table.Td>
                  <Stack className={classes.gridInfoCell} gap={5}>
                    <Group
                      align="flex-start"
                      gap="xs"
                      justify="space-between"
                      wrap="nowrap"
                    >
                      <Text
                        className={classes.gridTitle}
                        fw={600}
                        lineClamp={2}
                        title={task.title}
                      >
                        {task.title}
                      </Text>
                      <Tooltip label={t("workbench.actions.inspect")}>
                        <ActionIcon
                          aria-label={t("workbench.actions.inspect_title", {
                            title: task.title,
                          })}
                          className={classes.gridInspectAction}
                          onClick={() => onInspect(task.id)}
                          variant="subtle"
                        >
                          <IconEye aria-hidden="true" size={18} />
                        </ActionIcon>
                      </Tooltip>
                    </Group>
                    <Group align="center" gap={5} wrap="wrap">
                      <StatusBadge
                        scope="overall"
                        status={view.overallStatus}
                      />
                      <Text c="dimmed" size="xs">
                        {t("workbench.grid.version", {
                          version: task.packageVersion,
                        })}
                      </Text>
                    </Group>
                    <Stack gap={1}>
                      <Text c="dimmed" size="xs">
                        <Text component="span" fw={600} inherit>
                          {t("workbench.grid.created")}
                        </Text>{" "}
                        <time
                          dateTime={new Date(task.createdAtMs).toISOString()}
                        >
                          {formatTimestamp(task.createdAtMs, i18n.language)}
                        </time>
                      </Text>
                      <Text c="dimmed" size="xs">
                        <Text component="span" fw={600} inherit>
                          {t("workbench.grid.updated")}
                        </Text>{" "}
                        <time
                          dateTime={new Date(task.updatedAtMs).toISOString()}
                        >
                          {formatTimestamp(task.updatedAtMs, i18n.language)}
                        </time>
                      </Text>
                    </Stack>
                    {failure ? (
                      <Text c="red" lineClamp={2} mt={4} size="xs">
                        {localizedFailure(failure, t)}
                      </Text>
                    ) : null}
                  </Stack>
                </Table.Td>
                <Table.Td>
                  <StageActionCell
                    actionError={stageActionError}
                    actionLocked={actionLocked}
                    actionTarget={stageActionTarget}
                    onGenerateStage={onGenerateStage}
                    preview={
                      <GridTextPreview
                        onInspect={onInspectArtifact}
                        stage="article"
                        status={task.stages.article.status}
                        taskId={task.id}
                        title={task.title}
                        value={task.artifacts.markdown}
                      />
                    }
                    providerState={stageProviderState}
                    stage="article"
                    view={view}
                  />
                </Table.Td>
                <Table.Td>
                  <StageActionCell
                    actionError={stageActionError}
                    actionLocked={actionLocked}
                    actionTarget={stageActionTarget}
                    onGenerateStage={onGenerateStage}
                    preview={
                      <GridTextPreview
                        onInspect={onInspectArtifact}
                        stage="summary"
                        status={task.stages.summary.status}
                        taskId={task.id}
                        title={task.title}
                        value={task.artifacts.summary}
                      />
                    }
                    providerState={stageProviderState}
                    stage="summary"
                    view={view}
                  />
                </Table.Td>
                <Table.Td>
                  <StageActionCell
                    actionError={stageActionError}
                    actionLocked={actionLocked}
                    actionTarget={stageActionTarget}
                    onGenerateStage={onGenerateStage}
                    preview={
                      <GridImagePreview
                        attemptId={
                          task.artifacts.imageLineage?.attemptId ?? null
                        }
                        editRevision={task.artifacts.imageEditRevision}
                        hasImage={view.hasImage}
                        onInspect={onInspectArtifact}
                        status={task.stages.image.status}
                        taskId={task.id}
                        title={task.title}
                      />
                    }
                    providerState={stageProviderState}
                    stage="image"
                    view={view}
                  />
                </Table.Td>
                <Table.Td>
                  <StageActionCell
                    actionError={stageActionError}
                    actionLocked={actionLocked}
                    actionTarget={stageActionTarget}
                    onGenerateStage={onGenerateStage}
                    providerState={stageProviderState}
                    stage="format"
                    view={view}
                  />
                </Table.Td>
              </Table.Tr>
            );
          })}
        </Table.Tbody>
      </Table>
    </Box>
  );
}
