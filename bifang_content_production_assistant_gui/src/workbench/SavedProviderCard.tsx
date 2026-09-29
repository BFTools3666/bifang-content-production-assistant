import {
  Badge,
  Button,
  Group,
  Paper,
  Stack,
  Switch,
  Text,
} from "@mantine/core";
import { IconKey, IconTrash } from "@tabler/icons-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { ProviderConfigView } from "./workbenchTypes";

/** 已保存 AI 服务卡片的展示数据与独立操作回调。 */
interface SavedProviderCardProps {
  busy: boolean;
  onDelete: (provider: ProviderConfigView) => void;
  onEdit: (provider: ProviderConfigView) => void;
  onEnabledChange: (provider: ProviderConfigView, enabled: boolean) => void;
  provider: ProviderConfigView;
}

/** 集中管理一个共享服务连接，不在这里混排三个步骤的模型参数。 */
export function SavedProviderCard({
  busy,
  onDelete,
  onEdit,
  onEnabledChange,
  provider,
}: SavedProviderCardProps): ReactElement {
  const { t } = useTranslation("app");
  const isArk = provider.kind === "volcengineArkImage";
  return (
    <Paper
      aria-label={t("providers.saved_provider_named", { name: provider.name })}
      component="article"
      p="md"
      radius="lg"
      withBorder
    >
      <Stack gap="sm">
        <Group justify="space-between" wrap="wrap">
          <Text fw={600}>{provider.name}</Text>
          <Group gap={6}>
            <Badge variant="outline">r{provider.revision}</Badge>
            {isArk ? (
              <>
                <Badge color="orange" variant="light">
                  {t("providers.ark_badge")}
                </Badge>
                <Badge variant="outline">
                  {t("providers.image_only_badge")}
                </Badge>
              </>
            ) : null}
            <Badge color={provider.enabled ? "green" : "gray"}>
              {t(
                provider.enabled
                  ? "providers.status.enabled"
                  : "providers.status.disabled",
              )}
            </Badge>
            {provider.isTestFixture ? (
              <Badge color="violet">{t("providers.fixture_badge")}</Badge>
            ) : null}
            {!provider.isTestFixture ? (
              <Badge
                color={provider.hasApiKey ? "green" : "yellow"}
                variant="light"
              >
                {provider.hasApiKey
                  ? t("providers.key_saved")
                  : t("providers.key_missing")}
              </Badge>
            ) : null}
          </Group>
        </Group>
        <Text c="dimmed" size="sm">
          {provider.isTestFixture
            ? t("providers.fixture_description")
            : provider.baseUrl}
        </Text>
        <Group gap="xs" wrap="wrap">
          {!provider.defaultForArticle &&
          !provider.defaultForSummary &&
          !provider.defaultForImage ? (
            <Badge color="gray" variant="light">
              {t("providers.not_used_by_any_stage")}
            </Badge>
          ) : null}
          {provider.defaultForArticle ? (
            <Badge variant="outline">{t("providers.used_by_article")}</Badge>
          ) : null}
          {provider.defaultForSummary ? (
            <Badge variant="outline">{t("providers.used_by_summary")}</Badge>
          ) : null}
          {provider.defaultForImage ? (
            <Badge variant="outline">{t("providers.used_by_image")}</Badge>
          ) : null}
        </Group>
        {!provider.isTestFixture ? (
          <Switch
            aria-label={t("providers.toggle_named", { name: provider.name })}
            checked={provider.enabled}
            disabled={busy}
            label={t("providers.enabled_switch")}
            onChange={(event) =>
              onEnabledChange(provider, event.currentTarget.checked)
            }
          />
        ) : null}
        <Group>
          {!provider.isTestFixture ? (
            <Button
              aria-label={t("providers.edit_named", { name: provider.name })}
              disabled={busy}
              leftSection={<IconKey aria-hidden="true" size={17} />}
              onClick={() => onEdit(provider)}
              variant="default"
            >
              {t("providers.edit_connection")}
            </Button>
          ) : null}
          {!provider.isTestFixture ? (
            <Button
              aria-label={t("providers.delete_named", { name: provider.name })}
              color="red"
              disabled={busy}
              leftSection={<IconTrash aria-hidden="true" size={17} />}
              onClick={() => onDelete(provider)}
              variant="light"
            >
              {t("providers.delete.action")}
            </Button>
          ) : null}
        </Group>
      </Stack>
    </Paper>
  );
}
