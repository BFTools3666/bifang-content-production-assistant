import {
  Alert,
  Button,
  Checkbox,
  Modal,
  ScrollArea,
  Select,
  Stack,
  Text,
  TextInput,
} from "@mantine/core";
import { IconSparkles } from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { AiSetupModal } from "../research/AiSetupModal";
import {
  generateArticleTitles,
  researchError,
} from "../research/researchCommands";
import { platformOptions } from "../research/researchState";
import type { ContentPlatform } from "../research/researchTypes";
import { listProviderConfigs } from "./workbenchCommands";
import type { AddTitlesResult } from "./workbenchTypes";

/** 从主题生成可勾选的标题，再由既有用例加入批量工作台。 */
export function GenerateTitlesModal({
  opened,
  onClose,
  onSubmit,
}: {
  opened: boolean;
  onClose: () => void;
  onSubmit: (titles: string) => Promise<AddTitlesResult>;
}): ReactElement {
  const { t } = useTranslation("app");
  const [topic, setTopic] = useState("");
  const [platform, setPlatform] = useState<ContentPlatform>("baijiahao");
  const [titles, setTitles] = useState<string[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [resultTopic, setResultTopic] = useState("");
  const [error, setError] = useState("");
  const [adding, setAdding] = useState(false);
  const [setup, setSetup] = useState(false);
  const addLock = useRef(false);
  const providers = useQuery({
    queryKey: ["provider-configs"],
    queryFn: listProviderConfigs,
    enabled: opened,
    retry: false,
  });
  const available = (providers.data ?? []).filter(
    (item) =>
      item.enabled &&
      item.kind === "openAiCompatible" &&
      item.hasApiKey &&
      item.articleModel?.trim(),
  );
  const provider =
    available.find((item) => item.defaultForArticle) ?? available[0];
  const generation = useMutation({
    mutationFn: () =>
      generateArticleTitles(
        { platform, topic, requirements: "", count: 5 },
        provider?.id ?? "",
      ),
    retry: false,
    onSuccess: (result) => {
      setTitles(result);
      setSelected(result);
      setResultTopic(topic);
      setError("");
    },
    onError: (reason) => setError(researchError(reason)),
  });
  const busy = generation.isPending || adding;

  /** 一次点击只提交选中的真实候选，失败时保留选择与材料。 */
  const addSelected = async (): Promise<void> => {
    if (addLock.current || selected.length === 0) return;
    addLock.current = true;
    setAdding(true);
    setError("");
    try {
      await onSubmit(selected.join("\n"));
      setTitles([]);
      setSelected([]);
      onClose();
    } catch (reason) {
      setError(researchError(reason));
    } finally {
      addLock.current = false;
      setAdding(false);
    }
  };

  return (
    <>
      <Modal
        opened={opened}
        onClose={onClose}
        title={t("title_generation.title")}
        size="lg"
        centered
        closeOnEscape={!busy}
        closeOnClickOutside={!busy}
        withCloseButton={!busy}
        closeButtonProps={{ "aria-label": t("common.close") }}
        transitionProps={{ duration: 0 }}
      >
        <Stack gap="md">
          <Text c="dimmed" size="sm">
            {t("title_generation.description")}
          </Text>
          {providers.isError ? (
            <Alert color="red">
              {t("title_generation.config_failed")}
              <Button variant="subtle" onClick={() => void providers.refetch()}>
                {t("common.retry")}
              </Button>
            </Alert>
          ) : providers.isPending ? (
            <Text role="status">{t("title_generation.config_loading")}</Text>
          ) : !provider ? (
            <Alert color="blue">
              {t("title_generation.setup_hint")}
              <Button mt="sm" onClick={() => setSetup(true)}>
                {t("title_generation.setup")}
              </Button>
            </Alert>
          ) : null}
          <Select
            label={t("title_generation.platform")}
            data={platformOptions()}
            value={platform}
            allowDeselect={false}
            disabled={busy}
            onChange={(value) => {
              if (value) setPlatform(value as ContentPlatform);
            }}
          />
          <TextInput
            label={t("title_generation.topic")}
            placeholder={t("title_generation.topic_placeholder")}
            value={topic}
            required
            maxLength={200}
            disabled={busy}
            onChange={(event) => setTopic(event.currentTarget.value)}
          />
          {error ? (
            <Alert color="red" role="alert">
              {error}
            </Alert>
          ) : null}
          <Button
            leftSection={<IconSparkles aria-hidden="true" size={18} />}
            loading={generation.isPending}
            disabled={
              !providers.isSuccess || !provider || !topic.trim() || busy
            }
            onClick={() => generation.mutate()}
          >
            {t(
              titles.length
                ? "title_generation.regenerate"
                : "title_generation.generate",
            )}
          </Button>
          {titles.length > 0 ? (
            <Stack gap="sm">
              <Text fw={600} role="status">
                {t("title_generation.results", { topic: resultTopic })}
              </Text>
              <Checkbox
                label={t("title_generation.select_all")}
                checked={selected.length === titles.length}
                indeterminate={
                  selected.length > 0 && selected.length < titles.length
                }
                disabled={busy}
                onChange={(event) =>
                  setSelected(event.currentTarget.checked ? [...titles] : [])
                }
              />
              <ScrollArea.Autosize mah={280} type="auto">
                <Stack gap="sm">
                  {titles.map((title) => (
                    <Checkbox
                      key={title}
                      label={title}
                      checked={selected.includes(title)}
                      disabled={busy}
                      onChange={(event) => {
                        const checked = event.currentTarget.checked;
                        setSelected((current) =>
                          checked
                            ? [...current, title]
                            : current.filter((item) => item !== title),
                        );
                      }}
                    />
                  ))}
                </Stack>
              </ScrollArea.Autosize>
              <Button
                loading={adding}
                disabled={selected.length === 0 || busy}
                onClick={() => void addSelected()}
              >
                {t("title_generation.add", { count: selected.length })}
              </Button>
            </Stack>
          ) : null}
        </Stack>
      </Modal>
      {setup ? <AiSetupModal opened onClose={() => setSetup(false)} /> : null}
    </>
  );
}
