import { Alert, Anchor, Badge, Button, Checkbox, Group, Loader, NumberInput, Select, Stack, Text, TextInput } from "@mantine/core";
import { IconArrowRight, IconExternalLink, IconRefresh, IconSearch } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useAtom } from "jotai";
import { useEffect, useMemo, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { getCollectionStatus, researchError, startCollection } from "./researchCommands";
import { benchmarkSessionAtom, composerSessionAtom, writerModeAtom, writerStepAtom, writerDraftIdAtom, metricText, platformOptions, RESEARCH_QUERY_KEY, useResearchLibrary } from "./researchState";
import type { BenchmarkWork, ContentPlatform } from "./researchTypes";
import { ResearchHeader } from "./ResearchHeader";
import styles from "./ResearchPage.module.css";

/** 显示真实作品与公开指标，筛选和控件动作不由表格父级代理。 */
export function BenchmarkPage(): ReactElement {
  const { t } = useTranslation("research");
  const library = useResearchLibrary();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [, setWriterMode] = useAtom(writerModeAtom);
  const [, setWriterStep] = useAtom(writerStepAtom);
  const [, setWriterDraft] = useAtom(writerDraftIdAtom);
  const [session, setSession] = useAtom(benchmarkSessionAtom);
  const [, setComposer] = useAtom(composerSessionAtom);
  const [pageUrl, setPageUrl] = useState("");
  const [error, setError] = useState("");
  const collection = useQuery({ queryKey: ["collection-status"], queryFn: getCollectionStatus, refetchInterval: 2500, retry: false });
  useEffect(() => { if (collection.data?.updatedAtMs) void queryClient.invalidateQueries({queryKey: RESEARCH_QUERY_KEY}); }, [collection.data?.updatedAtMs, queryClient]);
  const start = useMutation({
    mutationFn: (url: string | string[] | null) => startCollection(session.platform, session.keyword, Array.isArray(url) ? null : url, Array.isArray(url) ? url : null),
    onSuccess: () => { setError(""); void collection.refetch(); },
    onError: (failure) => setError(researchError(failure)),
  });
  const works = useMemo(() => (library.data?.works || []).filter(work => work.platform === session.platform), [library.data, session.platform]);
  const authors = useMemo(() => {
    const grouped = new Map<string, BenchmarkWork[]>();
    for (const work of works) if (work.authorUrl) grouped.set(work.authorUrl, [...(grouped.get(work.authorUrl) || []), work]);
    return [...grouped.entries()].map(([url, samples]) => ({ url, author: samples[0]?.author || new URL(url).pathname, count: samples.length, likes: samples.reduce((total, sample) => total + (sample.metrics.likes || 0), 0), known: samples.some(sample => sample.metrics.likes !== null) })).sort((a, b) => b.likes - a.likes);
  }, [works]);
  const visible = useMemo(() => works.filter(work => {
    const matches = `${work.title} ${work.author}`.toLowerCase().includes(session.query.toLowerCase());
    const minimum = (value: number | null, boundary: number) => boundary <= 0 || (value !== null && value >= boundary);
    return matches && (!session.authorUrl || work.authorUrl === session.authorUrl) && minimum(work.metrics.views, session.minimumViews) && minimum(work.metrics.favorites, session.minimumFavorites) && minimum(work.metrics.likes, session.minimumLikes);
  }).sort((left, right) => {
    if (session.sort === "recent") return right.collectedAtMs - left.collectedAtMs;
    const a = left.metrics[session.sort], b = right.metrics[session.sort];
    return a === null ? b === null ? 0 : 1 : b === null ? -1 : b - a;
  }), [works, session]);
  const selected = works.filter(work => session.selected.includes(work.id));
  const full = selected.filter(work => work.body.length >= 80);

  /** 把已选择作品与关键词带入二创页，路由切换保留当前筛选。 */
  function writeSelected() {
    setComposer(previous => ({ ...previous, platform: session.platform, keyword: session.keyword || previous.keyword, sourceIds: selected.map(work => work.id), styleId: null }));
    setWriterMode("reference"); setWriterStep(0); setWriterDraft(null); void navigate({to: "/writer"});
  }

  return <main className={styles.page}>
    <ResearchHeader title={t("对标作品研究台")} description={t("先看作品表现，再决定研究谁。链接、指标、正文和采集时间都保留。")} />
    <div className={styles.toolbar}>
      <Select label={t("来源平台")} data={platformOptions()} value={session.platform} allowDeselect={false} onChange={value => setSession({...session, platform: value as ContentPlatform, selected: [], authorUrl: ""})} w={150}/>
      <TextInput className={styles.search} label={t("指定关键词")} placeholder={t("例如：Python自动化、AI办公效率")} value={session.keyword} onChange={event => setSession({...session, keyword:event.currentTarget.value})} onKeyDown={event => {if(event.key === "Enter" && session.keyword.trim()) start.mutate(null);}}/>
      <Button leftSection={<IconSearch size={18}/>} loading={start.isPending} disabled={!session.keyword.trim()} onClick={() => start.mutate(null)}>{t("搜索并采集")}</Button>
    </div>
    <div className={styles.toolbar}>
      <TextInput className={styles.search} aria-label={t("作品或主页链接")} placeholder={t("也可以直接粘贴作品或作者主页链接")} value={pageUrl} onChange={event => setPageUrl(event.currentTarget.value)}/>
      <Button variant="default" leftSection={<IconExternalLink size={16}/>} disabled={!pageUrl.trim() || start.isPending} onClick={() => start.mutate(pageUrl)}>{t("打开并采集链接")}</Button>
      <Button variant="subtle" leftSection={<IconRefresh size={16}/>} onClick={() => void library.refetch()}>{t("刷新作品")}</Button>
    </div>
    {error || library.error ? <Alert color="red" role="alert">{error || researchError(library.error)}</Alert> : null}
    <Text className={styles.status} role="status">{collection.data?.message || t("首次采集会打开可见窗口，平台需要登录时请在该窗口登录。打开详情后可补充正文与收藏等指标。")}</Text>
    <Text size="xs" c="dimmed" mb="sm">{t("正文以平台当前公开可见部分为准；页面折叠或只显示预览时，请展开后重新采集。")}</Text>
    <Text size="xs" c="dimmed" mb="sm">{t("指标来自各次公开页面采集；本次未读到的指标保留已有数值。")}</Text>
    <section className={styles.section}>
      <Group justify="space-between" mb="md"><Text fw={600}>{t("用公开数据筛选")}</Text><Text size="xs" c="dimmed">{t("未提供的指标不推算；设置最低值时会排除缺失项。")}</Text></Group>
      <div className={styles.toolbar}>
        <TextInput className={styles.search} label={t("筛选标题或作者")} value={session.query} onChange={event => setSession({...session, query:event.currentTarget.value})}/>
        <Select label={t("作者主页")} data={[{value:"",label:t("全部主页")}, ...authors.map(author => ({value:author.url,label:`${author.author} · ${author.count} ${t("作品")} · ${author.known ? author.likes.toLocaleString() : t("未提供")} ${t("公开已知赞")}`}))]} searchable value={session.authorUrl} onChange={value => setSession({...session, authorUrl:value || ""})} w={260}/>
        <Select label={t("排序")} data={[{value:"views",label:t("阅读／播放量")},{value:"favorites",label:t("收藏量")},{value:"likes",label:t("点赞量")},{value:"comments",label:t("评论量")},{value:"recent",label:t("最近采集")}]} value={session.sort} allowDeselect={false} onChange={value => setSession({...session, sort:value as typeof session.sort})} w={150}/>
      </div>
      <Group gap="md">
        <NumberInput label={t("最低阅读／播放")} min={0} allowDecimal={false} value={session.minimumViews} onChange={value => setSession({...session, minimumViews:Number(value) || 0})} w={160}/>
        <NumberInput label={t("最低收藏")} min={0} allowDecimal={false} value={session.minimumFavorites} onChange={value => setSession({...session, minimumFavorites:Number(value) || 0})} w={140}/>
        <NumberInput label={t("最低点赞")} min={0} allowDecimal={false} value={session.minimumLikes} onChange={value => setSession({...session, minimumLikes:Number(value) || 0})} w={140}/>
        <Button mt={24} variant="subtle" onClick={() => setSession({...session, minimumViews:0,minimumFavorites:0,minimumLikes:0,query:"",authorUrl:""})}>{t("清空筛选")}</Button>
      </Group>
    </section>
    <Group justify="space-between" py="lg">
      <Group gap="xs"><Badge variant="light">{visible.length} {t("作品")}</Badge><Text size="sm">{t("已选择")} {selected.length} · {t("已采集正文")} {full.length}</Text></Group>
      <Group gap="sm">
        <Button variant="default" loading={start.isPending} disabled={!selected.length || selected.length > 10} onClick={() => start.mutate(selected.map(work => work.url))}>{t("批量采集所选正文")}</Button>
        {session.authorUrl ? <Button variant="default" onClick={() => start.mutate(session.authorUrl)}>{t("采集这个主页")}</Button> : null}
        <Button variant="default" disabled={!selected.length} onClick={() => {void navigate({to:"/styles"});}}>{t("分析所选作品风格")}</Button>
        <Button rightSection={<IconArrowRight size={16}/>} disabled={!selected.length} onClick={writeSelected}>{t("用所选作品二创")}</Button>
      </Group>
    </Group>
    {library.isPending ? <Loader aria-label={t("正在读取作品")}/> : visible.length ? <table className={styles.workTable}>
      <colgroup><col style={{width:"42px"}}/><col style={{width:"34%"}}/><col/><col/><col/><col/><col/><col style={{width:"135px"}}/></colgroup>
      <thead><tr><th><Checkbox aria-label={t("选择当前筛选的作品")} checked={visible.every(work => session.selected.includes(work.id))} indeterminate={visible.some(work => session.selected.includes(work.id)) && !visible.every(work => session.selected.includes(work.id))} onChange={event => setSession({...session, selected:event.currentTarget.checked ? [...new Set([...session.selected,...visible.map(work => work.id)])] : session.selected.filter(id => !visible.some(work => work.id === id))})}/></th><th>{t("作品与来源")}</th><th>{t("阅读／播放")}</th><th>{t("点赞")}</th><th>{t("收藏")}</th><th>{t("评论")}</th><th>{t("分享")}</th><th>{t("下一步")}</th></tr></thead>
      <tbody>{visible.map(work => <tr key={work.id} data-selected={session.selected.includes(work.id)}>
        <td><Checkbox aria-label={`${t("选择")} ${work.title}`} checked={session.selected.includes(work.id)} onChange={event => setSession({...session, selected:event.currentTarget.checked ? [...session.selected,work.id] : session.selected.filter(id => id !== work.id)})}/></td>
        <td><Anchor className={styles.workTitle} href={work.url} target="_blank" rel="noreferrer">{work.title}</Anchor><div className={styles.metadata}>{work.author || t("作者未提供")} · {new Date(work.collectedAtMs).toLocaleString()}<br/>{work.body.length >= 80 ? `${t("正文已采集")} · ${work.body.length} ${t("字")}` : t("列表摘要，需打开详情采集正文")}</div>{work.authorUrl ? <Anchor size="xs" onClick={() => setSession({...session,authorUrl:work.authorUrl})} component="button">{t("筛选此作者")}</Anchor> : null}</td>
        <td data-label={t("阅读／播放")} className={styles.number}>{metricText(work.metrics.views)}</td><td data-label={t("点赞")} className={styles.number}>{metricText(work.metrics.likes)}</td><td data-label={t("收藏")} className={styles.number}>{metricText(work.metrics.favorites)}</td><td data-label={t("评论")} className={styles.number}>{metricText(work.metrics.comments)}</td>
        <td data-label={t("分享")} className={styles.number}>{metricText(work.metrics.shares)}</td>
        <td><Stack gap={4}><Button size="xs" variant="default" onClick={() => start.mutate(work.url)}>{t("采集正文与数据")}</Button>{work.authorUrl ? <Button size="xs" variant="subtle" onClick={() => start.mutate(work.authorUrl)}>{t("查看并采集主页")}</Button> : null}</Stack></td>
      </tr>)}</tbody>
    </table> : <div className={styles.empty}><IconSearch size={32}/><Text fw={600} mt="md">{works.length ? t("当前筛选没有作品") : t("先采集第一组真实作品")}</Text><Text c="dimmed" size="sm" mt="xs">{t("输入关键词或公开链接，采集完成后在这里比较作品表现。")}</Text></div>}
  </main>;
}
