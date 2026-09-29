import { SimpleGrid, Stack, Text } from "@mantine/core";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { ProviderConfigView } from "./workbenchTypes";
import classes from "./SavedProviderConfiguration.module.css";

/** 已保存配置摘要只接收后端返回的脱敏 Provider 视图。 */
interface SavedProviderConfigurationProps {
  provider: ProviderConfigView;
}

/** 单项参数使用稳定键渲染，避免相同文案导致列表身份漂移。 */
interface SavedStageValue {
  key: string;
  text: string;
}

/** 单个生成阶段包含模型与该阶段真实存在的非秘密参数。 */
interface SavedStage {
  key: string;
  label: string;
  model: string;
  values: SavedStageValue[];
}

/** 只读取已保存 Provider 视图，展示三个阶段的冻结非秘密配置。 */
export function SavedProviderConfiguration({
  provider,
}: SavedProviderConfigurationProps): ReactElement {
  const { i18n, t } = useTranslation("app");
  const numberFormatter = new Intl.NumberFormat(
    i18n.resolvedLanguage ?? i18n.language,
    { maximumFractionDigits: 3 },
  );
  const formatOptional = (value: number | null, divisor = 1): string =>
    value === null
      ? t("providers.parameter_default")
      : numberFormatter.format(value / divisor);
  const formatNumber = (value: number, divisor = 1): string =>
    numberFormatter.format(value / divisor);
  const stages: SavedStage[] = [
    {
      key: "article",
      label: t("providers.form.article_stage"),
      model: provider.articleModel,
      values: [
        {
          key: "temperature",
          text: t("providers.saved_temperature", {
            value: formatOptional(
              provider.articleParameters.temperatureMilli,
              1_000,
            ),
          }),
        },
        {
          key: "max-output-tokens",
          text: t("providers.saved_max_output_tokens", {
            value: formatOptional(provider.articleParameters.maxOutputTokens),
          }),
        },
        {
          key: "timeout",
          text: t("providers.saved_timeout_seconds", {
            value: formatNumber(provider.articleParameters.timeoutMs, 1_000),
          }),
        },
      ],
    },
    {
      key: "summary",
      label: t("providers.form.summary_stage"),
      model: provider.summaryModel,
      values: [
        {
          key: "temperature",
          text: t("providers.saved_temperature", {
            value: formatOptional(
              provider.summaryParameters.temperatureMilli,
              1_000,
            ),
          }),
        },
        {
          key: "max-output-tokens",
          text: t("providers.saved_max_output_tokens", {
            value: formatOptional(provider.summaryParameters.maxOutputTokens),
          }),
        },
        {
          key: "timeout",
          text: t("providers.saved_timeout_seconds", {
            value: formatNumber(provider.summaryParameters.timeoutMs, 1_000),
          }),
        },
      ],
    },
    {
      key: "image",
      label: t("providers.form.image_stage"),
      model: provider.imageModel,
      values: [
        {
          key: "size",
          text: t("providers.saved_image_size", {
            value: provider.imageParameters.size,
          }),
        },
        {
          key: "timeout",
          text: t("providers.saved_timeout_seconds", {
            value: formatNumber(provider.imageParameters.timeoutMs, 1_000),
          }),
        },
      ],
    },
  ];

  return (
    <Stack
      aria-label={t("providers.saved_configuration_named", {
        name: provider.name,
      })}
      gap="md"
      role="group"
    >
      <Text c="dimmed" fw={600} size="xs">
        {t("providers.saved_configuration")}
      </Text>
      {stages.map((stage) => (
        <Stack
          aria-label={t("providers.saved_stage_named", {
            stage: stage.label,
          })}
          className={classes.stage}
          gap="xs"
          key={stage.key}
          role="group"
        >
          <Text fw={600} size="sm">
            {stage.label}
          </Text>
          <Text className={classes.value} size="sm">
            {t("providers.saved_model", {
              value: stage.model.trim() || t("providers.stage_not_configured"),
            })}
          </Text>
          <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="xs">
            {stage.values.map((value) => (
              <Text
                c="dimmed"
                className={classes.value}
                key={value.key}
                size="xs"
              >
                {value.text}
              </Text>
            ))}
          </SimpleGrid>
        </Stack>
      ))}
    </Stack>
  );
}
