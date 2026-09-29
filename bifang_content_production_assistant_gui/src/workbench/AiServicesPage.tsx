import {
  Alert,
  Button,
  Divider,
  Group,
  Loader,
  Paper,
  SimpleGrid,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconDeviceFloppy,
  IconKey,
  IconRefresh,
} from "@tabler/icons-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useAtom } from "jotai";
import { useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { DeleteProviderModal } from "./DeleteProviderModal";
import { normalizedProviderEndpoint } from "./providerEndpoint";
import { ProviderConnectionFields } from "./ProviderConnectionFields";
import {
  type ProviderVerificationResult,
  type ProviderVerificationStage,
} from "./ProviderStageCard";
import { ProviderWorkflowPanel } from "./ProviderWorkflowPanel";
import { SavedProviderCard } from "./SavedProviderCard";
import { TestImageProviderModal } from "./TestImageProviderModal";
import {
  deleteProviderConfig,
  listProviderConfigs,
  newProviderConfigDraft,
  newVolcengineArkImageProviderConfigDraft,
  saveProviderConfig,
  setProviderEnabled,
  setProviderStageAssignment,
  testProviderImage,
  verifyProviderConnection,
} from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import { activeProviderStageAtom } from "./workbenchState";
import type {
  PromptTemplateKind,
  ProviderConfig,
  ProviderConfigView,
  ProviderImageTestResult,
  ProviderKind,
} from "./workbenchTypes";

/** 区分步骤参数、共享连接与新服务三种编辑上下文。 */
type ProviderEditorMode = "stage" | "connection" | "new";

/** 从不含密钥的后端视图建立完整可编辑草稿。 */
function providerDraft(provider: ProviderConfigView): ProviderConfig {
  return {
    articleModel: provider.articleModel,
    articleParameters: { ...provider.articleParameters },
    baseUrl: provider.baseUrl,
    defaultForArticle: provider.defaultForArticle,
    defaultForImage: provider.defaultForImage,
    defaultForSummary: provider.defaultForSummary,
    enabled: provider.enabled,
    id: provider.id,
    imageModel: provider.imageModel,
    imageParameters: { ...provider.imageParameters },
    kind: provider.kind,
    name: provider.name,
    revision: provider.revision,
    summaryModel: provider.summaryModel,
    summaryParameters: { ...provider.summaryParameters },
    textModel: provider.textModel,
  };
}

/** 新服务只允许当前可见步骤继承默认位，避免隐藏步骤被隐式绑定。 */
function newProviderForStage(
  template: ProviderConfig,
  stage: PromptTemplateKind,
): ProviderConfig {
  return {
    ...template,
    articleParameters: { ...template.articleParameters },
    defaultForArticle: stage === "article",
    defaultForImage: stage === "image",
    defaultForSummary: stage === "summary",
    imageParameters: { ...template.imageParameters },
    summaryParameters: { ...template.summaryParameters },
  };
}

/** 把当前步骤草稿合并到最新服务快照，隐藏步骤和共享连接逐字段保留。 */
function mergeStageDraft(
  latest: ProviderConfigView,
  edited: ProviderConfig,
  stage: PromptTemplateKind,
): ProviderConfig {
  const merged = providerDraft(latest);
  if (stage === "article") {
    merged.articleModel = edited.articleModel;
    merged.articleParameters = { ...edited.articleParameters };
    merged.defaultForArticle = edited.defaultForArticle;
  } else if (stage === "summary") {
    merged.summaryModel = edited.summaryModel;
    merged.summaryParameters = { ...edited.summaryParameters };
    merged.defaultForSummary = edited.defaultForSummary;
  } else {
    merged.imageModel = edited.imageModel;
    merged.imageParameters = { ...edited.imageParameters };
    merged.defaultForImage = edited.defaultForImage;
  }
  return merged;
}

/** 连接编辑只改变名称和端点，三个步骤始终取最新已保存值。 */
function mergeConnectionDraft(
  latest: ProviderConfigView,
  edited: ProviderConfig,
): ProviderConfig {
  return {
    ...providerDraft(latest),
    baseUrl: edited.baseUrl,
    name: edited.name,
  };
}

/** 返回某步骤的模型与默认位，供展示和提交门禁共用。 */
function stageValue(
  provider: ProviderConfig,
  stage: PromptTemplateKind,
): { isDefault: boolean; model: string } {
  if (stage === "article") {
    return {
      isDefault: provider.defaultForArticle,
      model: provider.articleModel,
    };
  }
  if (stage === "summary") {
    return {
      isDefault: provider.defaultForSummary,
      model: provider.summaryModel,
    };
  }
  return { isDefault: provider.defaultForImage, model: provider.imageModel };
}

/** 只改变草稿在当前步骤的显式分配位。 */
function withStageAssignment(
  provider: ProviderConfig,
  stage: PromptTemplateKind,
  assigned: boolean,
): ProviderConfig {
  if (stage === "article") {
    return { ...provider, defaultForArticle: assigned };
  }
  if (stage === "summary") {
    return { ...provider, defaultForSummary: assigned };
  }
  return { ...provider, defaultForImage: assigned };
}

/** 连接能力只决定它能否出现在当前步骤的显式选择菜单中。 */
function supportsStage(
  provider: ProviderConfigView,
  stage: PromptTemplateKind,
): boolean {
  return stage === "image" || provider.kind !== "volcengineArkImage";
}

/** 管理按 SOP 步骤拆分的模型配置与共享 OpenAI-compatible 服务连接。 */
export function AiServicesPage(): ReactElement {
  const { t } = useTranslation("app");
  const queryClient = useQueryClient();
  const [activeStage, setActiveStage] = useAtom(activeProviderStageAtom);
  const providersQuery = useQuery({
    queryFn: listProviderConfigs,
    queryKey: ["provider-configs"],
  });
  const providerDraftQuery = useQuery({
    queryFn: newProviderConfigDraft,
    queryKey: ["provider-config-draft"],
    staleTime: Number.POSITIVE_INFINITY,
  });
  const arkImageProviderDraftQuery = useQuery({
    queryFn: newVolcengineArkImageProviderConfigDraft,
    queryKey: ["volcengine-ark-image-provider-config-draft"],
    staleTime: Number.POSITIVE_INFINITY,
  });
  const [editorMode, setEditorMode] = useState<ProviderEditorMode | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editingProvider, setEditingProvider] =
    useState<ProviderConfigView | null>(null);
  const [draft, setDraft] = useState<ProviderConfig | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [mutatingProviderId, setMutatingProviderId] = useState<string | null>(
    null,
  );
  const [deleteCandidate, setDeleteCandidate] =
    useState<ProviderConfigView | null>(null);
  const [imageTestCandidate, setImageTestCandidate] =
    useState<ProviderConfigView | null>(null);
  const [imageTestAttempted, setImageTestAttempted] = useState(false);
  const [imageTestResult, setImageTestResult] =
    useState<ProviderImageTestResult | null>(null);
  const imageTestInFlightRef = useRef(false);
  const [verifying, setVerifying] = useState<{
    providerId: string;
    stage: ProviderVerificationStage;
  } | null>(null);
  const [verificationResult, setVerificationResult] =
    useState<ProviderVerificationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);

  const clearEditor = (): void => {
    setEditorMode(null);
    setSelectedId(null);
    setEditingProvider(null);
    setDraft(null);
    setApiKey("");
    setError(null);
    setFeedback(null);
  };

  const updateDraft = (next: ProviderConfig): void => {
    setDraft(next);
    setError(null);
    setFeedback(null);
  };

  const beginStageEdit = (provider: ProviderConfigView): void => {
    setEditorMode("stage");
    setSelectedId(provider.id);
    setEditingProvider(provider);
    setDraft(providerDraft(provider));
    setApiKey("");
    setError(null);
    setFeedback(null);
  };

  const beginStageAssignmentEdit = (provider: ProviderConfigView): void => {
    setEditorMode("stage");
    setSelectedId(provider.id);
    setEditingProvider(provider);
    setDraft(withStageAssignment(providerDraft(provider), activeStage, true));
    setApiKey("");
    setError(null);
    setFeedback(null);
  };

  const beginConnectionEdit = (provider: ProviderConfigView): void => {
    setEditorMode("connection");
    setSelectedId(provider.id);
    setEditingProvider(provider);
    setDraft(providerDraft(provider));
    setApiKey("");
    setError(null);
    setFeedback(null);
  };

  const beginAdd = (template: ProviderConfig): void => {
    setEditorMode("new");
    setSelectedId(null);
    setEditingProvider(null);
    setDraft(newProviderForStage(template, activeStage));
    setApiKey("");
    setError(null);
    setFeedback(null);
  };

  /** 新建图片服务切换协议时重置协议字段，但保留用户当前默认选择。 */
  const switchNewImageProtocol = (
    kind: Extract<ProviderKind, "openAiCompatible" | "volcengineArkImage">,
  ): void => {
    if (!draft || editorMode !== "new" || activeStage !== "image") return;
    const template =
      kind === "volcengineArkImage"
        ? arkImageProviderDraftQuery.data
        : providerDraftQuery.data;
    if (!template) return;
    const next = newProviderForStage(template, "image");
    next.defaultForImage = draft.defaultForImage;
    setDraft(next);
    setApiKey("");
    setError(null);
    setFeedback(null);
  };

  const providers = providersQuery.data ?? [];
  const latestEditingProvider =
    editingProvider === null
      ? null
      : (providers.find((provider) => provider.id === editingProvider.id) ??
        editingProvider);

  const handleSave = async (): Promise<void> => {
    if (!draft || !editorMode) return;
    let submission = draft;
    if (editorMode !== "new") {
      const latest = providers.find((provider) => provider.id === draft.id);
      if (!latest) {
        setError(t("providers.editor_stale"));
        return;
      }
      submission =
        editorMode === "stage"
          ? mergeStageDraft(latest, draft, activeStage)
          : mergeConnectionDraft(latest, draft);
    }
    const articleModel = submission.articleModel.trim();
    const summaryModel = submission.summaryModel.trim();
    setSaving(true);
    setError(null);
    setFeedback(null);
    try {
      const { providers: savedProviders, saved } = await saveProviderConfig(
        {
          ...submission,
          articleModel,
          baseUrl: submission.baseUrl.trim(),
          imageModel: submission.imageModel.trim(),
          name: submission.name.trim(),
          summaryModel,
          textModel: articleModel || summaryModel,
        },
        editorMode === "stage" ? null : apiKey || null,
      );
      setSelectedId(null);
      setEditingProvider(null);
      setDraft(null);
      setEditorMode(null);
      setVerificationResult((current) =>
        current?.providerId === saved.id ? null : current,
      );
      setApiKey("");
      queryClient.setQueryData<ProviderConfigView[]>(
        ["provider-configs"],
        savedProviders,
      );
      setFeedback(
        t(
          editorMode === "connection"
            ? "providers.feedback.connection_saved"
            : "providers.feedback.stage_saved",
          {
            name: saved.name,
            stage: t(`providers.stage_name.${activeStage}`),
          },
        ),
      );
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setSaving(false);
    }
  };

  const handleVerify = async (
    provider: ProviderConfigView,
    stage: "article" | "summary",
  ): Promise<void> => {
    setVerifying({ providerId: provider.id, stage });
    setVerificationResult(null);
    try {
      const result = await verifyProviderConnection(provider.id, stage);
      const stageLabel = t(`providers.stage_name.${stage}`);
      setVerificationResult({
        error: result.ok ? null : result.message,
        feedback: result.ok
          ? t(
              provider.isTestFixture
                ? "providers.feedback.fixture_connection_ok"
                : "providers.feedback.connection_ok",
              { name: provider.name, stage: stageLabel },
            )
          : null,
        providerId: provider.id,
        providerRevision: provider.revision,
        stage,
      });
    } catch (reason) {
      setVerificationResult({
        error: localizedCommandError(reason, t),
        feedback: null,
        providerId: provider.id,
        providerRevision: provider.revision,
        stage,
      });
    } finally {
      setVerifying(null);
    }
  };

  /** 发出一次已经确认的真实图片请求，并只把预览保存在当前页面内存。 */
  const handleImageTest = async (
    provider: ProviderConfigView,
  ): Promise<void> => {
    if (imageTestInFlightRef.current) {
      return;
    }
    imageTestInFlightRef.current = true;
    setImageTestAttempted(true);
    setVerifying({ providerId: provider.id, stage: "image" });
    setVerificationResult(null);
    setImageTestResult(null);
    try {
      const result = await testProviderImage(provider.id, provider.revision);
      setImageTestResult(result);
      setVerificationResult({
        error: null,
        feedback: t("providers.feedback.image_connection_ok", {
          name: provider.name,
        }),
        providerId: provider.id,
        providerRevision: provider.revision,
        stage: "image",
      });
    } catch (reason) {
      setVerificationResult({
        error: localizedCommandError(reason, t),
        feedback: null,
        providerId: provider.id,
        providerRevision: provider.revision,
        stage: "image",
      });
    } finally {
      imageTestInFlightRef.current = false;
      setVerifying(null);
    }
  };

  /** 文本直接验证；图片先打开可能计费的明确确认弹窗。 */
  const requestVerification = (
    provider: ProviderConfigView,
    stage: ProviderVerificationStage,
  ): void => {
    setVerificationResult(null);
    if (stage === "image") {
      setImageTestCandidate(provider);
      setImageTestAttempted(false);
      setImageTestResult(null);
      return;
    }
    void handleVerify(provider, stage);
  };

  const handleEnabledChange = async (
    provider: ProviderConfigView,
    enabled: boolean,
  ): Promise<void> => {
    setMutatingProviderId(provider.id);
    setError(null);
    setFeedback(null);
    try {
      const { providers: currentProviders, saved } = await setProviderEnabled(
        provider.id,
        provider.revision,
        enabled,
      );
      queryClient.setQueryData<ProviderConfigView[]>(
        ["provider-configs"],
        currentProviders,
      );
      setEditingProvider((current) =>
        current?.id === saved.id ? saved : current,
      );
      setDraft((current) =>
        current?.id === saved.id
          ? {
              ...current,
              defaultForArticle: saved.defaultForArticle,
              defaultForImage: saved.defaultForImage,
              defaultForSummary: saved.defaultForSummary,
              enabled: saved.enabled,
              revision: saved.revision,
            }
          : current,
      );
      setVerificationResult((current) =>
        current?.providerId === saved.id ? null : current,
      );
      setFeedback(
        t(
          enabled
            ? "providers.feedback.enabled"
            : "providers.feedback.disabled",
          { name: saved.name },
        ),
      );
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setMutatingProviderId(null);
    }
  };

  const handleStageAssignment = async (
    provider: ProviderConfigView,
    assigned: boolean,
  ): Promise<void> => {
    const stage = activeStage;
    setMutatingProviderId(provider.id);
    setError(null);
    setFeedback(null);
    try {
      const { providers: currentProviders, saved } =
        await setProviderStageAssignment(
          provider.id,
          provider.revision,
          stage,
          assigned,
        );
      queryClient.setQueryData<ProviderConfigView[]>(
        ["provider-configs"],
        currentProviders,
      );
      setVerificationResult((current) =>
        current?.providerId === saved.id ? null : current,
      );
      setFeedback(
        t(
          assigned
            ? "providers.feedback.stage_assigned"
            : "providers.feedback.stage_removed",
          {
            name: saved.name,
            stage: t(`providers.stage_name.${stage}`),
          },
        ),
      );
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setMutatingProviderId(null);
    }
  };

  const handleDelete = async (): Promise<void> => {
    if (!deleteCandidate) return;
    setMutatingProviderId(deleteCandidate.id);
    setError(null);
    setFeedback(null);
    try {
      const result = await deleteProviderConfig(
        deleteCandidate.id,
        deleteCandidate.revision,
      );
      queryClient.setQueryData<ProviderConfigView[]>(
        ["provider-configs"],
        result.providers,
      );
      if (selectedId === deleteCandidate.id) clearEditor();
      setVerificationResult((current) =>
        current?.providerId === deleteCandidate.id ? null : current,
      );
      setFeedback(
        t(
          result.credentialCleanupPending
            ? "providers.feedback.deleted_cleanup_pending"
            : result.historicalBindingsDetached > 0
              ? "providers.feedback.deleted_history_detached"
              : "providers.feedback.deleted",
          {
            count: result.historicalBindingsDetached,
            name: deleteCandidate.name,
          },
        ),
      );
      setDeleteCandidate(null);
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setMutatingProviderId(null);
    }
  };

  if (
    providersQuery.isPending ||
    providerDraftQuery.isPending ||
    arkImageProviderDraftQuery.isPending
  ) {
    return (
      <Stack align="center" gap="sm" mih={280} justify="center" role="status">
        <Loader aria-label={t("providers.loading")} size="sm" />
        <Text c="dimmed">{t("providers.loading")}</Text>
      </Stack>
    );
  }

  if (
    (providersQuery.isError && !providersQuery.data) ||
    (providerDraftQuery.isError && !providerDraftQuery.data) ||
    (arkImageProviderDraftQuery.isError && !arkImageProviderDraftQuery.data) ||
    !providerDraftQuery.data ||
    !arkImageProviderDraftQuery.data
  ) {
    return (
      <Alert
        color="red"
        icon={<IconAlertCircle aria-hidden="true" size={20} />}
        role="alert"
      >
        <Stack align="flex-start" gap="sm">
          <Text>{t("providers.load_failed")}</Text>
          <Button
            leftSection={<IconRefresh aria-hidden="true" size={17} />}
            onClick={() => {
              void providersQuery.refetch();
              void providerDraftQuery.refetch();
              void arkImageProviderDraftQuery.refetch();
            }}
            variant="default"
          >
            {t("common.retry")}
          </Button>
        </Stack>
      </Alert>
    );
  }

  const newProviderDraft = providerDraftQuery.data;
  const newArkImageProviderDraft = arkImageProviderDraftQuery.data;
  const activeStageLabel = t(`providers.stage_name.${activeStage}`);
  const currentDefault = providers.find((provider) => {
    if (activeStage === "article") return provider.defaultForArticle;
    if (activeStage === "summary") return provider.defaultForSummary;
    return provider.defaultForImage;
  });
  const availableStageProviders = providers.filter(
    (provider) =>
      provider.enabled &&
      !provider.isTestFixture &&
      provider.id !== currentDefault?.id &&
      supportsStage(provider, activeStage),
  );
  const activeDraftValue = draft ? stageValue(draft, activeStage) : null;
  const connectionEditor = editorMode === "connection" || editorMode === "new";
  const baseUrlInvalid =
    connectionEditor &&
    draft !== null &&
    draft.baseUrl.trim().length > 0 &&
    normalizedProviderEndpoint(draft.baseUrl.trim(), draft.kind) === null;
  const endpointChanged =
    editorMode === "connection" &&
    draft !== null &&
    latestEditingProvider !== null &&
    normalizedProviderEndpoint(draft.baseUrl.trim(), draft.kind) !==
      normalizedProviderEndpoint(
        latestEditingProvider.baseUrl.trim(),
        latestEditingProvider.kind,
      );
  const actualImageEndpoint =
    draft?.kind === "volcengineArkImage"
      ? normalizedProviderEndpoint(draft.baseUrl.trim(), draft.kind)
      : null;
  const editingProviderHasApiKey = latestEditingProvider?.hasApiKey ?? false;
  const keyRotationRequired = endpointChanged && editingProviderHasApiKey;
  const busy = saving || verifying !== null || mutatingProviderId !== null;
  const interactionLocked = busy || editorMode !== null;
  const imageTestError =
    imageTestCandidate !== null &&
    verificationResult?.providerId === imageTestCandidate.id &&
    verificationResult.providerRevision === imageTestCandidate.revision &&
    verificationResult.stage === "image"
      ? verificationResult.error
      : null;
  const hasAnyConfiguredStage =
    draft !== null &&
    (draft.articleModel.trim().length > 0 ||
      draft.summaryModel.trim().length > 0 ||
      draft.imageModel.trim().length > 0);
  const saveDisabled =
    draft === null ||
    editorMode === null ||
    verifying !== null ||
    draft.name.trim().length === 0 ||
    draft.baseUrl.trim().length === 0 ||
    baseUrlInvalid ||
    (keyRotationRequired && apiKey.trim().length === 0) ||
    !hasAnyConfiguredStage ||
    (editorMode === "new" && activeDraftValue?.model.trim().length === 0) ||
    (editorMode === "stage" &&
      activeDraftValue?.isDefault === true &&
      activeDraftValue.model.trim().length === 0) ||
    (!draft.enabled &&
      (draft.defaultForArticle ||
        draft.defaultForSummary ||
        draft.defaultForImage));

  return (
    <Stack gap="xl">
      <Stack gap={4}>
        <Title order={1}>{t("providers.title")}</Title>
        <Text c="dimmed">{t("providers.description")}</Text>
      </Stack>

      {(providersQuery.isFetching && !providersQuery.isPending) ||
      (providerDraftQuery.isFetching && !providerDraftQuery.isPending) ||
      (arkImageProviderDraftQuery.isFetching &&
        !arkImageProviderDraftQuery.isPending) ? (
        <Alert
          color="blue"
          icon={<Loader aria-hidden="true" size="xs" />}
          role="status"
        >
          {t("providers.refreshing")}
        </Alert>
      ) : null}
      {providersQuery.isRefetchError ||
      providerDraftQuery.isRefetchError ||
      arkImageProviderDraftQuery.isRefetchError ? (
        <Alert
          color="yellow"
          icon={<IconAlertCircle aria-hidden="true" size={20} />}
          role="alert"
        >
          <Group justify="space-between" wrap="wrap">
            <Text size="sm">{t("providers.refresh_failed")}</Text>
            <Button
              onClick={() => {
                void providersQuery.refetch();
                void providerDraftQuery.refetch();
                void arkImageProviderDraftQuery.refetch();
              }}
              size="xs"
              variant="default"
            >
              {t("common.retry")}
            </Button>
          </Group>
        </Alert>
      ) : null}
      {error && !deleteCandidate ? (
        <Alert color="red" role="alert">
          {error}
        </Alert>
      ) : null}
      {feedback ? (
        <Alert color="green" role="status">
          {feedback}
        </Alert>
      ) : null}

      <ProviderWorkflowPanel
        activeStage={activeStage}
        activeStageLabel={activeStageLabel}
        actualImageEndpoint={actualImageEndpoint}
        apiKey={apiKey}
        availableStageProviders={availableStageProviders}
        baseUrlInvalid={baseUrlInvalid}
        beginAdd={beginAdd}
        beginStageAssignmentEdit={beginStageAssignmentEdit}
        beginStageEdit={beginStageEdit}
        clearEditor={clearEditor}
        currentDefault={currentDefault}
        draft={draft}
        editorMode={editorMode}
        handleSave={handleSave}
        handleStageAssignment={handleStageAssignment}
        interactionLocked={interactionLocked}
        newArkImageProviderDraft={newArkImageProviderDraft}
        newProviderDraft={newProviderDraft}
        requestVerification={requestVerification}
        saveDisabled={saveDisabled}
        saving={saving}
        selectedId={selectedId}
        setActiveStage={setActiveStage}
        setApiKey={setApiKey}
        setError={setError}
        setFeedback={setFeedback}
        setVerificationResult={setVerificationResult}
        stageValue={stageValue}
        switchNewImageProtocol={switchNewImageProtocol}
        updateDraft={updateDraft}
        verificationResult={verificationResult}
        verifying={verifying}
      />

      <Divider />

      <Stack gap="md">
        <Stack gap={4}>
          <Title order={2} size="h3">
            {t("providers.connection_management_title")}
          </Title>
          <Text c="dimmed" size="sm">
            {t("providers.connection_management_description")}
          </Text>
        </Stack>
        <Alert color="yellow" icon={<IconKey aria-hidden="true" size={18} />}>
          {t("providers.connection_scope_notice")}
        </Alert>
        {providers.length > 0 ? (
          <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="md">
            {providers.map((provider) => (
              <SavedProviderCard
                busy={interactionLocked}
                key={provider.id}
                onDelete={(selected) => {
                  setError(null);
                  setFeedback(null);
                  setDeleteCandidate(selected);
                }}
                onEdit={beginConnectionEdit}
                onEnabledChange={(selected, enabled) =>
                  void handleEnabledChange(selected, enabled)
                }
                provider={provider}
              />
            ))}
          </SimpleGrid>
        ) : null}

        {editorMode === "connection" && draft ? (
          <Paper p="lg" radius="lg" withBorder>
            <Stack gap="md">
              <Stack gap={4}>
                <Title order={3} size="h4">
                  {t("providers.connection_editor_title", {
                    name: draft.name,
                  })}
                </Title>
                <Text c="dimmed" size="sm">
                  {t("providers.connection_editor_notice")}
                </Text>
              </Stack>
              <ProviderConnectionFields
                actualImageEndpoint={actualImageEndpoint}
                apiKey={apiKey}
                apiKeyDescription={
                  keyRotationRequired
                    ? t("providers.form.key_endpoint_changed_hint")
                    : editingProviderHasApiKey
                      ? t("providers.form.key_keep_hint")
                      : t("providers.form.key_store_hint")
                }
                apiKeyRequired={keyRotationRequired}
                baseUrlError={
                  baseUrlInvalid
                    ? t(
                        draft.kind === "volcengineArkImage"
                          ? "providers.form.ark_base_url_invalid"
                          : "providers.form.base_url_invalid",
                      )
                    : undefined
                }
                disabled={saving}
                draft={draft}
                onApiKeyChange={(value) => {
                  setApiKey(value);
                  setError(null);
                  setFeedback(null);
                }}
                onChange={updateDraft}
              />
              <Group justify="flex-end">
                <Button
                  disabled={saving}
                  onClick={clearEditor}
                  variant="default"
                >
                  {t("common.cancel")}
                </Button>
                <Button
                  disabled={saveDisabled}
                  leftSection={
                    <IconDeviceFloppy aria-hidden="true" size={17} />
                  }
                  loading={saving}
                  onClick={() => void handleSave()}
                >
                  {t("providers.save_connection")}
                </Button>
              </Group>
            </Stack>
          </Paper>
        ) : null}
      </Stack>

      <DeleteProviderModal
        busy={mutatingProviderId === deleteCandidate?.id}
        error={deleteCandidate ? error : null}
        onCancel={() => {
          if (mutatingProviderId === null) {
            setDeleteCandidate(null);
            setError(null);
          }
        }}
        onConfirm={() => void handleDelete()}
        provider={deleteCandidate}
      />
      <TestImageProviderModal
        attempted={imageTestAttempted}
        busy={
          imageTestCandidate !== null &&
          verifying !== null &&
          verifying.providerId === imageTestCandidate.id &&
          verifying.stage === "image"
        }
        error={imageTestError}
        onCancel={() => {
          if (!imageTestInFlightRef.current && verifying === null) {
            setImageTestCandidate(null);
            setImageTestAttempted(false);
            setImageTestResult(null);
          }
        }}
        onConfirm={() => {
          if (imageTestCandidate && !imageTestAttempted) {
            void handleImageTest(imageTestCandidate);
          }
        }}
        provider={imageTestCandidate}
        result={imageTestResult}
      />
    </Stack>
  );
}
