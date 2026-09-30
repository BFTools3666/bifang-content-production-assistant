import {
  Accordion,
  Alert,
  Badge,
  Button,
  Group,
  MultiSelect,
  Select,
  Stack,
  Text,
  Textarea,
  TextInput,
} from "@mantine/core";
import { IconCheck, IconList, IconSparkles } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useAtom } from "jotai";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { listProviderConfigs } from "../workbench/workbenchCommands";
import {
  generateContent,
  researchError,
  updatePrompts,
} from "./researchCommands";
import {
  composerSessionAtom,
  writerDraftIdAtom,
  writerProviderIdAtom,
  writerStepAtom,
  writerModeAtom,
  IS_XHS_EDITION,
  platformOptions,
  RESEARCH_QUERY_KEY,
  useResearchLibrary,
} from "./researchState";
import type { ContentPlatform } from "./researchTypes";
import { DraftEditor } from "./DraftEditor";
import { AiSetupModal } from "./AiSetupModal";
import styles from "./ResearchPage.module.css";

/** 用四个明确步骤呈现原有写作用例，未接入 AI 时先引导配置并阻止生成。 */
export function WriterPage(): ReactElement {
  const { t } = useTranslation("research");
  const library = useResearchLibrary();
  const client = useQueryClient();
  const navigate = useNavigate();
  const [input, setInput] = useAtom(composerSessionAtom);
  const [step, setStep] = useAtom(writerStepAtom);
  const [mode, setMode] = useAtom(writerModeAtom);
  const [providerId, setProviderId] = useAtom(writerProviderIdAtom);
  const [draftId, setDraftId] = useAtom(writerDraftIdAtom);
  const providers = useQuery({
    queryKey: ["provider-configs"],
    queryFn: listProviderConfigs,
    retry: false,
  });
  const [setup, setSetup] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const isNote = input.platform === "xiaohongshu";
  const draft = library.data?.drafts.find((item) => item.id === draftId);
  const textProviders = (providers.data ?? []).filter(
    (item) =>
      item.enabled &&
      item.kind === "openAiCompatible" &&
      item.hasApiKey &&
      item.articleModel?.trim(),
  );
  const provider =
    textProviders.find((item) => item.id === providerId) ??
    textProviders.find((item) => item.defaultForArticle) ??
    textProviders[0];
  const works = (library.data?.works ?? []).filter(
    (item) => item.platform === input.platform,
  );
  const selected = works.filter((item) => input.sourceIds.includes(item.id));
  const sourceReady =
    input.sourceIds.length <= 10 &&
    selected.length === input.sourceIds.length &&
    selected.every((item) => item.body.length >= 80);
  const availableStyles = (library.data?.styles ?? []).filter(
    (item) => item.platform === input.platform,
  );
  const styleReady =
    !input.styleId || availableStyles.some((item) => item.id === input.styleId);
  const builtInPrompt =
    input.platform === "xiaohongshu"
      ? library.data?.prompts.xhsRewrite
      : library.data?.prompts.seoRewrite;
  const generation = useMutation({
    mutationFn: () => generateContent(input, provider?.id ?? ""),
    onSuccess: (result) => {
      setDraftId(result.id);
      setError("");
      setMessage(t(isNote ? "笔记文字已生成并保存" : "技术文章已生成并保存"));
      void client.invalidateQueries({ queryKey: RESEARCH_QUERY_KEY });
    },
    onError: (failure) => setError(researchError(failure)),
  });
  const savePrompt = useMutation({
    mutationFn: () => {
      if (!library.data) throw new Error(t("正在读取写作资料"));
      const prompt = input.promptOverride || builtInPrompt || "";
      return updatePrompts({
        ...library.data.prompts,
        ...(input.platform === "xiaohongshu"
          ? { xhsRewrite: prompt }
          : { seoRewrite: prompt }),
      });
    },
    onSuccess: () => {
      setMessage(t("内置二创提示词已保存，下次生成可以继续使用"));
      void client.invalidateQueries({ queryKey: RESEARCH_QUERY_KEY });
    },
    onError: (failure) => setError(researchError(failure)),
  });
  const ready =
    library.isSuccess &&
    providers.isSuccess &&
    Boolean(provider) &&
    Boolean(input.keyword.trim()) &&
    sourceReady &&
    styleReady &&
    (mode !== "reference" || input.sourceIds.length > 0) &&
    (mode !== "style" || Boolean(input.styleId));
  const steps = [
    isNote ? "选主题和参考" : "选平台和主题",
    isNote ? "生成笔记文字" : "生成技术文章",
    isNote ? "检查笔记文字" : "检查标题和正文",
    isNote ? "导出文字或制作配图" : "导出或继续制作",
  ];
  return (
    <main className={styles.page}>
      <Group justify="space-between" align="flex-start" mb="lg">
        <div>
          <h1 className={styles.title}>{t("写一篇文章")}</h1>
          <Text c="dimmed" size="sm">
            {t(
              isNote
                ? "为小红书制作图文笔记：先生成笔记文字，核对后继续做配图。"
                : "为 CSDN 或百家号生产技术类长文章：定主题、选参考、核对后再导出。",
            )}
          </Text>
        </div>
        <Button
          variant="default"
          leftSection={<IconList size={17} />}
          disabled={generation.isPending}
          onClick={() => void navigate({ to: "/batch" })}
        >
          {t(IS_XHS_EDITION ? "图文制作工作台" : "批量文章工作台")}
        </Button>
      </Group>
      <div className={styles.writingSteps} aria-label={t("写作步骤")}>
        {steps.map((label, index) => (
          <Button
            key={label}
            variant={step === index ? "light" : "subtle"}
            color={step === index ? "brand" : "gray"}
            disabled={
              generation.isPending ||
              (index > 1 && !draft) ||
              (index === 1 && !ready)
            }
            onClick={() => setStep(index as 0 | 1 | 2 | 3)}
            aria-current={step === index ? "step" : undefined}
          >
            {index + 1}. {t(label)}
          </Button>
        ))}
      </div>
      {error || library.error ? (
        <Alert color="red" role="alert" my="md">
          {error || researchError(library.error)}
          {library.isError ? (
            <Button
              mt="sm"
              variant="default"
              onClick={() => void library.refetch()}
            >
              {t("重新读取资料")}
            </Button>
          ) : null}
        </Alert>
      ) : null}
      {message ? (
        <Text role="status" size="sm" c="brand" my="md">
          {message}
        </Text>
      ) : null}
      {providers.isPending ? (
        <Text role="status" mb="md">
          {t("正在读取 AI 设置")}
        </Text>
      ) : providers.isError ? (
        <Alert color="red" role="alert" mb="md">
          {t("AI 设置读取失败，请重试。原有配置不会被清空。")}
          <Button
            mt="sm"
            variant="default"
            onClick={() => void providers.refetch()}
          >
            {t("重新读取设置")}
          </Button>
        </Alert>
      ) : !provider ? (
        <Alert
          color="blue"
          title={t(
            isNote
              ? "先接入 AI，才能生成笔记文字"
              : "先接入 AI，才能生成技术文章",
          )}
          mb="lg"
        >
          <Text size="sm">
            {t(
              "填入自己的 API Key，设置一次后就能直接使用；还没有 Key 可查看申请步骤。",
            )}
          </Text>
          <Button mt="sm" onClick={() => setSetup(true)}>
            {t("接入 AI")}
          </Button>
        </Alert>
      ) : null}
      {step === 0 ? (
        <section className={styles.prepareSurface}>
          <Stack gap="lg">
            <Group justify="space-between" align="flex-start">
              <div>
                <Text fw={650} size="lg">
                  {t("这次用哪种方式生成？")}
                </Text>
                <Text c="dimmed" size="sm" mt="xs">
                  {t(
                    "第一次使用，选“从主题生成”即可；有对标作品或保存过风格时再切换。",
                  )}
                </Text>
              </div>
              {IS_XHS_EDITION ? (
                <Badge variant="light">{t("小红书图文笔记")}</Badge>
              ) : (
                <Select
                  label={t("计划发布的平台")}
                  data={platformOptions()}
                  value={input.platform}
                  allowDeselect={false}
                  onChange={(value) => {
                    if (value)
                      setInput({
                        ...input,
                        platform: value as ContentPlatform,
                        sourceIds: [],
                        styleId: null,
                        promptOverride: "",
                      });
                  }}
                />
              )}
            </Group>
            <div className={styles.modeChoices}>
              {(
                [
                  [
                    "topic",
                    "从主题生成",
                    isNote
                      ? "输入主题和自己的资料，生成笔记文字。"
                      : "输入主题和自己的资料，生成技术文章。",
                  ],
                  [
                    "reference",
                    "参考对标作品生成",
                    "选择已采集正文的对标作品，结合自己的资料重新组织。",
                  ],
                  [
                    "style",
                    "使用风格库生成",
                    "选择已保存的风格，再输入这次的新主题。",
                  ],
                ] as const
              ).map(([value, title, note]) => (
                <button
                  type="button"
                  key={value}
                  aria-pressed={mode === value}
                  className={styles.modeChoice}
                  onClick={() => {
                    setMode(value);
                    setInput({
                      ...input,
                      sourceIds: value === "reference" ? input.sourceIds : [],
                      styleId: value === "style" ? input.styleId : null,
                    });
                  }}
                >
                  <Group justify="space-between">
                    <Text fw={600}>{t(title)}</Text>
                    {mode === value ? <IconCheck size={16} /> : null}
                  </Group>
                  <Text size="sm" mt="xs">
                    {t(note)}
                  </Text>
                </button>
              ))}
            </div>
            <TextInput
              label={t(isNote ? "这篇图文笔记的主题" : "这篇技术文章的主题")}
              placeholder={t("输入主题、关键词，或读者想解决的问题")}
              value={input.keyword}
              maxLength={200}
              onChange={(event) =>
                setInput({ ...input, keyword: event.currentTarget.value })
              }
              required
            />
            {mode === "reference" ? (
              <Stack gap="xs">
                <MultiSelect
                  label={t("参考作品（最多10件）")}
                  data={works.map((item) => ({
                    value: item.id,
                    label: item.title,
                  }))}
                  value={input.sourceIds}
                  onChange={(sourceIds) => setInput({ ...input, sourceIds })}
                  searchable
                  clearable
                />
                <Button
                  variant="subtle"
                  onClick={() => void navigate({ to: "/benchmarks" })}
                >
                  {t("去找对标")}
                </Button>
                {!sourceReady ? (
                  <Alert color="orange">
                    {t(
                      "所选作品需要已采集正文，且不能超过10件。请回到研究台打开详情采集。",
                    )}
                  </Alert>
                ) : null}
              </Stack>
            ) : null}
            {mode === "style" ? (
              <Stack gap="xs">
                <Select
                  label={t("应用风格")}
                  data={availableStyles.map((item) => ({
                    value: item.id,
                    label: item.name,
                  }))}
                  value={input.styleId}
                  onChange={(styleId) => setInput({ ...input, styleId })}
                  searchable
                />
                <Button
                  variant="subtle"
                  onClick={() => void navigate({ to: "/styles" })}
                >
                  {t("去风格库")}
                </Button>
              </Stack>
            ) : null}
            <Textarea
              label={t("你自己的资料（选填）")}
              description={t(
                "填写真实步骤、案例、产品信息或观点，生成后仍需核对事实。",
              )}
              value={input.userMaterial}
              onChange={(event) =>
                setInput({ ...input, userMaterial: event.currentTarget.value })
              }
              autosize
              minRows={4}
              maxRows={12}
            />
            <Group justify="space-between">
              <Text size="sm" c="dimmed">
                {provider
                  ? `${provider.name} · ${provider.articleModel}`
                  : t("尚未配置 AI")}
              </Text>
              <Button
                disabled={!ready}
                onClick={() => {
                  setDraftId(null);
                  setMessage("");
                  setError("");
                  setStep(1);
                }}
              >
                {t(isNote ? "下一步：生成笔记文字" : "下一步：生成技术文章")}
              </Button>
            </Group>
          </Stack>
        </section>
      ) : step === 1 ? (
        <Stack gap="lg">
          <Group justify="space-between">
            <Text fw={650}>{input.keyword}</Text>
            <Button
              variant="subtle"
              disabled={generation.isPending}
              onClick={() => setStep(0)}
            >
              {t("返回修改主题和参考")}
            </Button>
          </Group>
          <Group align="flex-end">
            <Select
              label={t("使用的 AI 服务")}
              data={textProviders.map((item) => ({
                value: item.id,
                label: item.name,
              }))}
              value={provider?.id ?? null}
              onChange={setProviderId}
              disabled={generation.isPending}
            />
            <Button
              variant="subtle"
              disabled={generation.isPending}
              onClick={() => void navigate({ to: "/providers" })}
            >
              {t("管理 AI 服务")}
            </Button>
          </Group>
          <Text size="xs" c="dimmed">
            {provider
              ? `${t("模型")}：${provider.articleModel}`
              : t("尚未配置 AI")}
          </Text>
          <Accordion>
            <Accordion.Item value="prompt">
              <Accordion.Control>
                {t("查看或修改生成要求（选填）")}
              </Accordion.Control>
              <Accordion.Panel>
                <Textarea
                  label={t("这次给 AI 的要求")}
                  value={input.promptOverride || builtInPrompt || ""}
                  onChange={(event) =>
                    setInput({
                      ...input,
                      promptOverride: event.currentTarget.value,
                    })
                  }
                  minRows={6}
                  autosize
                  maxRows={16}
                  disabled={generation.isPending}
                />
                <Group mt="sm">
                  <Button
                    variant="default"
                    loading={savePrompt.isPending}
                    disabled={!library.data || generation.isPending}
                    onClick={() => savePrompt.mutate()}
                  >
                    {t("保存为默认要求")}
                  </Button>
                  <Button
                    variant="subtle"
                    disabled={generation.isPending}
                    onClick={() => setInput({ ...input, promptOverride: "" })}
                  >
                    {t("恢复默认要求")}
                  </Button>
                </Group>
              </Accordion.Panel>
            </Accordion.Item>
          </Accordion>
          <Group>
            <Button
              leftSection={<IconSparkles size={18} />}
              loading={generation.isPending}
              disabled={!ready}
              onClick={() => {
                setError("");
                setMessage("");
                generation.mutate();
              }}
            >
              {generation.isPending
                ? t("正在生成并保存")
                : t(isNote ? "生成笔记文字" : "生成技术文章")}
            </Button>
            {draft ? (
              <Button
                variant="default"
                disabled={generation.isPending}
                onClick={() => setStep(2)}
              >
                {t("检查修改")}
              </Button>
            ) : null}
          </Group>
          <Text size="xs" c="dimmed">
            {t(
              "生成会消耗所选服务的额度；失败不会自动重复调用，你的材料会保留。",
            )}
          </Text>
          {draft ? (
            <section className={styles.paper}>
              <Badge>{t("已保存原创草稿")}</Badge>
              <Text fw={650} size="lg" mt="md">
                {draft.title}
              </Text>
              <Text mt="sm" style={{ whiteSpace: "pre-wrap" }} lineClamp={12}>
                {draft.body}
              </Text>
            </section>
          ) : null}
        </Stack>
      ) : draft ? (
        <DraftEditor
          draft={draft}
          mode={step === 2 ? "review" : "export"}
          onSaved={() => setStep(3)}
        />
      ) : (
        <Alert>
          {t(
            isNote
              ? "先生成笔记文字，再检查，随后制作简介和配图。"
              : "先生成文章，再检查和导出。",
          )}
        </Alert>
      )}
      <AiSetupModal opened={setup} onClose={() => setSetup(false)} />
    </main>
  );
}
