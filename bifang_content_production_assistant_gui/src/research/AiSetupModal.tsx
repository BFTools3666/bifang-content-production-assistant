import {
  Accordion,
  Alert,
  Anchor,
  Button,
  Group,
  Modal,
  PasswordInput,
  Stack,
  Text,
  TextInput,
} from "@mantine/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  newDeepseekTextProviderDraft,
  saveProviderConfig,
} from "../workbench/workbenchCommands";
import type { ProviderConfig } from "../workbench/workbenchTypes";
import { researchError } from "./researchCommands";
import { IS_XHS_EDITION } from "./researchState";

/** 首次写作的快速接入向导，复用原保存命令和系统凭据库，不自动发起测试。 */
export function AiSetupModal({
  opened,
  onClose,
}: {
  opened: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation("research");
  const client = useQueryClient();
  const navigate = useNavigate();
  const defaults = useQuery({
    queryKey: ["deepseek-text-draft"],
    queryFn: newDeepseekTextProviderDraft,
    enabled: opened,
    retry: false,
  });
  const [config, setConfig] = useState<ProviderConfig | null>(null);
  const [key, setKey] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    if (opened && defaults.data) setConfig(defaults.data);
    if (!opened) {
      setKey("");
      setError("");
      setConfig(null);
    }
  }, [opened, defaults.data]);
  const save = useMutation({
    mutationFn: () => {
      if (!config) throw new Error(t("正在读取 AI 设置"));
      return saveProviderConfig(config, key);
    },
    onSuccess: (result) => {
      setKey("");
      setError("");
      client.setQueryData(["provider-configs"], result.providers);
      onClose();
    },
    onError: (failure) => setError(researchError(failure)),
  });
  return (
    <Modal
      opened={opened}
      onClose={() => {
        if (!save.isPending) onClose();
      }}
      title={t(
        IS_XHS_EDITION ? "接入 AI，生成笔记文字" : "接入 AI，生成技术文章",
      )}
      size="md"
      closeOnClickOutside={!save.isPending}
      closeOnEscape={!save.isPending}
      withCloseButton={!save.isPending}
    >
      <Stack>
        <Text size="sm">
          {t(
            IS_XHS_EDITION
              ? "这里接入文字 AI；制作配图时需要另行配置图片服务。"
              : "填入自己的 AI 服务后，就能生成 CSDN 或百家号的技术文章。",
          )}
        </Text>
        <Text fw={600}>DeepSeek</Text>
        <Text size="sm" c="dimmed">
          {t("接口地址和模型已填好，粘贴你的 API Key 即可。")}
        </Text>
        {error || defaults.error ? (
          <Alert color="red" role="alert">
            {error || researchError(defaults.error)}
          </Alert>
        ) : null}
        {defaults.isError ? (
          <Button variant="default" onClick={() => void defaults.refetch()}>
            {t("重新读取设置")}
          </Button>
        ) : null}
        <PasswordInput
          label={t("你的 API Key")}
          description={t(
            "密钥只保存在这台电脑的系统凭据库，导出的文件不会包含它。",
          )}
          value={key}
          onChange={(event) => setKey(event.currentTarget.value)}
          autoComplete="off"
          disabled={save.isPending}
        />
        <Accordion>
          <Accordion.Item value="guide">
            <Accordion.Control>
              {t("还没有 Key？查看获取方法")}
            </Accordion.Control>
            <Accordion.Panel>
              <Stack gap="xs">
                <Text size="sm">
                  {t("1. 打开服务商平台，注册并登录自己的账号。")}
                </Text>
                <Anchor
                  href="https://platform.deepseek.com/api_keys"
                  target="_blank"
                  rel="noreferrer"
                >
                  {t("打开 DeepSeek 获取 Key")}
                </Anchor>
                <Text size="sm">
                  {t("2. 按平台提示开通 API 并确认可用额度，再创建 Key。")}
                </Text>
                <Text size="sm">
                  {t(
                    "3. 复制 Key 回到这里保存。聊天会员和 API 额度由服务商分别管理。",
                  )}
                </Text>
              </Stack>
            </Accordion.Panel>
          </Accordion.Item>
          <Accordion.Item value="advanced">
            <Accordion.Control>{t("更多设置")}</Accordion.Control>
            <Accordion.Panel>
              {config ? (
                <Stack>
                  <TextInput
                    label={t("API 根地址")}
                    value={config.baseUrl}
                    onChange={(event) =>
                      setConfig({
                        ...config,
                        baseUrl: event.currentTarget.value,
                      })
                    }
                    disabled={save.isPending}
                  />
                  <TextInput
                    label={t("模型名称")}
                    value={config.articleModel}
                    onChange={(event) => {
                      const model = event.currentTarget.value;
                      setConfig({
                        ...config,
                        articleModel: model,
                        summaryModel: model,
                        textModel: model,
                      });
                    }}
                    disabled={save.isPending}
                  />
                  <Anchor
                    href="https://api-docs.deepseek.com/"
                    target="_blank"
                    rel="noreferrer"
                  >
                    {t("查看服务商当前接口说明")}
                  </Anchor>
                </Stack>
              ) : null}
            </Accordion.Panel>
          </Accordion.Item>
        </Accordion>
        <Group justify="space-between">
          <Button
            variant="subtle"
            disabled={save.isPending}
            onClick={() => {
              onClose();
              void navigate({ to: "/providers" });
            }}
          >
            {t("使用火山引擎或其他服务")}
          </Button>
          <Button
            loading={save.isPending}
            disabled={!config || !key.trim() || defaults.isError}
            onClick={() => save.mutate()}
          >
            {t("保存并返回写作")}
          </Button>
        </Group>
        <Text size="xs" c="dimmed">
          {t("保存不联网。生成或连接测试才会调用你的服务并可能消耗额度。")}
        </Text>
      </Stack>
    </Modal>
  );
}
