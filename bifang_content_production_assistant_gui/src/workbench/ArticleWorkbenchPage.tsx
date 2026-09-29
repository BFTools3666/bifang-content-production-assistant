import {
  Alert,
  Badge,
  Button,
  Checkbox,
  Group,
  Modal,
  Paper,
  Select,
  SimpleGrid,
  Stack,
  Text,
  TextInput,
  Title,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconPlayerPlay,
  IconPlus,
  IconRefresh,
  IconSearch,
  IconSettingsAutomation,
} from "@tabler/icons-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { useEffect, useMemo, useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { AddTitlesModal } from "./AddTitlesModal";
import { ArticleDetailsDrawer } from "./ArticleDetailsDrawer";
import {
  ArticleGrid,
  type StageActionError,
  type StageActionTarget,
  type StageProviderState,
} from "./ArticleGrid";
import { BatchStatusStrip } from "./BatchStatusStrip";
import { WorkbenchPagination } from "./WorkbenchPagination";
import { hasRetryableStage } from "./articleRetry";
import {
  addArticleTitles,
  approveArticle,
  cancelActiveBatch,
  generateArticleStage,
  getWorkbench,
  listProviderConfigs,
  pauseActiveBatch,
  previewSelectedBatch,
  regenerateArticle,
  saveArticleImageEdit,
  resumeActiveBatch,
  retryArticle,
  retrySelectedArticles,
  returnArticleToDraft,
  startSelectedBatch,
  updateArticleContent,
  updateArticleInputs,
} from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import {
  articleDetailsTabAtom,
  inspectedArticleIdAtom,
  selectedArticleIdsAtom,
  workbenchSearchAtom,
  workbenchSortAtom,
  workbenchStatusFilterAtom,
  workbenchPageSession,
} from "./workbenchState";
import { deriveVisibleArticles, paginateArticles } from "./workbenchGridModel";
import type {
  AddTitlesResult,
  ArticleView,
  BatchStartResult,
  OverallStatus,
  ProviderConfigView,
  WorkflowStage,
} from "./workbenchTypes";
import classes from "./workbench.module.css";

export { deriveVisibleArticles } from "./workbenchGridModel";

/** Grid 发起单步运行时冻结的文章版本与用户可见标题。 */
interface StageGenerationRequest extends StageActionTarget {
  expectedPackageVersion: number;
  expectedRowVersion: number;
  title: string;
}

/** 配图出站前保留用户看到的 Provider 修订与一次性确认状态。 */
interface ImageStageConfirmation {
  attempted: boolean;
  error: string | null;
  previousOutcomeUncertain: boolean;
  provider: ProviderConfigView;
  request: StageGenerationRequest;
}

const UNCERTAIN_IMAGE_FAILURE_CODES = new Set([
  "PROVIDER_UNKNOWN_OUTCOME",
  "RUN_OUTCOME_UNKNOWN",
  "PROVIDER_INVALID_RESPONSE",
]);

/** 这些图片失败发生在请求可能已经提交之后，再次生成前必须提示潜在重复计费。 */
function hasUncertainImageOutcome(article: ArticleView): boolean {
  if (
    UNCERTAIN_IMAGE_FAILURE_CODES.has(
      article.article.stages.image.failure?.code ?? "",
    )
  ) {
    return true;
  }
  for (const attempt of article.article.attemptHistory.slice().reverse()) {
    if (attempt.stage !== "image") continue;
    if (attempt.status === "succeeded") return false;
    if (UNCERTAIN_IMAGE_FAILURE_CODES.has(attempt.failure?.code ?? "")) {
      return true;
    }
  }
  return false;
}

/** 首版 Grid 工作台：批量录入、选择、预检、运行、恢复与审核。 */
export function ArticleWorkbenchPage(): ReactElement {
  const { t, i18n } = useTranslation("app");
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [search, setSearch] = useAtom(workbenchSearchAtom);
  const [statusFilter, setStatusFilter] = useAtom(workbenchStatusFilterAtom);
  const [sort, setSort] = useAtom(workbenchSortAtom);
  const pageSession = useAtomValue(workbenchPageSession.stateAtom);
  const reconcilePageAfterResult = useSetAtom(
    workbenchPageSession.reconcilePageAfterResultAtom,
  );
  const [selectedIds, setSelectedIds] = useAtom(selectedArticleIdsAtom);
  const [inspectedId, setInspectedId] = useAtom(inspectedArticleIdAtom);
  const [detailsTab, setDetailsTab] = useAtom(articleDetailsTabAtom);
  const [addOpened, setAddOpened] = useState(false);
  const [batchPreview, setBatchPreview] = useState<BatchStartResult | null>(
    null,
  );
  const [actionBusy, setActionBusy] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [stageActionTarget, setStageActionTarget] =
    useState<StageActionTarget | null>(null);
  const [stageActionError, setStageActionError] =
    useState<StageActionError | null>(null);
  const [imageStageConfirmation, setImageStageConfirmation] =
    useState<ImageStageConfirmation | null>(null);
  const actionLock = useRef(false);

  const workbenchQuery = useQuery({
    queryFn: getWorkbench,
    queryKey: ["workbench"],
    refetchInterval: (query) => (query.state.data?.activeRun ? 500 : false),
  });
  const providersQuery = useQuery({
    queryFn: listProviderConfigs,
    queryKey: ["provider-configs"],
  });

  const articles = useMemo(
    () => workbenchQuery.data?.articles ?? [],
    [workbenchQuery.data?.articles],
  );
  const visibleArticles = useMemo(
    () =>
      deriveVisibleArticles(
        articles,
        search,
        statusFilter,
        sort,
        i18n.language,
      ),
    [articles, i18n.language, search, sort, statusFilter],
  );
  const pageArticles = useMemo(
    () =>
      paginateArticles(visibleArticles, pageSession.page, pageSession.pageSize),
    [pageSession.page, pageSession.pageSize, visibleArticles],
  );
  const hasWorkbenchSnapshot = workbenchQuery.data !== undefined;
  const initialLoading = workbenchQuery.isPending && !hasWorkbenchSnapshot;
  const initialError = workbenchQuery.isError && !hasWorkbenchSnapshot;
  const backgroundRefreshing =
    workbenchQuery.isFetching && hasWorkbenchSnapshot;
  const backgroundError = workbenchQuery.isError && hasWorkbenchSnapshot;

  useEffect(() => {
    reconcilePageAfterResult({
      itemCount: pageArticles.length,
      status: workbenchQuery.isFetching
        ? "loading"
        : workbenchQuery.isError
          ? "error"
          : "success",
    });
  }, [
    pageSession.page,
    pageArticles.length,
    reconcilePageAfterResult,
    workbenchQuery.isError,
    workbenchQuery.isFetching,
  ]);
  const selectedVisibleIds = visibleArticles
    .map((view) => view.article.id)
    .filter((id) => selectedIds.has(id));
  const allVisibleSelected =
    visibleArticles.length > 0 &&
    selectedVisibleIds.length === visibleArticles.length;
  const selectionIndeterminate =
    selectedVisibleIds.length > 0 && !allVisibleSelected;
  const inspectedArticle =
    articles.find((view) => view.article.id === inspectedId) ?? null;
  const activeRun = workbenchQuery.data?.activeRun ?? null;
  const hasStageProviders = Boolean(
    providersQuery.data?.some(
      (provider) =>
        provider.enabled && provider.defaultForArticle && provider.hasApiKey,
    ) &&
    providersQuery.data?.some(
      (provider) =>
        provider.enabled && provider.defaultForSummary && provider.hasApiKey,
    ) &&
    providersQuery.data?.some(
      (provider) =>
        provider.enabled && provider.defaultForImage && provider.hasApiKey,
    ),
  );
  const fixtureEnabled = Boolean(
    providersQuery.data?.some(
      (provider) => provider.enabled && provider.isTestFixture,
    ),
  );
  const stageProviders = useMemo(() => {
    const providers = providersQuery.data ?? [];
    const available = (provider: ProviderConfigView): boolean =>
      provider.enabled && (provider.hasApiKey || provider.isTestFixture);
    return {
      article:
        providers.find(
          (provider) => provider.defaultForArticle && available(provider),
        ) ?? null,
      image:
        providers.find(
          (provider) => provider.defaultForImage && available(provider),
        ) ?? null,
      summary:
        providers.find(
          (provider) => provider.defaultForSummary && available(provider),
        ) ?? null,
    };
  }, [providersQuery.data]);
  const stageProviderState: StageProviderState = {
    article: providersQuery.isPending
      ? "loading"
      : stageProviders.article
        ? "ready"
        : "missing",
    format: "ready",
    image: providersQuery.isPending
      ? "loading"
      : stageProviders.image
        ? "ready"
        : "missing",
    summary: providersQuery.isPending
      ? "loading"
      : stageProviders.summary
        ? "ready"
        : "missing",
  };

  const counts = {
    approved: articles.filter((view) => view.overallStatus === "approved")
      .length,
    failed: articles.filter(
      (view) =>
        view.overallStatus === "partialFailure" ||
        view.overallStatus === "interrupted",
    ).length,
    pending: articles.filter((view) => view.overallStatus === "pendingReview")
      .length,
    total: articles.length,
  };

  /** 统一映射窄命令错误、刷新权威查询并防止重复动作。 */
  const performAction = async <T,>(
    action: () => Promise<T>,
    onError?: (message: string) => void,
    preserveRejection = false,
  ): Promise<T> => {
    if (actionLock.current) {
      const message = t("workbench.grid.action_locked");
      if (onError) onError(message);
      else setActionError(message);
      throw new Error(message);
    }
    actionLock.current = true;
    setActionBusy(true);
    setActionError(null);
    try {
      const result = await action();
      await queryClient.invalidateQueries({ queryKey: ["workbench"] });
      await queryClient.invalidateQueries({ queryKey: ["run-history"] });
      return result;
    } catch (reason) {
      const message = localizedCommandError(reason, t);
      if (onError) onError(message);
      else setActionError(message);
      if (preserveRejection) throw reason;
      throw new Error(message);
    } finally {
      actionLock.current = false;
      setActionBusy(false);
    }
  };

  const handleAddTitles = async (titles: string): Promise<AddTitlesResult> => {
    const result = await performAction(() => addArticleTitles(titles));
    setSelectedIds((current) => {
      const next = new Set(current);
      for (const id of result.taskIds) next.add(id);
      return next;
    });
    setFeedback(
      t("workbench.feedback.added", {
        count: result.createdCount,
        duplicates: result.duplicateCount,
      }),
    );
    return result;
  };

  const handlePreview = async (): Promise<void> => {
    setFeedback(null);
    try {
      const preview = await performAction(() =>
        previewSelectedBatch([...selectedIds]),
      );
      setBatchPreview(preview);
    } catch {
      // 就近错误区域已经展示稳定说明。
    }
  };

  const handleStartConfirmed = async (): Promise<void> => {
    if (!batchPreview) return;
    try {
      const result = await performAction(() =>
        startSelectedBatch(batchPreview.queuedTaskIds),
      );
      setBatchPreview(null);
      setFeedback(
        t("workbench.feedback.started", { count: result.queuedCount }),
      );
    } catch {
      // 就近错误区域已经展示稳定说明。
    }
  };

  const handleRetrySelected = async (): Promise<void> => {
    try {
      const result = await performAction(() =>
        retrySelectedArticles([...selectedIds]),
      );
      setFeedback(
        t("workbench.feedback.retry_started", { count: result.queuedCount }),
      );
    } catch {
      // 就近错误区域已经展示稳定说明。
    }
  };

  const hasRetryableSelection = articles.some(
    (article) =>
      selectedIds.has(article.article.id) && hasRetryableStage(article.article),
  );

  /** 调用单步命令并以同步锁、乐观版本和精确错误落点防止重复请求。 */
  const runSingleStage = async (
    request: StageGenerationRequest,
    imageProvider: ProviderConfigView | null,
    onError: (message: string) => void,
  ): Promise<boolean> => {
    if (actionLock.current) return false;
    setStageActionTarget(request);
    setStageActionError(null);
    setFeedback(null);
    try {
      await performAction(
        () =>
          generateArticleStage(
            request.taskId,
            request.stage,
            request.expectedRowVersion,
            request.expectedPackageVersion,
            request.stage === "image",
            imageProvider?.id ?? null,
            imageProvider?.revision ?? null,
          ),
        onError,
      );
      setFeedback(
        t("workbench.stage_feedback", {
          stage: t(`workbench.grid.${request.stage}`),
          title: request.title,
        }),
      );
      return true;
    } catch {
      return false;
    } finally {
      setStageActionTarget(null);
    }
  };

  /** 文本与本地格式直接单步；配图先进入绑定当前服务修订的付费确认。 */
  const handleStageGenerationRequest = (
    taskId: string,
    stage: WorkflowStage,
    expectedRowVersion: number,
    expectedPackageVersion: number,
  ): void => {
    if (actionLock.current) return;
    const current = articles.find((view) => view.article.id === taskId);
    if (!current) return;
    const request: StageGenerationRequest = {
      expectedPackageVersion,
      expectedRowVersion,
      stage,
      taskId,
      title: current.article.title,
    };
    if (stage === "image") {
      const provider = stageProviders.image;
      if (!provider) {
        setStageActionError({
          message: t("errors.PROVIDER_NOT_CONFIGURED"),
          stage,
          taskId,
        });
        return;
      }
      setActionError(null);
      setStageActionError(null);
      setImageStageConfirmation({
        attempted: false,
        error: null,
        previousOutcomeUncertain: hasUncertainImageOutcome(current),
        provider,
        request,
      });
      return;
    }
    void runSingleStage(request, null, (message) =>
      setStageActionError({ message, stage, taskId }),
    );
  };

  /** 一次图片确认最多调用一次；失败后必须关闭并重新取得确认。 */
  const handleConfirmedImageGeneration = async (): Promise<void> => {
    const confirmation = imageStageConfirmation;
    if (!confirmation || confirmation.attempted) return;
    setImageStageConfirmation({
      ...confirmation,
      attempted: true,
      error: null,
    });
    const succeeded = await runSingleStage(
      confirmation.request,
      confirmation.provider,
      (message) =>
        setImageStageConfirmation((current) =>
          current ? { ...current, attempted: true, error: message } : current,
        ),
    );
    if (succeeded) setImageStageConfirmation(null);
  };

  const detailAction = async (
    action: () => Promise<unknown>,
  ): Promise<void> => {
    await performAction(action);
  };

  /** 让配图编辑器保留后端稳定错误码，以便就近展示精确原因。 */
  const imageEditorAction = async (
    action: () => Promise<unknown>,
  ): Promise<void> => {
    await performAction(action, () => undefined, true);
  };

  const statusOptions = [
    "all",
    "draft",
    "queued",
    "running",
    "pendingReview",
    "approved",
    "partialFailure",
    "cancelled",
    "interrupted",
  ].map((value) => ({
    label:
      value === "all"
        ? t("workbench.filters.all_statuses")
        : t(`status.overall.${value}`),
    value,
  }));

  return (
    <Stack className={classes.page} gap="lg">
      <Group align="flex-start" justify="space-between" wrap="wrap">
        <Stack gap={4}>
          <Title order={1}>{t("workbench.title")}</Title>
          <Text c="dimmed">{t("workbench.description")}</Text>
        </Stack>
        <Group>
          <Button
            leftSection={<IconPlus aria-hidden="true" size={18} />}
            onClick={() => setAddOpened(true)}
            variant="default"
          >
            {t("workbench.actions.add_titles")}
          </Button>
          <Button
            disabled={
              selectedIds.size === 0 || Boolean(activeRun) || actionBusy
            }
            leftSection={<IconPlayerPlay aria-hidden="true" size={18} />}
            loading={actionBusy && stageActionTarget === null && !batchPreview}
            onClick={() => void handlePreview()}
          >
            {t("workbench.actions.run_selected", {
              count: selectedIds.size,
            })}
          </Button>
        </Group>
      </Group>

      {fixtureEnabled ? (
        <Alert
          color="violet"
          icon={<IconSettingsAutomation aria-hidden="true" size={20} />}
        >
          {t("workbench.fixture_notice")}
        </Alert>
      ) : !providersQuery.isPending && !hasStageProviders ? (
        <Alert
          color="yellow"
          icon={<IconAlertCircle aria-hidden="true" size={20} />}
        >
          <Group justify="space-between" wrap="wrap">
            <Text>
              {t("workbench.provider_missing", {
                stages: (["article", "summary", "image"] as const)
                  .filter((stage) => !stageProviders[stage])
                  .map((stage) => t(`workbench.grid.${stage}`))
                  .join(t("workbench.stage_separator")),
              })}
            </Text>
            <Button
              onClick={() => void navigate({ to: "/providers" })}
              variant="default"
            >
              {t("workbench.actions.configure_provider")}
            </Button>
          </Group>
        </Alert>
      ) : null}

      {activeRun ? (
        <BatchStatusStrip
          busy={actionBusy}
          onCancel={() => void performAction(cancelActiveBatch)}
          onPause={() => void performAction(pauseActiveBatch)}
          onResume={() => void performAction(resumeActiveBatch)}
          run={activeRun}
        />
      ) : null}

      {actionError ? (
        <Alert
          color="red"
          icon={<IconAlertCircle aria-hidden="true" size={20} />}
          role="alert"
        >
          {actionError}
        </Alert>
      ) : null}
      {feedback ? (
        <Alert color="green" role="status">
          {feedback}
        </Alert>
      ) : null}

      <SimpleGrid cols={{ base: 2, md: 4 }} spacing="sm">
        {(
          [
            ["total", counts.total],
            ["pending", counts.pending],
            ["approved", counts.approved],
            ["failed", counts.failed],
          ] as const
        ).map(([key, value]) => (
          <Paper key={key} p="md" radius="lg" withBorder>
            <Text c="dimmed" size="xs">
              {t(`workbench.metrics.${key}`)}
            </Text>
            <Text fw={700} size="xl">
              {value}
            </Text>
          </Paper>
        ))}
      </SimpleGrid>

      <Paper className={classes.gridSurface} radius="lg" withBorder>
        <Stack gap={0}>
          <Group align="flex-end" gap="sm" p="md" wrap="wrap">
            <TextInput
              className={classes.toolbarField}
              label={t("workbench.filters.search_label")}
              leftSection={<IconSearch aria-hidden="true" size={17} />}
              onChange={(event) => setSearch(event.currentTarget.value)}
              placeholder={t("workbench.filters.search_placeholder")}
              value={search}
            />
            <Select
              className={classes.toolbarField}
              data={statusOptions}
              label={t("workbench.filters.status_label")}
              onChange={(value) => {
                if (value) setStatusFilter(value as OverallStatus | "all");
              }}
              value={statusFilter}
            />
            <Select
              className={classes.toolbarField}
              data={[
                { label: t("workbench.sort.created"), value: "created" },
                { label: t("workbench.sort.updated"), value: "updated" },
                { label: t("workbench.sort.title"), value: "title" },
                { label: t("workbench.sort.status"), value: "status" },
              ]}
              label={t("workbench.sort.label")}
              onChange={(value) => {
                if (
                  value === "created" ||
                  value === "updated" ||
                  value === "title" ||
                  value === "status"
                ) {
                  setSort(value);
                }
              }}
              value={sort}
            />
            {backgroundRefreshing ? (
              <Group gap="xs" role="status">
                <Text c="dimmed" size="xs">
                  {t("workbench.refreshing")}
                </Text>
              </Group>
            ) : null}
          </Group>

          <Group justify="space-between" px="md" py="sm" wrap="wrap">
            <Checkbox
              checked={allVisibleSelected}
              indeterminate={selectionIndeterminate}
              label={t("workbench.actions.select_current", {
                count: visibleArticles.length,
              })}
              onChange={(event) => {
                const checked = event.currentTarget.checked;
                setSelectedIds((current) => {
                  const next = new Set(current);
                  for (const view of visibleArticles) {
                    if (checked) next.add(view.article.id);
                    else next.delete(view.article.id);
                  }
                  return next;
                });
              }}
            />
            <Group gap="xs">
              <Badge variant="light">
                {t("workbench.selected_count", { count: selectedIds.size })}
              </Badge>
              <Button
                disabled={
                  !hasRetryableSelection || Boolean(activeRun) || actionBusy
                }
                leftSection={<IconRefresh aria-hidden="true" size={17} />}
                onClick={() => void handleRetrySelected()}
                size="compact-sm"
                variant="default"
              >
                {t("workbench.actions.retry_selected")}
              </Button>
            </Group>
          </Group>

          {initialError ? (
            <Alert
              color="red"
              icon={<IconAlertCircle aria-hidden="true" size={20} />}
              m="md"
              role="alert"
            >
              <Stack align="flex-start" gap="sm">
                <Text>{t("workbench.load_failed")}</Text>
                <Button
                  leftSection={<IconRefresh aria-hidden="true" size={17} />}
                  onClick={() => void workbenchQuery.refetch()}
                  variant="default"
                >
                  {t("common.retry")}
                </Button>
              </Stack>
            </Alert>
          ) : (
            <>
              {backgroundError ? (
                <Alert
                  color="red"
                  icon={<IconAlertCircle aria-hidden="true" size={20} />}
                  m="md"
                  role="alert"
                >
                  <Group justify="space-between" wrap="wrap">
                    <Text>{t("workbench.refresh_failed")}</Text>
                    <Button
                      leftSection={<IconRefresh aria-hidden="true" size={17} />}
                      onClick={() => void workbenchQuery.refetch()}
                      variant="default"
                    >
                      {t("common.retry")}
                    </Button>
                  </Group>
                </Alert>
              ) : null}
              <ArticleGrid
                actionLocked={actionBusy || Boolean(activeRun)}
                articles={pageArticles}
                emptyState={
                  articles.length === 0
                    ? "workspace"
                    : visibleArticles.length === 0
                      ? "filtered"
                      : "page"
                }
                loading={initialLoading}
                onClearFilters={() => {
                  setSearch("");
                  setStatusFilter("all");
                }}
                onGenerateStage={handleStageGenerationRequest}
                onInspect={(taskId) => setInspectedId(taskId)}
                onInspectArtifact={(taskId, stage) => {
                  setDetailsTab(stage);
                  setInspectedId(taskId);
                }}
                onOpenAdd={() => setAddOpened(true)}
                onSelectionChange={(taskId, selected) =>
                  setSelectedIds((current) => {
                    const next = new Set(current);
                    if (selected) next.add(taskId);
                    else next.delete(taskId);
                    return next;
                  })
                }
                selectedIds={selectedIds}
                stageActionError={stageActionError}
                stageActionTarget={stageActionTarget}
                stageProviderState={stageProviderState}
              />
              <WorkbenchPagination resultCount={visibleArticles.length} />
            </>
          )}
        </Stack>
      </Paper>

      <AddTitlesModal
        onClose={() => setAddOpened(false)}
        onSubmit={handleAddTitles}
        opened={addOpened}
      />

      <Modal
        centered
        onClose={() => setBatchPreview(null)}
        opened={batchPreview !== null}
        title={t("workbench.preview.title")}
        transitionProps={{ duration: 0 }}
        withinPortal={false}
      >
        {batchPreview ? (
          <Stack gap="md">
            <Text>
              {t("workbench.preview.summary", {
                runnable: batchPreview.queuedCount,
                skipped: batchPreview.skippedCount,
              })}
            </Text>
            {batchPreview.skippedCount > 0 ? (
              <Alert color="yellow">
                {t("workbench.preview.skipped_hint")}
              </Alert>
            ) : null}
            <Text c="dimmed" size="sm">
              {t("workbench.preview.external_notice")}
            </Text>
            <Group justify="flex-end">
              <Button onClick={() => setBatchPreview(null)} variant="default">
                {t("common.cancel")}
              </Button>
              <Button
                disabled={batchPreview.queuedCount === 0 || actionBusy}
                loading={actionBusy}
                onClick={() => void handleStartConfirmed()}
              >
                {t("workbench.preview.confirm")}
              </Button>
            </Group>
          </Stack>
        ) : null}
      </Modal>

      <Modal
        centered
        closeOnClickOutside={!stageActionTarget}
        closeOnEscape={!stageActionTarget}
        onClose={() => {
          if (!stageActionTarget) setImageStageConfirmation(null);
        }}
        opened={imageStageConfirmation !== null}
        title={t("workbench.image_confirmation.title")}
        transitionProps={{ duration: 0 }}
        withCloseButton={!stageActionTarget}
        zIndex={300}
      >
        {imageStageConfirmation ? (
          <Stack gap="md">
            <Text>
              {t("workbench.image_confirmation.description", {
                title: imageStageConfirmation.request.title,
              })}
            </Text>
            <Alert
              color={
                imageStageConfirmation.provider.isTestFixture
                  ? "violet"
                  : "yellow"
              }
            >
              {t(
                imageStageConfirmation.provider.isTestFixture
                  ? "workbench.image_confirmation.fixture_notice"
                  : "workbench.image_confirmation.billing_notice",
              )}
            </Alert>
            {imageStageConfirmation.previousOutcomeUncertain ? (
              <Alert color="orange">
                {t("workbench.image_confirmation.uncertain_notice")}
              </Alert>
            ) : null}
            <Paper p="sm" radius="md" withBorder>
              <Stack gap={4}>
                <Text fw={600} size="sm">
                  {t("workbench.image_confirmation.service", {
                    name: imageStageConfirmation.provider.name,
                    revision: imageStageConfirmation.provider.revision,
                  })}
                </Text>
                <Text size="sm">
                  {t("workbench.image_confirmation.model", {
                    model: imageStageConfirmation.provider.imageModel,
                  })}
                </Text>
                <Text c="dimmed" size="sm">
                  {t("workbench.image_confirmation.parameters", {
                    size: imageStageConfirmation.provider.imageParameters.size,
                    watermark: t(
                      imageStageConfirmation.provider.imageParameters.watermark
                        ? "workbench.image_confirmation.watermark_on"
                        : "workbench.image_confirmation.watermark_off",
                    ),
                  })}
                </Text>
              </Stack>
            </Paper>
            {imageStageConfirmation.error ? (
              <Alert color="red" role="alert">
                {imageStageConfirmation.error}
              </Alert>
            ) : null}
            <Group justify="flex-end">
              <Button
                disabled={Boolean(stageActionTarget)}
                onClick={() => setImageStageConfirmation(null)}
                variant="default"
              >
                {imageStageConfirmation.attempted
                  ? t("workbench.image_confirmation.close_after_failure")
                  : t("common.cancel")}
              </Button>
              {!imageStageConfirmation.attempted ? (
                <Button
                  loading={
                    stageActionTarget?.stage === "image" &&
                    stageActionTarget.taskId ===
                      imageStageConfirmation.request.taskId
                  }
                  onClick={() => void handleConfirmedImageGeneration()}
                >
                  {t("workbench.image_confirmation.confirm")}
                </Button>
              ) : null}
            </Group>
          </Stack>
        ) : null}
      </Modal>

      <ArticleDetailsDrawer
        activeTab={detailsTab}
        actionsLocked={actionBusy || Boolean(activeRun)}
        imageEditorActionsLocked={actionBusy}
        article={inspectedArticle}
        onActiveTabChange={setDetailsTab}
        onApprove={(taskId) => detailAction(() => approveArticle(taskId))}
        onClose={() => setInspectedId(null)}
        onGenerateStage={handleStageGenerationRequest}
        onRegenerate={(taskId) => detailAction(() => regenerateArticle(taskId))}
        onRetry={(taskId) => detailAction(() => retryArticle(taskId))}
        onReturnToDraft={(taskId) =>
          detailAction(() => returnArticleToDraft(taskId))
        }
        onSave={(taskId, markdown, articleSummary) =>
          detailAction(() =>
            updateArticleContent(taskId, markdown, articleSummary),
          )
        }
        onSaveInputs={(taskId, keywords, requirements) =>
          detailAction(() =>
            updateArticleInputs(taskId, keywords, requirements),
          )
        }
        onSaveImage={(request) =>
          imageEditorAction(() =>
            saveArticleImageEdit(
              request.taskId,
              request.expectedRowVersion,
              request.expectedPackageVersion,
              request.expectedImageAttemptId,
              request.expectedEditRevision,
              request.spec,
            ),
          )
        }
        opened={inspectedId !== null}
        runHistory={workbenchQuery.data?.runHistory ?? []}
        stageActionError={stageActionError}
        stageActionTarget={stageActionTarget}
        stageProviderState={stageProviderState}
      />
    </Stack>
  );
}
