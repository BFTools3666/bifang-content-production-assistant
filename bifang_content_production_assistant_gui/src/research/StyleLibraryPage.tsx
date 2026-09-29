import { Accordion, Alert, Anchor, Badge, Button, Group, Modal, Select, Stack, Text, Textarea, TextInput } from "@mantine/core";
import { IconArrowRight, IconBook2, IconEdit, IconSparkles } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useAtom } from "jotai";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { listProviderConfigs } from "../workbench/workbenchCommands";
import { analyzeStyle, researchError, updatePrompts, updateStyle } from "./researchCommands";
import { benchmarkSessionAtom, composerSessionAtom, platformLabel, RESEARCH_QUERY_KEY, useResearchLibrary } from "./researchState";
import type { StyleProfile } from "./researchTypes";
import { ResearchHeader } from "./ResearchHeader";
import styles from "./ResearchPage.module.css";

/** 依据同一公开主页的已采集正文分析并保存多个风格，应用时保留当前写作需求。 */
export function StyleLibraryPage(): ReactElement {
  const { t } = useTranslation("research");
  const library = useResearchLibrary();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [benchmark] = useAtom(benchmarkSessionAtom);
  const [, setComposer] = useAtom(composerSessionAtom);
  const providers = useQuery({queryKey:["provider-configs"],queryFn:listProviderConfigs});
  const [name, setName] = useState("");
  const [providerId, setProviderId] = useState<string | null>(null);
  const [editing, setEditing] = useState<StyleProfile | null>(null);
  const [analysisPrompt, setAnalysisPrompt] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const samples = (library.data?.works || []).filter(work => benchmark.selected.includes(work.id) && work.platform === benchmark.platform);
  const homepage = samples[0]?.authorUrl || benchmark.authorUrl;
  const complete = samples.filter(work => work.body.length >= 80);
  const sameAuthor = Boolean(homepage) && samples.every(work => work.authorUrl === homepage);
  const textProviders = (providers.data || []).filter(provider => provider.enabled && provider.kind === "openAiCompatible" && provider.hasApiKey && provider.articleModel?.trim());
  const actualProvider = providerId || textProviders.find(provider => provider.defaultForArticle)?.id || textProviders[0]?.id || null;
  const create = useMutation({
    mutationFn: () => analyzeStyle(benchmark.platform, homepage, samples.map(work => work.id), name.trim() || samples[0]?.author || t("新风格"), actualProvider || ""),
    onSuccess: (saved) => { setError(""); setMessage(`${t("风格已保存")}：${saved.name}`); void queryClient.invalidateQueries({queryKey: RESEARCH_QUERY_KEY}); },
    onError: failure => setError(researchError(failure)),
  });
  const save = useMutation({
    mutationFn: (style: StyleProfile) => updateStyle(style),
    onSuccess: () => { setEditing(null); setError(""); setMessage(t("风格修改已保存")); void queryClient.invalidateQueries({queryKey:RESEARCH_QUERY_KEY}); },
    onError: failure => setError(researchError(failure)),
  });
  const savePrompt = useMutation({mutationFn: () => updatePrompts({...library.data!.prompts, styleAnalysis:analysisPrompt ?? library.data!.prompts.styleAnalysis}), onSuccess: () => {setMessage(t("内置提示词已保存")); void queryClient.invalidateQueries({queryKey:RESEARCH_QUERY_KEY});}, onError:failure => setError(researchError(failure))});

  /** 一键切换目标平台与风格，将编辑交给原有进程内写作状态。 */
  function applyStyle(style: StyleProfile) {
    setComposer(previous => ({...previous, platform:style.platform, styleId:style.id, sourceIds:[], keyword:previous.keyword || benchmark.keyword}));
    void navigate({to:"/writer"});
  }

  return <main className={styles.page}>
    <ResearchHeader title={t("把对标沉淀成自己的风格库")} description={t("用同一主页的多篇作品提炼写法。保存多个风格，每次写作都可以切换。")} />
    {error || library.error ? <Alert color="red" role="alert">{error || researchError(library.error)}</Alert> : null}
    {message ? <Text role="status" size="sm" c="brand" py="sm">{message}</Text> : null}
    <section className={styles.section}>
      <Group justify="space-between" mb="md"><Text fw={650}>{t("分析当前所选作品")}</Text><Button variant="subtle" onClick={() => void navigate({to:"/benchmarks"})}>{t("返回作品筛选")}</Button></Group>
      <Text size="sm" c="dimmed">{samples.length ? `${samples[0]?.author || homepage} · ${t("已选择")} ${samples.length} · ${t("已采集正文")} ${complete.length}` : t("先在对标研究台按数据筛选作者主页，再选择同一作者的3至10件作品并采集正文。")}</Text>
      {homepage ? <Anchor href={homepage} target="_blank" rel="noreferrer" size="xs">{homepage}</Anchor> : null}
      <div className={styles.toolbar} style={{marginTop:16}}>
        <TextInput className={styles.search} label={t("风格名称")} placeholder={samples[0]?.author || t("例如：清晰教程、短句经验分享")} value={name} onChange={event => setName(event.currentTarget.value)}/>
        <Select label={t("文本AI服务")} w={250} data={textProviders.map(provider => ({value:provider.id,label:provider.name}))} value={actualProvider} onChange={setProviderId} placeholder={t("先在AI服务页接入自己的API")}/>
        <Button leftSection={<IconSparkles size={17}/>} loading={create.isPending} disabled={!actualProvider || complete.length < 3 || complete.length !== samples.length || samples.length > 10 || !sameAuthor} onClick={() => create.mutate()}>{t("分析并保存风格")}</Button>
      </div>
      {samples.length > 0 && (!sameAuthor || complete.length !== samples.length || samples.length < 3 || samples.length > 10) ? <Text size="xs" c="orange">{t("需要同一主页3至10件作品，且每件都已采集正文。")}</Text> : null}
      {!actualProvider ? <Button variant="subtle" onClick={() => void navigate({to:"/providers"})}>{t("配置自己的AI服务")}</Button> : null}
      <Accordion variant="default" mt="md"><Accordion.Item value="analysis"><Accordion.Control>{t("查看和修改内置风格分析提示词")}</Accordion.Control><Accordion.Panel><Textarea value={analysisPrompt ?? library.data?.prompts.styleAnalysis ?? ""} onChange={event => setAnalysisPrompt(event.currentTarget.value)} minRows={5} autosize maxRows={14} label={t("分析提示词")}/><Button mt="sm" variant="default" loading={savePrompt.isPending} disabled={!library.data} onClick={() => savePrompt.mutate()}>{t("保存内置提示词")}</Button></Accordion.Panel></Accordion.Item></Accordion>
    </section>
    <Group justify="space-between" mt="xl"><Text fw={650}>{t("已保存风格")}</Text><Badge variant="light">{library.data?.styles.length || 0}</Badge></Group>
    {library.data?.styles.length ? <div className={styles.styleList}>{library.data.styles.map(style => <article key={style.id} className={styles.styleRow}>
      <div><Text fw={650} size="lg">{style.name}</Text><Text size="xs" c="dimmed" mt={4}>{style.author} · {platformLabel(style.platform)} · {style.samples.length} {t("正文样本")} · {t("版本")} {style.revision}</Text><Anchor href={style.homepageUrl} target="_blank" rel="noreferrer" size="xs">{t("来源主页")}</Anchor></div>
      <Text size="sm" c="dimmed" lineClamp={3}>{style.instructions}</Text>
      <Group gap="xs"><Button variant="default" size="sm" leftSection={<IconEdit size={15}/>} onClick={() => setEditing(style)}>{t("查看和编辑")}</Button><Button size="sm" rightSection={<IconArrowRight size={15}/>} onClick={() => applyStyle(style)}>{t("应用风格写作")}</Button></Group>
    </article>)}</div> : <div className={styles.empty}><IconBook2 size={34}/><Text mt="md" fw={600}>{t("风格库还没有对标")}</Text><Text mt="xs" c="dimmed" size="sm">{t("采集同一主页的真实作品，分析后将写作规律保存在这里。")}</Text></div>}
    <Modal opened={Boolean(editing)} onClose={() => !save.isPending && setEditing(null)} title={t("编辑风格应用指令")} size="xl" closeOnClickOutside={!save.isPending}>
      {editing ? <Stack><TextInput label={t("风格名称")} value={editing.name} onChange={event => setEditing({...editing,name:event.currentTarget.value})}/><Textarea label={t("可修改的风格指令")} value={editing.instructions} onChange={event => setEditing({...editing,instructions:event.currentTarget.value})} minRows={10} autosize maxRows={22}/><Text size="xs" c="dimmed">{t("修改影响下次生成，已有草稿和来源样本保持可追溯。")}</Text><Accordion><Accordion.Item value="source"><Accordion.Control>{t("查看分析来源")}</Accordion.Control><Accordion.Panel><Stack gap="xs">{editing.samples.map(work => <Anchor key={work.id} href={work.url} target="_blank" rel="noreferrer">{work.title}</Anchor>)}</Stack><div className={styles.analysis}>{editing.analysis}</div></Accordion.Panel></Accordion.Item></Accordion><Button loading={save.isPending} onClick={() => save.mutate(editing)}>{t("保存风格修改")}</Button></Stack> : null}
    </Modal>
  </main>;
}
