import {
  Alert,
  Badge,
  Button,
  Group,
  Paper,
  SimpleGrid,
  Stack,
  Text,
} from "@mantine/core";
import {
  IconPhoto,
  IconPlugConnected,
  IconSettings,
  IconUnlink,
} from "@tabler/icons-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { PromptTemplateKind, ProviderConfigView } from "./workbenchTypes";

/** 支持用户显式连接验证的三个 AI 生成步骤。 */
export type ProviderVerificationStage = PromptTemplateKind;

/** 把连接验证结果限定到具体服务版本和生成步骤。 */
export interface ProviderVerificationResult {
  error: string | null;
  feedback: string | null;
  providerId: string;
  providerRevision: number;
  stage: ProviderVerificationStage;
}

/** 当前工作流步骤内单张服务卡片的交互契约。 */
interface ProviderStageCardProps {
  busy: boolean;
  onConfigure: (provider: ProviderConfigView) => void;
  onRemove: (provider: ProviderConfigView) => void;
  onVerify: (
    provider: ProviderConfigView,
    stage: ProviderVerificationStage,
  ) => void;
  provider: ProviderConfigView;
  selected: boolean;
  stage: PromptTemplateKind;
  verificationResult: ProviderVerificationResult | null;
  verifying: {
    providerId: string;
    stage: ProviderVerificationStage;
  } | null;
}

/** 读取某个服务在当前 SOP 步骤中的模型与默认位。 */
function stageConfiguration(
  provider: ProviderConfigView,
  stage: PromptTemplateKind,
): { isDefault: boolean; model: string } {
  if (stage === "article") {
    return {
      isDefault: provider.defaultForArticle,
      model: provider.articleModel,
    };
  }
  if (stage === "summary") {
    return {
      isDefault: provider.defaultForSummary,
      model: provider.summaryModel,
    };
  }
  return { isDefault: provider.defaultForImage, model: provider.imageModel };
}

/** 在当前步骤内展示一个可复用服务，不混入其它步骤的模型参数。 */
export function ProviderStageCard({
  busy,
  onConfigure,
  onRemove,
  onVerify,
  provider,
  selected,
  stage,
  verificationResult,
  verifying,
}: ProviderStageCardProps): ReactElement {
  const { i18n, t } = useTranslation("app");
  const numberFormatter = new Intl.NumberFormat(
    i18n.resolvedLanguage ?? i18n.language,
  );
  const stageLabel = t(`providers.stage_name.${stage}`);
  const { isDefault, model } = stageConfiguration(provider, stage);
  const isArk = provider.kind === "volcengineArkImage";
  const matchingResult =
    verificationResult?.providerId === provider.id &&
    verificationResult.providerRevision === provider.revision &&
    verificationResult.stage === stage
      ? verificationResult
      : null;
  const parameterDefault = t("providers.parameter_default");
  const stageParameters =
    stage === "image"
      ? [
          t("providers.saved_image_size", {
            value: provider.imageParameters.size,
          }),
          ...(isArk
            ? [
                t("providers.saved_watermark", {
                  value: t(
                    provider.imageParameters.watermark
                      ? "providers.status.enabled"
                      : "providers.status.disabled",
                  ),
                }),
              ]
            : []),
          t("providers.saved_timeout_seconds", {
            value: numberFormatter.format(
              provider.imageParameters.timeoutMs / 1_000,
            ),
          }),
        ]
      : (() => {
          const parameters =
            stage === "article"
              ? provider.articleParameters
              : provider.summaryParameters;
          return [
            t("providers.saved_temperature", {
              value:
                parameters.temperatureMilli === null
                  ? parameterDefault
                  : numberFormatter.format(parameters.temperatureMilli / 1_000),
            }),
            t("providers.saved_max_output_tokens", {
              value:
                parameters.maxOutputTokens === null
                  ? parameterDefault
                  : numberFormatter.format(parameters.maxOutputTokens),
            }),
            t("providers.saved_timeout_seconds", {
              value: numberFormatter.format(parameters.timeoutMs / 1_000),
            }),
          ];
        })();

  return (
    <Paper
      aria-label={t("providers.stage_provider_named", {
        name: provider.name,
        stage: stageLabel,
      })}
      component="article"
      p="md"
      radius="lg"
      shadow={selected ? "xs" : undefined}
      withBorder
    >
      <Stack gap="sm">
        <Group justify="space-between" wrap="wrap">
          <Stack gap={2}>
            <Text fw={600}>{provider.name}</Text>
            <Text c="dimmed" size="xs">
              {provider.isTestFixture
                ? t("providers.fixture_description")
                : provider.baseUrl}
            </Text>
          </Stack>
          <Group gap={6}>
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
            {isDefault ? (
              <Badge color="blue">{t("providers.stage_active")}</Badge>
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
            ) : (
              <Badge color="violet">{t("providers.fixture_badge")}</Badge>
            )}
          </Group>
        </Group>
        <Stack gap={2}>
          <Text c="dimmed" size="xs">
            {isArk && stage === "image"
              ? t("providers.form.ark_endpoint_id")
              : t("providers.stage_model_label", { stage: stageLabel })}
          </Text>
          <Text fw={600} size="sm">
            {model.trim() || t("providers.stage_not_configured")}
          </Text>
        </Stack>
        {model.trim() ? (
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing={4}>
            {stageParameters.map((value) => (
              <Text c="dimmed" key={value} size="xs">
                {value}
              </Text>
            ))}
          </SimpleGrid>
        ) : null}
        <Group>
          {!provider.isTestFixture ? (
            <Button
              aria-label={t("providers.configure_stage_named", {
                name: provider.name,
                stage: stageLabel,
              })}
              disabled={busy}
              leftSection={<IconSettings aria-hidden="true" size={17} />}
              onClick={() => onConfigure(provider)}
              variant={selected ? "filled" : "default"}
            >
              {t("providers.configure_stage", { stage: stageLabel })}
            </Button>
          ) : null}
          {!provider.isTestFixture ? (
            <Button
              aria-label={t("providers.remove_from_stage_named", {
                name: provider.name,
                stage: stageLabel,
              })}
              color="red"
              disabled={busy}
              leftSection={<IconUnlink aria-hidden="true" size={17} />}
              onClick={() => onRemove(provider)}
              variant="subtle"
            >
              {t("providers.remove_from_stage", { stage: stageLabel })}
            </Button>
          ) : null}
          {(
            stage === "image"
              ? !provider.isTestFixture && model.trim().length > 0
              : provider.isTestFixture || model.trim().length > 0
          ) ? (
            <Button
              aria-label={t(
                stage === "image"
                  ? "providers.image_test.open_named"
                  : "providers.verify_named",
                {
                  name: provider.name,
                  stage: stageLabel,
                },
              )}
              disabled={!provider.enabled || busy}
              leftSection={
                stage === "image" ? (
                  <IconPhoto aria-hidden="true" size={17} />
                ) : (
                  <IconPlugConnected aria-hidden="true" size={17} />
                )
              }
              loading={
                verifying?.providerId === provider.id &&
                verifying.stage === stage
              }
              onClick={() => onVerify(provider, stage)}
              variant="default"
            >
              {stage === "image"
                ? t("providers.image_test.open")
                : t("providers.verify_stage", { stage: stageLabel })}
            </Button>
          ) : null}
        </Group>
        {matchingResult?.error ? (
          <Alert color="red" role="alert">
            <Stack gap={2}>
              <Text fw={600} size="sm">
                {t("providers.feedback.connection_failed", {
                  stage: stageLabel,
                })}
              </Text>
              <Text size="sm">{matchingResult.error}</Text>
            </Stack>
          </Alert>
        ) : null}
        {matchingResult?.feedback ? (
          <Alert color="green" role="status">
            {matchingResult.feedback}
          </Alert>
        ) : null}
      </Stack>
    </Paper>
  );
}
