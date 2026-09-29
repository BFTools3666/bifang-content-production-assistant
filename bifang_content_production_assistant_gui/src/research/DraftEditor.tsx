import {
  Accordion,
  Alert,
  Badge,
  Button,
  Group,
  Select,
  Checkbox,
  Divider,
  Stack,
  Text,
  Textarea,
  TextInput,
} from "@mantine/core";
import { IconCopy, IconDownload, IconDeviceFloppy } from "@tabler/icons-react";
import { useNavigate } from "@tanstack/react-router";
import { useAtom } from "jotai";
import {
  inspectedArticleIdAtom,
  articleDetailsTabAtom,
} from "../workbench/workbenchState";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import {
  exportDraft,
  researchError,
  updateDraft,
  prepareDraftArticle,
} from "./researchCommands";
import { listConnectorConfigs } from "../workbench/workbenchCommands";
import { draftEditsAtom, RESEARCH_QUERY_KEY } from "./researchState";
import type { ResearchDraft, ResearchData } from "./researchTypes";
import styles from "./ResearchPage.module.css";

/** 编辑、复制和导出真实AI草稿，成功反馈只在宿主动作确实完成后出现。 */
export function DraftEditor({
  draft,
  mode = "all",
  onSaved,
}: {
  draft: ResearchDraft;
  mode?: "all" | "review" | "export";
  onSaved?: () => void;
}): ReactElement {
  const { t } = useTranslation("research");
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [, inspectArticle] = useAtom(inspectedArticleIdAtom);
  const [, setArticleTab] = useAtom(articleDetailsTabAtom);
  const connectors = useQuery({
    queryKey: ["connector-configs"],
    queryFn: listConnectorConfigs,
    retry: false,
  });
  const [checks, setChecks] = useState([false, false, false]);
  const [edits, setEdits] = useAtom(draftEditsAtom);
  const { title, body } = edits[draft.id] ?? draft;
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    setMessage("");
    setError("");
    setChecks([false, false, false]);
  }, [draft.id]);
  /** 表单输入保留在当前会话，不能用后台重新读取的旧正文覆盖。 */
  function editContent(change: Partial<{ title: string; body: string }>) {
    setEdits((previous) => ({
      ...previous,
      [draft.id]: { title, body, ...change },
    }));
    setMessage("");
  }
  const save = useMutation({
    mutationFn: () => updateDraft(draft.id, title, body),
    onSuccess: () => {
      setMessage(t("草稿修改已保存"));
      setError("");
      queryClient.setQueryData<ResearchData>(RESEARCH_QUERY_KEY, (previous) =>
        previous
          ? {
              ...previous,
              drafts: previous.drafts.map((item) =>
                item.id === draft.id ? { ...item, title, body } : item,
              ),
            }
          : previous,
      );
      setEdits((previous) => {
        const next = { ...previous };
        delete next[draft.id];
        return next;
      });
      void queryClient.invalidateQueries({ queryKey: RESEARCH_QUERY_KEY });
      onSaved?.();
    },
    onError: (failure) => setError(researchError(failure)),
  });
  const exporting = useMutation({
    mutationFn: async (format: "markdown" | "text" | "html") => {
      return exportDraft(draft.id, format);
    },
    onSuccess: (saved) => {
      if (saved) setMessage(t("文件已导出"));
      void queryClient.invalidateQueries({ queryKey: RESEARCH_QUERY_KEY });
    },
    onError: (failure) => setError(researchError(failure)),
  });

  const dirty = title !== draft.title || body !== draft.body;
  const delivery = useMutation({
    mutationFn: () => prepareDraftArticle(draft.id),
    onSuccess: (id) => {
      inspectArticle(id);
      setArticleTab("summary");
      void queryClient.invalidateQueries({ queryKey: ["workbench"] });
      void navigate({ to: "/batch" });
    },
    onError: (failure) => setError(researchError(failure)),
  });
  const busy = save.isPending || exporting.isPending || delivery.isPending;

  /** 复制用户当前编辑的原创文本，剪贴板失败不能显示成功。 */
  async function copyContent() {
    try {
      await navigator.clipboard.writeText(
        `${title}\n\n${body}\n\n${draft.tags.map((tag) => `#${tag}`).join(" ")}`,
      );
      setMessage(t("标题和正文已复制"));
      setError("");
    } catch {
      setError(t("剪贴板写入失败，请保存或导出文件"));
    }
  }

  return (
    <section className={styles.paper}>
      <Group justify="space-between" mb="lg">
        <Badge variant="light">{t("已保存原创草稿")}</Badge>
        <Text size="xs" c="dimmed">
          {body.length} {t("字")} · {draft.model}
        </Text>
      </Group>
      {error ? (
        <Alert color="red" role="alert" mb="md">
          {error}
        </Alert>
      ) : null}
      <Stack gap="md">
        <Select
          label={t("选择二次标题")}
          disabled={busy || mode === "export"}
          data={draft.titles.map((candidate) => ({
            value: candidate,
            label: candidate,
          }))}
          value={draft.titles.includes(title) ? title : null}
          onChange={(candidate) => {
            if (candidate) editContent({ title: candidate });
          }}
          placeholder={t("也可以直接修改标题")}
        />
        <TextInput
          label={t("当前标题")}
          classNames={{ input: styles.paperTitle }}
          disabled={busy || mode === "export"}
          value={title}
          onChange={(event) =>
            editContent({ title: event.currentTarget.value })
          }
        />
        <Textarea
          label={t("正文／笔记")}
          disabled={busy || mode === "export"}
          autosize
          minRows={14}
          maxRows={30}
          value={body}
          onChange={(event) => editContent({ body: event.currentTarget.value })}
        />
        <Group gap="xs">
          {draft.tags.map((tag) => (
            <Badge key={tag} variant="outline">
              #{tag}
            </Badge>
          ))}
        </Group>
        {mode === "review" ? (
          <Stack gap="xs">
            <Divider label={t("保存前核对")} />
            {[
              "事实、数字与引用有依据",
              "标题和正文讲的是同一件事",
              "表达符合自己的口吻",
            ].map((label, index) => (
              <Checkbox
                key={label}
                label={t(label)}
                checked={checks[index] ?? false}
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setChecks((previous) =>
                    previous.map((value, i) => (i === index ? checked : value)),
                  );
                }}
              />
            ))}
            <Text size="xs" c="dimmed">
              {t("这是你的检查清单，不是 AI 自动审核结论。")}
            </Text>
          </Stack>
        ) : null}
        <Group>
          {mode !== "export" ? (
            <Button
              leftSection={<IconDeviceFloppy size={16} />}
              loading={save.isPending}
              disabled={busy || delivery.isPending}
              onClick={() => save.mutate()}
            >
              {mode === "review" ? t("保存并准备导出") : t("保存修改")}
            </Button>
          ) : null}
          <Button
            variant="default"
            leftSection={<IconCopy size={16} />}
            onClick={() => void copyContent()}
          >
            {t("复制内容")}
          </Button>
          {mode !== "review" ? (
            <>
              <Button
                variant="default"
                leftSection={<IconDownload size={16} />}
                loading={exporting.isPending}
                disabled={dirty || busy || delivery.isPending}
                onClick={() => exporting.mutate("markdown")}
              >
                Markdown
              </Button>
              <Button
                variant="default"
                disabled={dirty || busy || delivery.isPending}
                onClick={() => exporting.mutate("text")}
              >
                TXT
              </Button>
              <Button
                variant="default"
                disabled={dirty || busy || delivery.isPending}
                onClick={() => exporting.mutate("html")}
              >
                HTML
              </Button>
            </>
          ) : null}
        </Group>
        {dirty ? (
          <Text size="sm" c="orange">
            {t("修改尚未保存。切换页面会保留本次输入，关闭软件前请保存。")}
            {mode === "export" ? (
              <Button
                ml="sm"
                variant="default"
                disabled={busy}
                loading={save.isPending}
                onClick={() => save.mutate()}
              >
                {t("保存修改")}
              </Button>
            ) : null}
          </Text>
        ) : null}
        {mode !== "review" ? (
          <Stack gap="xs">
            <Divider label={t("飞书与后续制作")} />
            <Text size="sm" c="dimmed">
              {connectors.isPending
                ? t("正在读取飞书连接")
                : connectors.isError
                  ? t("飞书配置读取失败，请到设置重试。")
                  : connectors.data?.some((item) => item.enabled)
                    ? t("已有飞书连接，完成配图和排版后可确认同步。")
                    : t("飞书尚未连接，本地保存和导出可以正常使用。")}
            </Text>
            <Group>
              <Button
                variant="default"
                disabled={dirty || busy}
                loading={delivery.isPending}
                onClick={() => delivery.mutate()}
              >
                {t("继续简介、配图与飞书同步")}
              </Button>
              <Button
                variant="subtle"
                disabled={busy || delivery.isPending}
                onClick={() => void navigate({ to: "/connectors" })}
              >
                {t("管理飞书连接")}
              </Button>
              <Button
                variant="subtle"
                onClick={() => void navigate({ to: "/drafts" })}
              >
                {t("查看我的文章")}
              </Button>
            </Group>
            <Text size="xs" c="dimmed">
              {t("正文将保留并交给原批量流程；飞书写入仍需另行确认。")}
            </Text>
          </Stack>
        ) : null}

        {message ? (
          <Text size="sm" role="status" c="brand">
            {message}
          </Text>
        ) : null}
        <Accordion>
          <Accordion.Item value="trace">
            <Accordion.Control>{t("查看来源与生成记录")}</Accordion.Control>
            <Accordion.Panel>
              <Text size="xs" c="dimmed">
                {new Date(draft.createdAtMs).toLocaleString()} · {t("参考作品")}{" "}
                {draft.sourceIds.length} · {t("风格")}{" "}
                {draft.styleId || t("未应用")} · {t("总Token")}{" "}
                {draft.tokenUsage?.totalTokens ?? t("未提供")}
              </Text>
              <Textarea
                label={t("本次实际提示词")}
                value={draft.prompt}
                readOnly
                minRows={6}
                autosize
                maxRows={16}
              />
            </Accordion.Panel>
          </Accordion.Item>
        </Accordion>
      </Stack>
    </section>
  );
}
