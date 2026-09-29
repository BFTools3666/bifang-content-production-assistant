import { Alert, Anchor, Button, Group, Modal, Stack, Text } from "@mantine/core";
import { IconFileText } from "@tabler/icons-react";
import { useNavigate } from "@tanstack/react-router";
import { type ReactElement } from "react";
import { useAtom } from "jotai";
import { useTranslation } from "react-i18next";
import { researchError } from "./researchCommands";
import { useResearchLibrary, historyDraftIdAtom, platformLabel } from "./researchState";
import { DraftEditor } from "./DraftEditor";
import { ResearchHeader } from "./ResearchHeader";
import styles from "./ResearchPage.module.css";

/** 展示退出重启后仍保留的真实二创草稿与生成来源。 */
export function ResearchDraftsPage(): ReactElement {
  const {t} = useTranslation("research");
  const library = useResearchLibrary();
  const navigate = useNavigate();
  const [selectedId,setSelectedId] = useAtom(historyDraftIdAtom);
  const draft = library.data?.drafts.find(item => item.id === selectedId);
  return <main className={styles.page}>
    <ResearchHeader title={t("原创草稿记录")} description={t("每次成功生成都会保存。标题、正文、参考作品、风格和模型记录都可以找回。")}/>
    {library.error ? <Alert color="red">{researchError(library.error)}</Alert> : null}
    <Group justify="space-between" mb="md"><Text fw={650}>{library.data?.drafts.length || 0} {t("草稿")}</Text><Button onClick={() => void navigate({to:"/writer"})}>{t("开始新写作")}</Button></Group>
    {library.data?.drafts.length ? <div className={styles.styleList}>{library.data.drafts.map(item => <article key={item.id} className={styles.styleRow}><div><Anchor component="button" fw={650} size="lg" onClick={() => setSelectedId(item.id)}>{item.title}</Anchor><Text size="xs" c="dimmed" mt={6}>{new Date(item.createdAtMs).toLocaleString()} · {platformLabel(item.platform)} · {item.model}</Text></div><Text c="dimmed" size="sm" lineClamp={3}>{item.body}</Text><Button variant="default" onClick={() => setSelectedId(item.id)}>{t("编辑和导出")}</Button></article>)}</div> : <div className={styles.empty}><IconFileText size={36}/><Text fw={600} mt="md">{t("还没有生成草稿")}</Text><Text size="sm" c="dimmed" mt="xs">{t("完成第一次标题与正文生成后，草稿会出现在这里。")}</Text></div>}
    <Modal opened={Boolean(draft)} onClose={() => setSelectedId(null)} title={t("原创草稿")} size="xl">{draft ? <Stack><DraftEditor draft={draft}/></Stack> : null}</Modal>
  </main>;
}
