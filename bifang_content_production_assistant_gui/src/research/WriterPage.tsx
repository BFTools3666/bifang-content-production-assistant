import { Accordion, Alert, Anchor, Badge, Button, Group, MultiSelect, Select, Stack, Text, Textarea, TextInput } from "@mantine/core";
import { IconPencil, IconSparkles } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useAtom } from "jotai";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { listProviderConfigs } from "../workbench/workbenchCommands";
import { generateContent, researchError, updatePrompts } from "./researchCommands";
import { composerSessionAtom, writerDraftIdAtom, writerProviderIdAtom, IS_XHS_EDITION, platformOptions, RESEARCH_QUERY_KEY, useResearchLibrary } from "./researchState";
import type { ContentPlatform } from "./researchTypes";
import { DraftEditor } from "./DraftEditor";
import { ResearchHeader } from "./ResearchHeader";
import styles from "./ResearchPage.module.css";

/** 将关键词、公开参考、可切换风格和内置指令交给共享core生成原创内容。 */
export function WriterPage(): ReactElement {
  const { t } = useTranslation("research");
  const library = useResearchLibrary();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const [input,setInput] = useAtom(composerSessionAtom);
  const providers = useQuery({queryKey:["provider-configs"],queryFn:listProviderConfigs});
  const [providerId,setProviderId] = useAtom(writerProviderIdAtom);
  const [draftId,setDraftId] = useAtom(writerDraftIdAtom);
  const draft = library.data?.drafts.find(item => item.id === draftId);
  const [error,setError] = useState("");
  const [message,setMessage] = useState("");
  const textProviders = (providers.data || []).filter(provider => provider.enabled && provider.kind === "openAiCompatible" && provider.hasApiKey && provider.articleModel?.trim());
  const actualProvider = providerId || textProviders.find(provider => provider.defaultForArticle)?.id || textProviders[0]?.id || null;
  const works = (library.data?.works || []).filter(work => work.platform === input.platform);
  const selected = works.filter(work => input.sourceIds.includes(work.id));
  const sourceReady = input.sourceIds.length <= 10 && selected.length === input.sourceIds.length && selected.every(work => work.body.length >= 80);
  const availableStyles = (library.data?.styles || []).filter(style => style.platform === input.platform);
  const activeStyle = availableStyles.find(style => style.id === input.styleId);
  const builtInPrompt = input.platform === "xiaohongshu" ? library.data?.prompts.xhsRewrite : library.data?.prompts.seoRewrite;
  const generation = useMutation({
    mutationFn:() => generateContent(input, actualProvider || ""),
    onSuccess:result => {queryClient.setQueryData(RESEARCH_QUERY_KEY, {...library.data!,drafts:[result,...(library.data?.drafts || []).filter(item => item.id !== result.id)]});setDraftId(result.id);setError("");setMessage(t("标题与正文已生成并保存"));void queryClient.invalidateQueries({queryKey:RESEARCH_QUERY_KEY});},
    onError:failure => setError(researchError(failure)),
  });
  const savePrompt = useMutation({
    mutationFn:() => updatePrompts({...library.data!.prompts,...(input.platform === "xiaohongshu" ? {xhsRewrite:input.promptOverride || builtInPrompt!} : {seoRewrite:input.promptOverride || builtInPrompt!})}),
    onSuccess:() => {setMessage(t("内置二创提示词已保存，下次生成可以继续使用"));setError("");void queryClient.invalidateQueries({queryKey:RESEARCH_QUERY_KEY});},
    onError:failure => setError(researchError(failure)),
  });

  return <main className={styles.page}>
    <ResearchHeader title={IS_XHS_EDITION ? t("把选题写成一篇好笔记") : t("从对标资料到原创文章")} description={t("选择参考和风格，补充自己的材料。二次标题与正文一起生成，提示词可以随时修改。")} />
    {error || library.error ? <Alert color="red" role="alert" mb="md">{error || researchError(library.error)}</Alert> : null}
    {message ? <Text role="status" size="sm" c="brand" mb="md">{message}</Text> : null}
    <div className={styles.twoColumns}>
      <section><Stack gap="lg">
        <Group grow><Select label={t("目标平台")} data={platformOptions()} value={input.platform} allowDeselect={false} disabled={generation.isPending} onChange={value => setInput({...input,platform:value as ContentPlatform,sourceIds:[],styleId:null,promptOverride:""})}/><Select label={t("文本AI服务")} data={textProviders.map(provider => ({value:provider.id,label:provider.name}))} value={actualProvider} onChange={setProviderId} disabled={generation.isPending} placeholder={t("接入自己的API")}/></Group>
        {!actualProvider ? <Alert color="orange"><Text size="sm">{t("在原有AI服务页填写自己的API地址、模型和密钥。密钥只保存到系统凭据库。")}</Text><Button mt="sm" variant="default" onClick={() => void navigate({to:"/providers"})}>{t("配置自己的AI服务")}</Button></Alert> : null}
        <TextInput label={t("这次要写的关键词")} placeholder={t("明确一个读者问题或使用场景")} value={input.keyword} maxLength={200} disabled={generation.isPending} onChange={event => setInput({...input,keyword:event.currentTarget.value})} required/>
        <MultiSelect label={t("参考作品（最多10件）")} data={works.map(work => ({value:work.id,label:`${work.title}${work.body.length < 80 ? ` · ${t("正文未采集")}` : ""}`}))} value={input.sourceIds} onChange={sourceIds => setInput({...input,sourceIds})} searchable clearable disabled={generation.isPending} description={t("使用所选作品的内容重新构建文章，避免逐句改写。")}/>
        {!sourceReady ? <Alert color="orange">{t("所选作品需要已采集正文，且不能超过10件。请回到研究台打开详情采集。")}</Alert> : null}
        {selected.length ? <Stack gap={4}>{selected.map(work => <Anchor key={work.id} size="xs" href={work.url} target="_blank" rel="noreferrer">{work.title}</Anchor>)}</Stack> : null}
        <Select label={t("应用风格")} data={[{value:"",label:t("自然清晰，不套用风格")},...availableStyles.map(style => ({value:style.id,label:`${style.name} · ${style.author}`}))]} value={input.styleId || ""} onChange={value => setInput({...input,styleId:value || null})} disabled={generation.isPending} searchable description={t("每次可以一键切换已保存的对标风格。")}/>
        {activeStyle ? <Text size="xs" c="dimmed" lineClamp={4}>{activeStyle.instructions}</Text> : null}
        <Textarea label={t("自己的事实材料和写作要求")} placeholder={t("填写真实经历、步骤、案例、读者问题或产品信息；没有提供的个人经验不会编造。")} value={input.userMaterial} onChange={event => setInput({...input,userMaterial:event.currentTarget.value})} minRows={5} autosize maxRows={12} disabled={generation.isPending}/>
        <Accordion variant="default"><Accordion.Item value="prompt"><Accordion.Control>{t("查看和二次修改内置二创提示词")}</Accordion.Control><Accordion.Panel><Textarea label={t("本次二创指令")} value={input.promptOverride || builtInPrompt || ""} onChange={event => setInput({...input,promptOverride:event.currentTarget.value})} minRows={7} autosize maxRows={16} disabled={generation.isPending}/><Group mt="sm"><Button variant="default" loading={savePrompt.isPending} disabled={!library.data || generation.isPending} onClick={() => savePrompt.mutate()}>{t("保存为内置默认")}</Button><Button variant="subtle" disabled={generation.isPending} onClick={() => setInput({...input,promptOverride:""})}>{t("使用库内默认")}</Button></Group></Accordion.Panel></Accordion.Item></Accordion>
        <Button size="lg" leftSection={<IconSparkles size={18}/>} loading={generation.isPending} disabled={!library.data || !actualProvider || !input.keyword.trim() || !sourceReady || Boolean(input.styleId && !activeStyle)} onClick={() => {setError("");setMessage("");generation.mutate();}}>{generation.isPending ? t("正在生成并保存") : t("生成二次标题和正文")}</Button>
        <Text size="xs" c="dimmed">{t("调用你选择的文本模型。失败不会自动重复付费请求；生成内容需要你核对事实后使用。")}</Text>
      </Stack></section>
      {draft ? <DraftEditor draft={draft}/> : <section className={styles.paper}><Badge variant="outline">{t("文稿区")}</Badge><div className={styles.empty}><IconPencil size={38}/><Text fw={650} mt="lg">{t("让真实资料变成自己的表达")}</Text><Text c="dimmed" size="sm" mt="sm">{t("生成后这里会出现多个二次标题、完整正文和关键词。可以编辑、复制、导出，并在草稿记录中找回。")}</Text></div></section>}
    </div>
  </main>;
}
