import { Accordion, Alert, Badge, Button, Group, Select, Stack, Text, Textarea, TextInput } from "@mantine/core";
import { IconCopy, IconDownload, IconDeviceFloppy } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { exportDraft, researchError, updateDraft } from "./researchCommands";
import { RESEARCH_QUERY_KEY } from "./researchState";
import type { ResearchDraft } from "./researchTypes";
import styles from "./ResearchPage.module.css";

/** 编辑、复制和导出真实AI草稿，成功反馈只在宿主动作确实完成后出现。 */
export function DraftEditor({ draft }: { draft: ResearchDraft }): ReactElement {
  const { t } = useTranslation("research");
  const queryClient = useQueryClient();
  const [title,setTitle] = useState(draft.title);
  const [body,setBody] = useState(draft.body);
  const [message,setMessage] = useState("");
  const [error,setError] = useState("");
  useEffect(() => {setTitle(draft.title);setBody(draft.body);setMessage("");setError("");},[draft.id,draft.title,draft.body]);
  const save = useMutation({mutationFn:() => updateDraft(draft.id,title,body),onSuccess:() => {setMessage(t("草稿修改已保存"));setError("");void queryClient.invalidateQueries({queryKey:RESEARCH_QUERY_KEY});},onError:failure => setError(researchError(failure))});
  const exporting = useMutation({mutationFn:async (format:"markdown"|"text") => {await updateDraft(draft.id,title,body); return exportDraft(draft.id,format);},onSuccess:saved => {if(saved) setMessage(t("文件已导出"));void queryClient.invalidateQueries({queryKey:RESEARCH_QUERY_KEY});},onError:failure => setError(researchError(failure))});

  /** 复制用户当前编辑的原创文本，剪贴板失败不能显示成功。 */
  async function copyContent() {
    try {await navigator.clipboard.writeText(`${title}\n\n${body}\n\n${draft.tags.map(tag => `#${tag}`).join(" ")}`);setMessage(t("标题和正文已复制"));setError("");} catch {setError(t("剪贴板写入失败，请保存或导出文件"));}
  }

  return <section className={styles.paper}>
    <Group justify="space-between" mb="lg"><Badge variant="light">{t("已保存原创草稿")}</Badge><Text size="xs" c="dimmed">{body.length} {t("字")} · {draft.model}</Text></Group>
    {error ? <Alert color="red" role="alert" mb="md">{error}</Alert> : null}
    <Stack gap="md"><Select label={t("选择二次标题")} data={draft.titles.map(candidate => ({value:candidate,label:candidate}))} value={draft.titles.includes(title) ? title : null} onChange={candidate => {if(candidate)setTitle(candidate);}} placeholder={t("也可以直接修改标题")}/><TextInput label={t("当前标题")} classNames={{input:styles.paperTitle}} value={title} onChange={event => setTitle(event.currentTarget.value)}/><Textarea label={t("正文／笔记")} autosize minRows={14} maxRows={30} value={body} onChange={event => setBody(event.currentTarget.value)}/><Group gap="xs">{draft.tags.map(tag => <Badge key={tag} variant="outline">#{tag}</Badge>)}</Group>
      <Group><Button leftSection={<IconDeviceFloppy size={16}/>} loading={save.isPending} onClick={() => save.mutate()}>{t("保存修改")}</Button><Button variant="default" leftSection={<IconCopy size={16}/>} onClick={() => void copyContent()}>{t("复制内容")}</Button><Button variant="default" leftSection={<IconDownload size={16}/>} loading={exporting.isPending} onClick={() => exporting.mutate("markdown")}>Markdown</Button><Button variant="subtle" disabled={exporting.isPending} onClick={() => exporting.mutate("text")}>TXT</Button></Group>
      {message ? <Text size="sm" role="status" c="brand">{message}</Text> : null}
      <Accordion><Accordion.Item value="trace"><Accordion.Control>{t("查看来源与生成记录")}</Accordion.Control><Accordion.Panel><Text size="xs" c="dimmed">{new Date(draft.createdAtMs).toLocaleString()} · {t("参考作品")} {draft.sourceIds.length} · {t("风格")} {draft.styleId || t("未应用")} · {t("总Token")} {draft.tokenUsage?.totalTokens ?? t("未提供")}</Text><Textarea label={t("本次实际提示词")} value={draft.prompt} readOnly minRows={6} autosize maxRows={16}/></Accordion.Panel></Accordion.Item></Accordion>
    </Stack>
  </section>;
}
