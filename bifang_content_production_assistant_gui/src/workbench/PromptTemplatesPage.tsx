import {
  Alert,
  Badge,
  Button,
  Code,
  Group,
  Loader,
  Paper,
  Stack,
  Tabs,
  Text,
  Textarea,
  Title,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconDeviceFloppy,
  IconEye,
  IconRefresh,
} from "@tabler/icons-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { TFunction } from "i18next";
import { useAtom } from "jotai";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import {
  listPromptTemplates,
  previewPromptTemplate,
  savePromptTemplate,
} from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import { activePromptKindAtom } from "./workbenchState";
import type {
  PromptTemplate,
  PromptTemplateKind,
  PromptTemplateView,
} from "./workbenchTypes";
import classes from "./workbench.module.css";

const PROMPT_KINDS: PromptTemplateKind[] = ["article", "summary", "image"];

/** 为三类模板准备不含真实业务数据的本地预览变量。 */
function previewValues(t: TFunction<"app">): Record<string, string> {
  return {
    article: t("prompts.preview_values.article"),
    keywords: t("prompts.preview_values.keywords"),
    requirements: t("prompts.preview_values.requirements"),
    summary: t("prompts.preview_values.summary"),
    title: t("prompts.preview_values.title"),
  };
}

/** 每个 Prompt 类型在页面会话内独立保留的未保存草稿。 */
type PromptDraft = Pick<PromptTemplate, "content" | "name">;
/** Prompt 页面当前允许执行的异步动作。 */
type PromptAction = "preview" | "save";

/** 管理正文、简介和配图 Prompt 的版本与零出站本地预览。 */
export function PromptTemplatesPage(): ReactElement {
  const { t } = useTranslation("app");
  const queryClient = useQueryClient();
  const [activeKind, setActiveKind] = useAtom(activePromptKindAtom);
  const [drafts, setDrafts] = useState<
    Partial<Record<PromptTemplateKind, PromptDraft>>
  >({});
  const [preview, setPreview] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<PromptAction | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const templatesQuery = useQuery({
    queryFn: listPromptTemplates,
    queryKey: ["prompt-templates"],
  });
  const templates = PROMPT_KINDS.map((kind) =>
    templatesQuery.data?.find((template) => template.kind === kind),
  ).filter((template): template is PromptTemplateView => Boolean(template));
  const activeTemplate = templates.find(
    (template) => template.kind === activeKind,
  );

  const activeDraft = activeTemplate
    ? (drafts[activeKind] ?? {
        content: activeTemplate.content,
        name: activeTemplate.name,
      })
    : null;
  const currentDraft =
    activeTemplate && activeDraft
      ? {
          content: activeDraft.content,
          id: activeTemplate.id,
          kind: activeTemplate.kind,
          name: activeDraft.name,
          version: activeTemplate.version,
        }
      : null;

  const handlePreview = async (): Promise<void> => {
    if (!currentDraft) return;
    setPendingAction("preview");
    setError(null);
    setFeedback(null);
    try {
      setPreview(await previewPromptTemplate(currentDraft, previewValues(t)));
      setFeedback(t("prompts.feedback.preview_ready"));
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setPendingAction(null);
    }
  };

  const handleSave = async (): Promise<void> => {
    if (!currentDraft || !activeTemplate) return;
    const variableContract = {
      allowedVariables: activeTemplate.allowedVariables,
      requiredVariables: activeTemplate.requiredVariables,
    };
    setPendingAction("save");
    setError(null);
    setFeedback(null);
    try {
      const saved = await savePromptTemplate(currentDraft);
      queryClient.setQueryData<PromptTemplateView[]>(
        ["prompt-templates"],
        (current = []) =>
          current.map((template) =>
            template.id === saved.id
              ? {
                  ...saved,
                  ...variableContract,
                }
              : template,
          ),
      );
      setFeedback(t("prompts.feedback.saved", { version: saved.version }));
    } catch (reason) {
      setError(localizedCommandError(reason, t));
    } finally {
      setPendingAction(null);
    }
  };

  if (templatesQuery.isPending) {
    return (
      <Stack align="center" gap="sm" mih={280} justify="center" role="status">
        <Loader aria-label={t("prompts.loading")} size="sm" />
        <Text c="dimmed">{t("prompts.loading")}</Text>
      </Stack>
    );
  }

  if (
    (templatesQuery.isError && !templatesQuery.data) ||
    !activeTemplate ||
    !currentDraft
  ) {
    return (
      <Alert
        color="red"
        icon={<IconAlertCircle aria-hidden="true" size={20} />}
        role="alert"
      >
        <Stack align="flex-start" gap="sm">
          <Text>{t("prompts.load_failed")}</Text>
          <Button
            leftSection={<IconRefresh aria-hidden="true" size={17} />}
            onClick={() => void templatesQuery.refetch()}
            variant="default"
          >
            {t("common.retry")}
          </Button>
        </Stack>
      </Alert>
    );
  }

  const requiredVariables = activeTemplate.requiredVariables;
  const optionalVariables = activeTemplate.allowedVariables.filter(
    (variable) => !requiredVariables.includes(variable),
  );

  return (
    <Stack gap="lg">
      <Stack gap={4}>
        <Title order={1}>{t("prompts.title")}</Title>
        <Text c="dimmed">{t("prompts.description")}</Text>
      </Stack>

      {templatesQuery.isFetching && !templatesQuery.isPending ? (
        <Alert
          color="blue"
          icon={<Loader aria-hidden="true" size="xs" />}
          role="status"
        >
          {t("prompts.refreshing")}
        </Alert>
      ) : null}
      {templatesQuery.isRefetchError ? (
        <Alert
          color="yellow"
          icon={<IconAlertCircle aria-hidden="true" size={20} />}
          role="alert"
        >
          <Group justify="space-between" wrap="wrap">
            <Text size="sm">{t("prompts.refresh_failed")}</Text>
            <Button
              onClick={() => void templatesQuery.refetch()}
              size="xs"
              variant="default"
            >
              {t("common.retry")}
            </Button>
          </Group>
        </Alert>
      ) : null}

      <Paper p="lg" radius="lg" withBorder>
        <Tabs
          onChange={(value) => {
            if (
              value === "article" ||
              value === "summary" ||
              value === "image"
            ) {
              setActiveKind(value);
              setPreview(null);
              setError(null);
              setFeedback(null);
            }
          }}
          value={activeKind}
        >
          <Tabs.List>
            {PROMPT_KINDS.map((kind) => {
              const template = templates.find((item) => item.kind === kind);
              return (
                <Tabs.Tab
                  disabled={pendingAction !== null && kind !== activeKind}
                  key={kind}
                  value={kind}
                >
                  {t(`prompts.kind.${kind}`)} · v{template?.version ?? 1}
                </Tabs.Tab>
              );
            })}
          </Tabs.List>

          {PROMPT_KINDS.map((kind) => (
            <Tabs.Panel key={kind} value={kind}>
              {kind === activeKind ? (
                <Stack gap="md" pt="lg">
                  <Textarea
                    autosize
                    disabled={pendingAction !== null}
                    label={t("prompts.name_label")}
                    minRows={1}
                    onChange={(event) => {
                      const name = event.currentTarget.value;
                      setPreview(null);
                      setError(null);
                      setFeedback(null);
                      setDrafts((current) => ({
                        ...current,
                        [activeKind]: {
                          content: currentDraft.content,
                          name,
                        },
                      }));
                    }}
                    required
                    value={currentDraft.name}
                  />
                  <Textarea
                    autosize
                    disabled={pendingAction !== null}
                    label={t("prompts.content_label")}
                    maxRows={18}
                    minRows={12}
                    onChange={(event) => {
                      const content = event.currentTarget.value;
                      setPreview(null);
                      setError(null);
                      setFeedback(null);
                      setDrafts((current) => ({
                        ...current,
                        [activeKind]: {
                          content,
                          name: currentDraft.name,
                        },
                      }));
                    }}
                    required
                    value={currentDraft.content}
                  />
                  <Group
                    aria-label={t("prompts.required_variables")}
                    gap="xs"
                    role="group"
                    wrap="wrap"
                  >
                    <Text c="dimmed" size="sm">
                      {t("prompts.required_variables")}
                    </Text>
                    {requiredVariables.map((variable) => (
                      <Badge color="blue" key={variable} variant="light">
                        {`{{${variable}}}`}
                      </Badge>
                    ))}
                  </Group>
                  <Group
                    aria-label={t("prompts.optional_variables")}
                    gap="xs"
                    role="group"
                    wrap="wrap"
                  >
                    <Text c="dimmed" size="sm">
                      {t("prompts.optional_variables")}
                    </Text>
                    {optionalVariables.map((variable) => (
                      <Badge key={variable} variant="outline">
                        {`{{${variable}}}`}
                      </Badge>
                    ))}
                  </Group>
                  <Alert color="blue">
                    {t("prompts.local_preview_notice")}
                  </Alert>
                  {error ? (
                    <Alert color="red" role="alert">
                      {error}
                    </Alert>
                  ) : null}
                  {feedback ? (
                    <Alert color="green" role="status">
                      {feedback}
                    </Alert>
                  ) : null}
                  {preview ? (
                    <Stack gap={6}>
                      <Text fw={600}>{t("prompts.preview_title")}</Text>
                      <Code block className={classes.artifactCode}>
                        {preview}
                      </Code>
                    </Stack>
                  ) : null}
                  <Group justify="flex-end">
                    <Button
                      disabled={pendingAction === "save"}
                      leftSection={<IconEye aria-hidden="true" size={17} />}
                      loading={pendingAction === "preview"}
                      onClick={() => void handlePreview()}
                      variant="default"
                    >
                      {t("prompts.preview")}
                    </Button>
                    <Button
                      disabled={
                        pendingAction === "preview" ||
                        currentDraft.name.trim().length === 0 ||
                        currentDraft.content.trim().length === 0
                      }
                      leftSection={
                        <IconDeviceFloppy aria-hidden="true" size={17} />
                      }
                      loading={pendingAction === "save"}
                      onClick={() => void handleSave()}
                    >
                      {t("prompts.save_new_version")}
                    </Button>
                  </Group>
                </Stack>
              ) : null}
            </Tabs.Panel>
          ))}
        </Tabs>
      </Paper>
    </Stack>
  );
}
