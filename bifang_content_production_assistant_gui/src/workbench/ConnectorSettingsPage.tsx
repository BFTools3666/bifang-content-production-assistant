import {
  Alert,
  Badge,
  Button,
  Group,
  Loader,
  Modal,
  NumberInput,
  Paper,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  TextInput,
  Title,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconCloudUpload,
  IconDeviceFloppy,
  IconEdit,
  IconPlugConnected,
  IconPlus,
  IconRefresh,
  IconTrash,
} from "@tabler/icons-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import {
  deleteConnectorConfig,
  listConnectorConfigs,
  newFeishuConnectorConfigDraft,
  saveConnectorConfig,
  setConnectorEnabled,
  verifyConnectorConnection,
} from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import type {
  ConnectorConfig,
  ConnectorConfigView,
  ConnectorDocumentFormat,
} from "./workbenchTypes";
import { isSafeLarkCliProfile, LarkCliSetupGuide } from "./LarkCliSetupGuide";

/** 从非秘密视图建立可编辑发布目标；CLI token 从不进入 WebView。 */
function editableConnector(connector: ConnectorConfigView): ConnectorConfig {
  return {
    cliProfile: connector.cliProfile,
    defaultFormat: connector.defaultFormat,
    enabled: connector.enabled,
    folderToken: connector.folderToken,
    id: connector.id,
    isDefault: connector.isDefault,
    kind: connector.kind,
    name: connector.name,
    revision: connector.revision,
    timeoutMs: connector.timeoutMs,
  };
}

/** 管理可扩展的云文档连接器；首版由官方 CLI 发布 Markdown/HTML。 */
export function ConnectorSettingsPage(): ReactElement {
  const { t } = useTranslation("app");
  const queryClient = useQueryClient();
  const connectorsQuery = useQuery({
    queryFn: listConnectorConfigs,
    queryKey: ["connector-configs"],
  });
  const draftQuery = useQuery({
    queryFn: newFeishuConnectorConfigDraft,
    queryKey: ["feishu-connector-config-draft"],
    staleTime: Number.POSITIVE_INFINITY,
  });
  const [draft, setDraft] = useState<ConnectorConfig | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [deleteCandidate, setDeleteCandidate] =
    useState<ConnectorConfigView | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const connectors = connectorsQuery.data ?? [];
  const clearEditor = (): void => {
    setDraft(null);
    setEditingId(null);
  };
  const beginAdd = (): void => {
    if (!draftQuery.data) return;
    setDraft({ ...draftQuery.data });
    setEditingId(null);
    setError(null);
    setFeedback(null);
  };
  const beginEdit = (connector: ConnectorConfigView): void => {
    setDraft(editableConnector(connector));
    setEditingId(connector.id);
    setError(null);
    setFeedback(null);
  };
  const beginAddFromGuide = (cliProfile: string, folderToken: string): void => {
    if (!draftQuery.data) return;
    const migrated = connectors.find(
      (connector) => connector.cliProfile.trim().length === 0,
    );
    setDraft({
      ...(migrated ? editableConnector(migrated) : draftQuery.data),
      cliProfile,
      enabled: true,
      folderToken,
    });
    setEditingId(migrated?.id ?? null);
    setError(null);
    setFeedback(t("connectors.guide.form_prefilled"));
  };

  /** 保存时始终合并最新修订，避免陈旧编辑器覆盖刚完成的启停操作。 */
  const handleSave = async (): Promise<void> => {
    if (!draft) return;
    const latest = editingId
      ? connectors.find((connector) => connector.id === editingId)
      : null;
    if (editingId && !latest) {
      setError(t("connectors.editor_stale"));
      return;
    }
    const submission: ConnectorConfig = {
      ...(latest ? editableConnector(latest) : draft),
      cliProfile: draft.cliProfile,
      defaultFormat: draft.defaultFormat,
      enabled: draft.enabled,
      folderToken: draft.folderToken.trim(),
      isDefault: draft.isDefault,
      name: draft.name.trim(),
      timeoutMs: draft.timeoutMs,
    };
    setPendingId(editingId ?? "new");
    setError(null);
    setFeedback(null);
    try {
      const result = await saveConnectorConfig(submission);
      queryClient.setQueryData<ConnectorConfigView[]>(
        ["connector-configs"],
        result.connectors,
      );
      clearEditor();
      setFeedback(t("connectors.feedback.saved", { name: result.saved.name }));
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setPendingId(null);
    }
  };

  const handleEnabledChange = async (
    connector: ConnectorConfigView,
    enabled: boolean,
  ): Promise<void> => {
    setPendingId(connector.id);
    setError(null);
    setFeedback(null);
    try {
      const result = await setConnectorEnabled(
        connector.id,
        connector.revision,
        enabled,
      );
      queryClient.setQueryData<ConnectorConfigView[]>(
        ["connector-configs"],
        result.connectors,
      );
      setFeedback(
        t(
          enabled
            ? "connectors.feedback.enabled"
            : "connectors.feedback.disabled",
          { name: result.saved.name },
        ),
      );
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setPendingId(null);
    }
  };

  /** 连接测试只列取目标文件夹，成功也不会创建任何远端文件。 */
  const handleVerify = async (
    connector: ConnectorConfigView,
  ): Promise<void> => {
    setPendingId(connector.id);
    setError(null);
    setFeedback(null);
    try {
      const result = await verifyConnectorConnection(connector.id);
      if (!result.ok) {
        setError(t("connectors.connection_failed"));
      } else {
        setFeedback(
          t("connectors.feedback.connection_ok", { name: connector.name }),
        );
      }
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setPendingId(null);
    }
  };

  const handleDelete = async (): Promise<void> => {
    if (!deleteCandidate) return;
    setPendingId(deleteCandidate.id);
    setError(null);
    setFeedback(null);
    try {
      const result = await deleteConnectorConfig(
        deleteCandidate.id,
        deleteCandidate.revision,
      );
      queryClient.setQueryData<ConnectorConfigView[]>(
        ["connector-configs"],
        result.connectors,
      );
      if (editingId === deleteCandidate.id) clearEditor();
      setFeedback(
        t(
          result.credentialCleanupPending
            ? "connectors.feedback.deleted_cleanup_pending"
            : "connectors.feedback.deleted",
          { name: deleteCandidate.name },
        ),
      );
      setDeleteCandidate(null);
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setPendingId(null);
    }
  };

  if (connectorsQuery.isPending || draftQuery.isPending) {
    return (
      <Stack align="center" gap="sm" mih={280} justify="center" role="status">
        <Loader aria-label={t("connectors.loading")} size="sm" />
        <Text c="dimmed">{t("connectors.loading")}</Text>
      </Stack>
    );
  }
  if (
    (connectorsQuery.isError && !connectorsQuery.data) ||
    (draftQuery.isError && !draftQuery.data) ||
    !draftQuery.data
  ) {
    return (
      <Alert
        color="red"
        icon={<IconAlertCircle aria-hidden="true" size={20} />}
        role="alert"
      >
        <Stack align="flex-start" gap="sm">
          <Text>{t("connectors.load_failed")}</Text>
          <Button
            leftSection={<IconRefresh aria-hidden="true" size={17} />}
            onClick={() => {
              void connectorsQuery.refetch();
              void draftQuery.refetch();
            }}
            variant="default"
          >
            {t("common.retry")}
          </Button>
        </Stack>
      </Alert>
    );
  }

  const saveDisabled =
    !draft ||
    pendingId !== null ||
    draft.name.trim().length === 0 ||
    !isSafeLarkCliProfile(draft.cliProfile) ||
    draft.folderToken.trim().length === 0 ||
    (!draft.enabled && draft.isDefault);

  return (
    <Stack gap="xl">
      <Group align="flex-start" justify="space-between" wrap="wrap">
        <Stack gap={4} maw={720}>
          <Title order={1}>{t("connectors.title")}</Title>
          <Text c="dimmed">{t("connectors.description")}</Text>
        </Stack>
        <Button
          disabled={draft !== null || pendingId !== null}
          leftSection={<IconPlus aria-hidden="true" size={18} />}
          onClick={beginAdd}
        >
          {t("connectors.add")}
        </Button>
      </Group>

      <LarkCliSetupGuide
        disabled={draft !== null || pendingId !== null}
        onUseConfiguration={beginAddFromGuide}
        openInitially={
          connectors.length === 0 ||
          connectors.some(
            (connector) => connector.cliProfile.trim().length === 0,
          )
        }
      />

      {error ? <Alert color="red">{error}</Alert> : null}
      {feedback ? (
        <Alert color="green" role="status">
          {feedback}
        </Alert>
      ) : null}

      {draft ? (
        <Paper p="lg" radius="md" withBorder>
          <Stack gap="md">
            <Group justify="space-between" wrap="wrap">
              <Title order={2} size="h3">
                {t(
                  editingId ? "connectors.edit_title" : "connectors.new_title",
                )}
              </Title>
              <Badge color="blue" variant="light">
                {t("connectors.feishu_drive")}
              </Badge>
            </Group>
            <SimpleGrid cols={{ base: 1, md: 2 }} spacing="md">
              <TextInput
                label={t("connectors.form.name")}
                onChange={(event) =>
                  setDraft({ ...draft, name: event.currentTarget.value })
                }
                required
                value={draft.name}
              />
              <TextInput
                description={t("connectors.form.cli_profile_hint")}
                error={
                  draft.cliProfile.length > 0 &&
                  !isSafeLarkCliProfile(draft.cliProfile)
                    ? t("errors.CONNECTOR_CLI_PROFILE_INVALID")
                    : undefined
                }
                label={t("connectors.form.cli_profile")}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    cliProfile: event.currentTarget.value,
                  })
                }
                required
                value={draft.cliProfile}
              />
              <TextInput
                description={t("connectors.form.folder_token_hint")}
                label={t("connectors.form.folder_token")}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    folderToken: event.currentTarget.value,
                  })
                }
                required
                value={draft.folderToken}
              />
              <Select
                allowDeselect={false}
                data={[
                  { label: "Markdown (.md)", value: "markdown" },
                  { label: "HTML (.html)", value: "html" },
                ]}
                label={t("connectors.form.default_format")}
                onChange={(value) => {
                  if (value === "markdown" || value === "html") {
                    setDraft({
                      ...draft,
                      defaultFormat: value as ConnectorDocumentFormat,
                    });
                  }
                }}
                value={draft.defaultFormat}
              />
              <NumberInput
                allowDecimal={false}
                clampBehavior="strict"
                label={t("connectors.form.timeout")}
                max={300}
                min={1}
                onChange={(value) => {
                  if (typeof value === "number") {
                    setDraft({ ...draft, timeoutMs: value * 1_000 });
                  }
                }}
                suffix={` ${t("connectors.form.seconds")}`}
                value={draft.timeoutMs / 1_000}
              />
            </SimpleGrid>
            <Group gap="xl" wrap="wrap">
              <Switch
                checked={draft.enabled}
                label={t("connectors.form.enabled")}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    enabled: event.currentTarget.checked,
                    isDefault: event.currentTarget.checked
                      ? draft.isDefault
                      : false,
                  })
                }
              />
              <Switch
                checked={draft.isDefault}
                disabled={!draft.enabled}
                label={t("connectors.form.default_connector")}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    isDefault: event.currentTarget.checked,
                  })
                }
              />
            </Group>
            <Alert color="blue" icon={<IconCloudUpload size={18} />}>
              {t("connectors.upload_scope")}
            </Alert>
            <Group justify="flex-end">
              <Button onClick={clearEditor} variant="default">
                {t("common.cancel")}
              </Button>
              <Button
                disabled={saveDisabled}
                leftSection={<IconDeviceFloppy aria-hidden="true" size={18} />}
                loading={pendingId === (editingId ?? "new")}
                onClick={() => void handleSave()}
              >
                {t("connectors.save")}
              </Button>
            </Group>
          </Stack>
        </Paper>
      ) : null}

      {connectors.length === 0 ? (
        <Alert color="gray">{t("connectors.empty")}</Alert>
      ) : (
        <SimpleGrid cols={{ base: 1, xl: 2 }} spacing="md">
          {connectors.map((connector) => (
            <Paper key={connector.id} p="lg" radius="md" withBorder>
              <Stack gap="sm">
                <Group align="flex-start" justify="space-between" wrap="wrap">
                  <Stack gap={2}>
                    <Text fw={700}>{connector.name}</Text>
                    <Text c="dimmed" size="xs">
                      {t("connectors.card.revision", {
                        revision: connector.revision,
                      })}
                    </Text>
                  </Stack>
                  <Group gap="xs">
                    {connector.isDefault ? (
                      <Badge color="blue">{t("connectors.card.default")}</Badge>
                    ) : null}
                    <Badge color={connector.enabled ? "green" : "gray"}>
                      {t(
                        connector.enabled
                          ? "connectors.card.enabled"
                          : "connectors.card.disabled",
                      )}
                    </Badge>
                  </Group>
                </Group>
                <Text size="sm">
                  {t("connectors.card.target", {
                    profile:
                      connector.cliProfile ||
                      t("connectors.card.profile_missing"),
                    folderToken: connector.folderToken,
                  })}
                </Text>
                <Switch
                  checked={connector.enabled}
                  disabled={
                    pendingId !== null ||
                    draft !== null ||
                    connector.cliProfile.trim().length === 0
                  }
                  label={t("connectors.card.enable_switch")}
                  onChange={(event) =>
                    void handleEnabledChange(
                      connector,
                      event.currentTarget.checked,
                    )
                  }
                />
                <Group gap="xs" wrap="wrap">
                  <Button
                    disabled={
                      pendingId !== null ||
                      draft !== null ||
                      !connector.enabled ||
                      connector.cliProfile.trim().length === 0
                    }
                    leftSection={
                      <IconPlugConnected aria-hidden="true" size={16} />
                    }
                    loading={pendingId === connector.id}
                    onClick={() => void handleVerify(connector)}
                    size="compact-sm"
                    variant="default"
                  >
                    {t("connectors.test")}
                  </Button>
                  <Button
                    disabled={pendingId !== null || draft !== null}
                    leftSection={<IconEdit aria-hidden="true" size={16} />}
                    onClick={() => beginEdit(connector)}
                    size="compact-sm"
                    variant="default"
                  >
                    {t("connectors.edit")}
                  </Button>
                  <Button
                    color="red"
                    disabled={pendingId !== null || draft !== null}
                    leftSection={<IconTrash aria-hidden="true" size={16} />}
                    onClick={() => setDeleteCandidate(connector)}
                    size="compact-sm"
                    variant="subtle"
                  >
                    {t("connectors.delete")}
                  </Button>
                </Group>
              </Stack>
            </Paper>
          ))}
        </SimpleGrid>
      )}

      <Modal
        centered
        onClose={() => setDeleteCandidate(null)}
        opened={deleteCandidate !== null}
        title={t("connectors.delete_title")}
      >
        <Stack gap="md">
          <Text>
            {t("connectors.delete_body", { name: deleteCandidate?.name ?? "" })}
          </Text>
          <Group justify="flex-end">
            <Button
              disabled={pendingId !== null}
              onClick={() => setDeleteCandidate(null)}
              variant="default"
            >
              {t("common.cancel")}
            </Button>
            <Button
              color="red"
              disabled={pendingId !== null}
              loading={pendingId === deleteCandidate?.id}
              onClick={() => void handleDelete()}
            >
              {t("connectors.confirm_delete")}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
