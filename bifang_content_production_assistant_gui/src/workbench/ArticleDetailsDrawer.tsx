import {
  Alert,
  Badge,
  Button,
  Divider,
  Drawer,
  Group,
  Paper,
  ScrollArea,
  Stack,
  Tabs,
  Text,
  Textarea,
  Title,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconArrowBackUp,
  IconCheck,
  IconPlayerPlay,
  IconRefresh,
} from "@tabler/icons-react";
import type { TFunction } from "i18next";
import { useEffect, useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { StatusBadge } from "./StatusBadge";
import { ArticleFormatViewer } from "./ArticleFormatViewer";
import { copyArtifactText } from "./artifactClipboard";
import type { ImageEditorSaveRequest } from "./ArticleImageEditor";
import { ArticleImageEditorModal } from "./ArticleImageEditorModal";
import { hasRetryableStage } from "./articleRetry";
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
  ArtifactLineage,
  RunRecord,
  StageAttempt,
  WorkflowStage,
} from "./workbenchTypes";
import { useDetailDrawerWidth } from "./detailDrawerWidth";
import classes from "./workbench.module.css";

/** 文章详情抽屉中可直接定位的业务页签。 */
type DetailsTab =
  "inputs" | "article" | "summary" | "image" | "formats" | "history";

/** 详情内一次只执行一个显式动作；key 只控制该按钮自己的 loading。 */
type DetailActionKey =
  | "approve"
  | "regenerate"
  | "retry"
  | "returnToDraft"
  | "saveArticle"
  | "saveInputs"
  | "saveSummary";

/** 记录当前详情动作，避免把全局批次状态误显示为按钮加载。 */
interface PendingDetailAction {
  key: DetailActionKey;
  taskId: string;
}

/** 展示不会泄露正文和密钥的生成血缘摘要。 */
function LineageSummary({
  lineage,
}: {
  lineage: ArtifactLineage | null;
}): ReactElement | null {
  const { t } = useTranslation("app");
  if (!lineage) return null;
  return (
    <Paper p="sm" radius="md" withBorder>
      <Stack gap={4}>
        <Text c="dimmed" size="xs">
          {t("details.lineage")}
        </Text>
        <Text size="xs">
          {t("details.lineage_summary", {
            model: lineage.model ?? t("details.local_processing"),
            service: lineage.providerId
              ? `${lineage.providerId} r${lineage.providerRevision ?? "?"}`
              : t("details.local_processing"),
            template: lineage.templateId
              ? `${lineage.templateId} v${lineage.templateVersion ?? "?"}`
              : t("details.local_processing"),
          })}
        </Text>
        <Text c="dimmed" size="xs">
          {t("details.lineage_source", {
            attemptId: lineage.attemptId,
            packageVersion: lineage.packageVersion,
            summarySourceVersion:
              lineage.sourceSummaryVersion ??
              t("details.source_not_applicable"),
            sourceVersion:
              lineage.sourceArticleVersion ?? t("details.source_self"),
          })}
        </Text>
      </Stack>
    </Paper>
  );
}

/** 以当前界面语言格式化持久毫秒时间。 */
function formatTime(value: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

/** 把阶段枚举映射到详情页已有的用户可见标签。 */
function stageLabel(stage: WorkflowStage, t: TFunction<"app">): string {
  return t(`details.tabs.${stage === "format" ? "formats" : stage}`);
}

/** 只为仍在运行的历史条目建立秒级时钟。 */
function useAttemptDuration(attempt: StageAttempt): {
  running: boolean;
  seconds: string;
} {
  const running = attempt.status === "running";
  const [nowMs, setNowMs] = useState(() => Date.now());
  useEffect(() => {
    if (!running) return undefined;
    setNowMs(Date.now());
    const interval = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [attempt.id, attempt.startedAtMs, running]);
  const endMs = running ? nowMs : attempt.finishedAtMs;
  const durationMs =
    endMs === null ? null : Math.max(0, endMs - attempt.startedAtMs);
  const seconds = durationMs === null ? "" : String(durationMs / 1_000);
  return { running, seconds };
}

/** 只展示 Provider 实际返回的 token 字段；缺失值保持“未提供”。 */
function AttemptTokenUsage({
  attempt,
}: {
  attempt: StageAttempt;
}): ReactElement {
  const { t } = useTranslation("app");
  if (attempt.stage === "format") {
    return (
      <Text c="dimmed" size="xs">
        {t("details.local_token_usage")}
      </Text>
    );
  }
  if (attempt.status === "running") {
    return (
      <Text c="dimmed" size="xs">
        {t("details.token_pending")}
      </Text>
    );
  }
  if (attempt.tokenUsage === null) {
    return (
      <Text c="dimmed" size="xs">
        {t("details.token_unavailable")}
      </Text>
    );
  }
  const unavailable = t("details.usage_value_unavailable");
  return (
    <Text c="dimmed" size="xs">
      {t("details.token_usage", {
        input: attempt.tokenUsage.inputTokens ?? unavailable,
        output: attempt.tokenUsage.outputTokens ?? unavailable,
        total: attempt.tokenUsage.totalTokens ?? unavailable,
      })}
    </Text>
  );
}

/** 展示一次持久阶段尝试及其成功血缘或稳定失败码。 */
function AttemptHistoryEntry({
  attempt,
}: {
  attempt: StageAttempt;
}): ReactElement {
  const { t, i18n } = useTranslation("app");
  const duration = useAttemptDuration(attempt);
  return (
    <Paper data-testid={`attempt-${attempt.id}`} p="sm" radius="md" withBorder>
      <Stack gap={5}>
        <Group justify="space-between" wrap="wrap">
          <Text fw={600} size="sm">
            {t("details.attempt_entry", {
              stage: stageLabel(attempt.stage, t),
              status: t(`attemptStatus.${attempt.status}`),
              version: attempt.packageVersion,
            })}
          </Text>
          <StatusBadge scope="stage" status={attempt.status} />
        </Group>
        <Text ff="monospace" size="xs">
          {t("details.attempt_id", { attemptId: attempt.id })}
        </Text>
        <Text c="dimmed" size="xs">
          {t("details.attempt_time", {
            finished: attempt.finishedAtMs
              ? formatTime(attempt.finishedAtMs, i18n.language)
              : t("details.attempt_running"),
            started: formatTime(attempt.startedAtMs, i18n.language),
          })}
        </Text>
        <Text c="dimmed" size="xs">
          {duration.seconds
            ? t(
                duration.running
                  ? "details.attempt_elapsed"
                  : "details.attempt_duration",
                { seconds: duration.seconds },
              )
            : t("details.attempt_duration_unavailable")}
        </Text>
        <AttemptTokenUsage attempt={attempt} />
        {attempt.failure ? (
          <Text c="red" size="xs">
            {localizedFailure(attempt.failure, t)}
          </Text>
        ) : null}
        <LineageSummary lineage={attempt.lineage} />
      </Stack>
    </Paper>
  );
}

/** 详情页复用 Grid 的依赖、Provider、锁和精确单步语义。 */
function StageGenerationAction({
  actionError,
  actionLocked,
  actionTarget,
  onGenerateStage,
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
  providerState: StageProviderState;
  stage: WorkflowStage;
  view: ArticleView;
}): ReactElement {
  const { t } = useTranslation("app");
  const task = view.article;
  const dependency = missingStageDependency(view, stage);
  const rowOwned = articleHasOwnedStage(view);
  const isTarget =
    actionTarget?.taskId === task.id && actionTarget.stage === stage;
  const isInitial = task.stages[stage].status === "notStarted";
  const providerUnavailable = providerState[stage] !== "ready";
  const disabled =
    actionLocked || rowOwned || dependency !== null || providerUnavailable;
  const stageName = t(`workbench.grid.${stage}`);
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
  const localError =
    actionError?.taskId === task.id && actionError.stage === stage
      ? actionError.message
      : null;
  const hintId = `detail-stage-action-hint-${task.id}-${stage}`;

  return (
    <Stack gap={5}>
      <Button
        aria-busy={isTarget}
        aria-describedby={hintId}
        aria-label={t(
          isInitial
            ? "workbench.grid.generate_stage_title"
            : "workbench.grid.regenerate_stage_title",
          { stage: stageName, title: task.title },
        )}
        disabled={disabled}
        leftSection={<IconPlayerPlay aria-hidden="true" size={16} />}
        loading={isTarget}
        onClick={() =>
          onGenerateStage(task.id, stage, task.rowVersion, task.packageVersion)
        }
        variant="default"
      >
        {t(isInitial ? "details.generate_stage" : "details.regenerate_stage", {
          stage: stageName,
        })}
      </Button>
      <Text c={disabled ? "orange" : "dimmed"} id={hintId} size="xs">
        {hint}
      </Text>
      {localError ? (
        <Text c="red" role="alert" size="xs">
          {localError}
        </Text>
      ) : null}
    </Stack>
  );
}

/** 一条文章的产物、审核和精确恢复界面。 */
export function ArticleDetailsDrawer({
  activeTab,
  actionsLocked,
  imageEditorActionsLocked,
  article,
  opened,
  onActiveTabChange,
  onApprove,
  onClose,
  onGenerateStage,
  onRegenerate,
  onRetry,
  onReturnToDraft,
  runHistory,
  onSave,
  onSaveInputs,
  onSaveImage,
  stageActionError,
  stageActionTarget,
  stageProviderState,
}: {
  activeTab: DetailsTab;
  actionsLocked: boolean;
  imageEditorActionsLocked: boolean;
  article: ArticleView | null;
  opened: boolean;
  onActiveTabChange: (tab: DetailsTab) => void;
  onApprove: (taskId: string) => Promise<void>;
  onClose: () => void;
  onGenerateStage: (
    taskId: string,
    stage: WorkflowStage,
    expectedRowVersion: number,
    expectedPackageVersion: number,
  ) => void;
  onRegenerate: (taskId: string) => Promise<void>;
  onRetry: (taskId: string) => Promise<void>;
  onReturnToDraft: (taskId: string) => Promise<void>;
  runHistory: RunRecord[];
  onSave: (
    taskId: string,
    markdown: string | null,
    summary: string | null,
  ) => Promise<void>;
  onSaveInputs: (
    taskId: string,
    keywords: string[],
    requirements: string,
  ) => Promise<void>;
  onSaveImage: (request: ImageEditorSaveRequest) => Promise<void>;
  stageActionError: StageActionError | null;
  stageActionTarget: StageActionTarget | null;
  stageProviderState: StageProviderState;
}): ReactElement {
  const { t, i18n } = useTranslation("app");
  const {
    maximumWidth,
    minimumWidth,
    onResizeKeyDown,
    onResizePointerDown,
    resizing,
    width,
  } = useDetailDrawerWidth();
  const [markdown, setMarkdown] = useState("");
  const [summary, setSummary] = useState("");
  const [keywords, setKeywords] = useState("");
  const [requirements, setRequirements] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [imageEditorLayerOpened, setImageEditorLayerOpened] = useState(false);
  const [pendingAction, setPendingAction] =
    useState<PendingDetailAction | null>(null);
  const currentArticleId = article?.article.id ?? null;
  const currentArticleIdRef = useRef<string | null>(currentArticleId);
  currentArticleIdRef.current = currentArticleId;

  useEffect(() => {
    setMarkdown(article?.article.artifacts.markdown ?? "");
    setSummary(article?.article.artifacts.summary ?? "");
    setKeywords(article?.article.keywords.join("、") ?? "");
    setRequirements(article?.article.requirements ?? "");
    setFeedback(null);
    setError(null);
  }, [
    article?.article.id,
    article?.article.packageVersion,
    article?.article.rowVersion,
  ]);

  const perform = async (
    key: DetailActionKey,
    taskId: string,
    action: () => Promise<void>,
    successMessage: string,
  ): Promise<void> => {
    if (pendingAction !== null) return;
    setPendingAction({ key, taskId });
    setError(null);
    setFeedback(null);
    try {
      await action();
      if (currentArticleIdRef.current === taskId) {
        setFeedback(successMessage);
      }
    } catch (reason) {
      if (currentArticleIdRef.current === taskId) {
        setError(reason instanceof Error ? reason.message : t("common.failed"));
      }
    } finally {
      setPendingAction((current) =>
        current?.key === key && current.taskId === taskId ? null : current,
      );
    }
  };

  const copyArtifact = async (value: string, format: string): Promise<void> => {
    setError(null);
    setFeedback(null);
    try {
      await copyArtifactText(value);
      setFeedback(t("details.feedback.copied", { format }));
    } catch {
      setError(t("details.copy_failed"));
    }
  };

  const task = article?.article ?? null;
  const retryable = task !== null && hasRetryableStage(task);
  const actionLocked =
    actionsLocked ||
    pendingAction !== null ||
    article?.overallStatus === "queued" ||
    article?.overallStatus === "running";
  const isPending = (key: DetailActionKey): boolean =>
    pendingAction?.key === key && pendingAction.taskId === task?.id;
  const relatedRuns = task
    ? runHistory.filter((run) => run.taskIds.includes(task.id))
    : [];

  return (
    <Drawer
      classNames={{
        body: classes.detailDrawerBody,
        content: classes.detailDrawerContent,
        header: classes.detailDrawerHeader,
      }}
      closeButtonProps={{ "aria-label": t("common.close") }}
      closeOnClickOutside={!imageEditorLayerOpened}
      closeOnEscape={!imageEditorLayerOpened}
      data-testid="article-details-drawer"
      onClose={onClose}
      opened={opened}
      position="right"
      size={`${width}px`}
      title={t("details.drawer_title")}
      transitionProps={{ duration: 0 }}
    >
      <div
        aria-label={t("details.resize_handle")}
        aria-orientation="vertical"
        aria-valuemax={maximumWidth}
        aria-valuemin={minimumWidth}
        aria-valuenow={width}
        className={classes.detailDrawerResizeHandle}
        data-resizing={resizing || undefined}
        onKeyDown={onResizeKeyDown}
        onPointerDown={onResizePointerDown}
        role="separator"
        tabIndex={0}
        title={t("details.resize_handle")}
      />
      {article && task ? (
        <Stack className={classes.detailDrawerLayout} gap="md">
          <Stack gap={6}>
            <Group align="center" justify="space-between" wrap="wrap">
              <Badge variant="outline">
                {t("workbench.grid.version", { version: task.packageVersion })}
              </Badge>
              <StatusBadge scope="overall" status={article.overallStatus} />
            </Group>
            <Title order={2} size="h3">
              {task.title}
            </Title>
          </Stack>

          <Group gap="xs" wrap="wrap">
            {retryable ? (
              <Button
                disabled={actionLocked}
                leftSection={<IconRefresh aria-hidden="true" size={17} />}
                loading={isPending("retry")}
                onClick={() =>
                  void perform(
                    "retry",
                    task.id,
                    () => onRetry(task.id),
                    t("details.feedback.retry_started"),
                  )
                }
                variant="default"
              >
                {t("workbench.actions.retry_failed")}
              </Button>
            ) : null}
            <Button
              disabled={actionLocked}
              leftSection={<IconRefresh aria-hidden="true" size={17} />}
              loading={isPending("regenerate")}
              onClick={() =>
                void perform(
                  "regenerate",
                  task.id,
                  () => onRegenerate(task.id),
                  t("details.feedback.regenerate_started"),
                )
              }
              variant="default"
            >
              {t("workbench.actions.regenerate")}
            </Button>
            {task.reviewStatus === "pending" ? (
              <Button
                disabled={actionLocked}
                leftSection={<IconArrowBackUp aria-hidden="true" size={17} />}
                loading={isPending("returnToDraft")}
                onClick={() =>
                  void perform(
                    "returnToDraft",
                    task.id,
                    () => onReturnToDraft(task.id),
                    t("details.feedback.returned"),
                  )
                }
                variant="default"
              >
                {t("workbench.actions.return_to_draft")}
              </Button>
            ) : null}
            <Button
              disabled={actionLocked || task.reviewStatus !== "pending"}
              leftSection={<IconCheck aria-hidden="true" size={17} />}
              loading={isPending("approve")}
              onClick={() =>
                void perform(
                  "approve",
                  task.id,
                  () => onApprove(task.id),
                  t("details.feedback.approved"),
                )
              }
            >
              {t("workbench.actions.approve")}
            </Button>
          </Group>

          {error ? (
            <Alert
              color="red"
              icon={<IconAlertCircle aria-hidden="true" size={18} />}
              role="alert"
            >
              {error}
            </Alert>
          ) : null}
          {feedback ? (
            <Alert color="green" role="status">
              {feedback}
            </Alert>
          ) : null}

          <Tabs
            className={classes.detailDrawerTabs}
            onChange={(value) => {
              if (
                value === "article" ||
                value === "inputs" ||
                value === "summary" ||
                value === "image" ||
                value === "formats" ||
                value === "history"
              ) {
                onActiveTabChange(value);
              }
            }}
            value={activeTab}
          >
            <Tabs.List className={classes.detailDrawerTabsList}>
              <Tabs.Tab value="inputs">{t("details.tabs.inputs")}</Tabs.Tab>
              <Tabs.Tab value="article">{t("details.tabs.article")}</Tabs.Tab>
              <Tabs.Tab value="summary">{t("details.tabs.summary")}</Tabs.Tab>
              <Tabs.Tab value="image">{t("details.tabs.image")}</Tabs.Tab>
              <Tabs.Tab value="formats">{t("details.tabs.formats")}</Tabs.Tab>
              <Tabs.Tab value="history">{t("details.tabs.history")}</Tabs.Tab>
            </Tabs.List>

            <Tabs.Panel pt="md" value="inputs">
              <Stack gap="md">
                <Textarea
                  autosize
                  disabled={actionLocked}
                  label={t("details.keywords_label")}
                  minRows={2}
                  onChange={(event) => setKeywords(event.currentTarget.value)}
                  placeholder={t("details.keywords_placeholder")}
                  value={keywords}
                />
                <Textarea
                  autosize
                  disabled={actionLocked}
                  label={t("details.requirements_label")}
                  maxRows={10}
                  minRows={5}
                  onChange={(event) =>
                    setRequirements(event.currentTarget.value)
                  }
                  placeholder={t("details.requirements_placeholder")}
                  value={requirements}
                />
                <Text c="dimmed" size="xs">
                  {t("details.inputs_edit_warning")}
                </Text>
                <Button
                  disabled={actionLocked}
                  loading={isPending("saveInputs")}
                  onClick={() =>
                    void perform(
                      "saveInputs",
                      task.id,
                      () =>
                        onSaveInputs(
                          task.id,
                          keywords
                            .split(/[，,、\n]/u)
                            .map((value) => value.trim())
                            .filter((value) => value.length > 0),
                          requirements,
                        ),
                      t("details.feedback.inputs_saved"),
                    )
                  }
                >
                  {t("details.save_inputs")}
                </Button>
              </Stack>
            </Tabs.Panel>

            <Tabs.Panel pt="md" value="article">
              <Stack gap="md">
                <Textarea
                  autosize
                  disabled={actionLocked}
                  label={t("details.article_label")}
                  maxRows={20}
                  minRows={14}
                  onChange={(event) => setMarkdown(event.currentTarget.value)}
                  placeholder={t("details.article_empty")}
                  value={markdown}
                />
                <Text c="dimmed" size="xs">
                  {t("details.article_edit_warning")}
                </Text>
                <LineageSummary lineage={task.artifacts.articleLineage} />
                <Button
                  disabled={actionLocked || markdown.trim().length === 0}
                  loading={isPending("saveArticle")}
                  onClick={() =>
                    void perform(
                      "saveArticle",
                      task.id,
                      () => onSave(task.id, markdown, null),
                      t("details.feedback.article_saved"),
                    )
                  }
                >
                  {t("details.save_article")}
                </Button>
                <StageGenerationAction
                  actionError={stageActionError}
                  actionLocked={actionLocked}
                  actionTarget={stageActionTarget}
                  onGenerateStage={onGenerateStage}
                  providerState={stageProviderState}
                  stage="article"
                  view={article}
                />
              </Stack>
            </Tabs.Panel>

            <Tabs.Panel pt="md" value="summary">
              <Stack gap="md">
                <Textarea
                  autosize
                  disabled={actionLocked}
                  label={t("details.summary_label")}
                  maxRows={12}
                  minRows={8}
                  onChange={(event) => setSummary(event.currentTarget.value)}
                  placeholder={t("details.summary_empty")}
                  value={summary}
                />
                <Text c="dimmed" size="xs">
                  {t("details.summary_edit_warning")}
                </Text>
                <LineageSummary lineage={task.artifacts.summaryLineage} />
                <Button
                  disabled={actionLocked || summary.trim().length === 0}
                  loading={isPending("saveSummary")}
                  onClick={() =>
                    void perform(
                      "saveSummary",
                      task.id,
                      () => onSave(task.id, null, summary),
                      t("details.feedback.summary_saved"),
                    )
                  }
                >
                  {t("details.save_summary")}
                </Button>
                <StageGenerationAction
                  actionError={stageActionError}
                  actionLocked={actionLocked}
                  actionTarget={stageActionTarget}
                  onGenerateStage={onGenerateStage}
                  providerState={stageProviderState}
                  stage="summary"
                  view={article}
                />
              </Stack>
            </Tabs.Panel>

            <Tabs.Panel pt="md" value="image">
              <Stack gap="md">
                <ArticleImageEditorModal
                  actionLocked={imageEditorActionsLocked}
                  active={opened && activeTab === "image"}
                  hasImage={article.hasImage}
                  key={task.id}
                  manualLineage={task.artifacts.imageEditLineage}
                  onLayerChange={setImageEditorLayerOpened}
                  onSave={onSaveImage}
                  refreshKey={`${task.rowVersion}:${task.packageVersion}:${task.artifacts.imageEditRevision}:${task.artifacts.imageLineage?.attemptId ?? "none"}`}
                  taskId={task.id}
                  title={task.title}
                />
                <LineageSummary lineage={task.artifacts.imageLineage} />
                <StageGenerationAction
                  actionError={stageActionError}
                  actionLocked={actionLocked}
                  actionTarget={stageActionTarget}
                  onGenerateStage={onGenerateStage}
                  providerState={stageProviderState}
                  stage="image"
                  view={article}
                />
              </Stack>
            </Tabs.Panel>

            <Tabs.Panel pt="md" value="formats">
              <Stack gap="md">
                <ArticleFormatViewer
                  active={opened && activeTab === "formats"}
                  formatReady={
                    task.stages.format.status === "succeeded" &&
                    task.stages.format.artifactVersion === task.packageVersion
                  }
                  hasImage={article.hasImage}
                  html={task.artifacts.html}
                  imageRefreshKey={`${task.packageVersion}:${task.artifacts.imageEditRevision}:${task.artifacts.imageLineage?.attemptId ?? "none"}`}
                  markdown={task.artifacts.markdown}
                  onCopy={copyArtifact}
                  packageVersion={task.packageVersion}
                  rowVersion={task.rowVersion}
                  taskId={task.id}
                  title={task.title}
                />
                <LineageSummary lineage={task.artifacts.formatLineage} />
                <StageGenerationAction
                  actionError={stageActionError}
                  actionLocked={actionLocked}
                  actionTarget={stageActionTarget}
                  onGenerateStage={onGenerateStage}
                  providerState={stageProviderState}
                  stage="format"
                  view={article}
                />
              </Stack>
            </Tabs.Panel>

            <Tabs.Panel pt="md" value="history">
              <ScrollArea.Autosize mah={430} type="auto">
                <Stack gap="sm">
                  <Paper p="sm" radius="md" withBorder>
                    <Text fw={600}>{t("details.current_attempts")}</Text>
                    <Text c="dimmed" size="sm">
                      {t("details.attempt_summary", {
                        article: task.stages.article.attempts,
                        format: task.stages.format.attempts,
                        image: task.stages.image.attempts,
                        summary: task.stages.summary.attempts,
                      })}
                    </Text>
                  </Paper>

                  <Divider
                    label={t("details.attempt_history")}
                    labelPosition="left"
                  />
                  {task.attemptHistory.length === 0 ? (
                    <Alert color="gray">
                      {t("details.attempt_history_empty")}
                    </Alert>
                  ) : (
                    <Stack data-testid="attempt-history" gap="sm">
                      {task.attemptHistory
                        .slice()
                        .reverse()
                        .map((attempt) => (
                          <AttemptHistoryEntry
                            attempt={attempt}
                            key={attempt.id}
                          />
                        ))}
                    </Stack>
                  )}

                  <Divider
                    label={t("details.run_history")}
                    labelPosition="left"
                  />
                  {relatedRuns.length === 0 ? (
                    <Alert color="gray">{t("details.run_history_empty")}</Alert>
                  ) : (
                    relatedRuns.map((run) => (
                      <Paper key={run.id} p="sm" radius="md" withBorder>
                        <Group justify="space-between" wrap="wrap">
                          <Text fw={600} size="sm">
                            {t("details.run_entry", {
                              runId: run.id,
                              status: t(`status.run.${run.status}`),
                            })}
                          </Text>
                          <StatusBadge scope="run" status={run.status} />
                        </Group>
                        <Text c="dimmed" size="xs">
                          {t("details.run_progress", {
                            completed: run.completedCount,
                            failed: run.failedCount,
                            total: run.taskIds.length,
                          })}
                        </Text>
                      </Paper>
                    ))
                  )}

                  <Divider
                    label={t("details.package_history")}
                    labelPosition="left"
                  />
                  {task.packageHistory.length === 0 ? (
                    <Alert color="gray">
                      {t("details.package_history_empty")}
                    </Alert>
                  ) : (
                    task.packageHistory
                      .slice()
                      .reverse()
                      .map((snapshot) => (
                        <Paper
                          key={`${snapshot.version}-${snapshot.capturedAtMs}`}
                          p="sm"
                          radius="md"
                          withBorder
                        >
                          <Text fw={600}>
                            {t("details.package_snapshot", {
                              status: t(
                                `status.overall.${
                                  snapshot.reviewStatus === "pending"
                                    ? "pendingReview"
                                    : snapshot.reviewStatus
                                }`,
                              ),
                              version: snapshot.version,
                            })}
                          </Text>
                          <Text c="dimmed" size="sm">
                            {formatTime(snapshot.capturedAtMs, i18n.language)}
                          </Text>
                        </Paper>
                      ))
                  )}

                  <Divider
                    label={t("details.approved_history")}
                    labelPosition="left"
                  />
                  {task.approvedHistory.length === 0 ? (
                    <Alert color="gray">{t("details.history_empty")}</Alert>
                  ) : (
                    task.approvedHistory
                      .slice()
                      .reverse()
                      .map((approved) => (
                        <Paper
                          key={`${approved.version}-${approved.approvedAtMs}`}
                          p="sm"
                          radius="md"
                          withBorder
                        >
                          <Text fw={600}>
                            {t("details.approved_version", {
                              version: approved.version,
                            })}
                          </Text>
                          <Text c="dimmed" size="sm">
                            {formatTime(approved.approvedAtMs, i18n.language)}
                          </Text>
                          <Text c="dimmed" size="xs">
                            {t("details.approved_inputs", {
                              keywords:
                                approved.keywords.join("、") ||
                                t("details.empty_value"),
                              requirements:
                                approved.requirements ||
                                t("details.empty_value"),
                              title: approved.title || t("details.empty_value"),
                            })}
                          </Text>
                        </Paper>
                      ))
                  )}
                </Stack>
              </ScrollArea.Autosize>
            </Tabs.Panel>
          </Tabs>
        </Stack>
      ) : (
        <Alert color="gray">{t("details.not_found")}</Alert>
      )}
    </Drawer>
  );
}
