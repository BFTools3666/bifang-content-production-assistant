import {
  Alert,
  NumberInput,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  TextInput,
} from "@mantine/core";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { PromptTemplateKind, ProviderConfig } from "./workbenchTypes";

/** 当前工作流步骤参数编辑器的输入与变更契约。 */
interface ProviderStageFieldsProps {
  disabled: boolean;
  draft: ProviderConfig;
  onChange: (draft: ProviderConfig) => void;
  stage: PromptTemplateKind;
}

/** 把 NumberInput 的空值安全转换为可空整数。 */
function optionalInteger(value: string | number): number | null {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.round(value)
    : null;
}

/** 只展示和修改当前 SOP 步骤，调用方仍持有完整 Provider 草稿。 */
export function ProviderStageFields({
  disabled,
  draft,
  onChange,
  stage,
}: ProviderStageFieldsProps): ReactElement {
  const { t } = useTranslation("app");
  const stageLabel = t(`providers.stage_name.${stage}`);
  const model =
    stage === "article"
      ? draft.articleModel
      : stage === "summary"
        ? draft.summaryModel
        : draft.imageModel;

  const updateModel = (value: string): void => {
    if (stage === "article") {
      onChange({ ...draft, articleModel: value });
    } else if (stage === "summary") {
      onChange({ ...draft, summaryModel: value });
    } else {
      onChange({ ...draft, imageModel: value });
    }
  };

  if (stage === "image") {
    const isArk = draft.kind === "volcengineArkImage";
    const imageSizes: Array<{
      label: string;
      value: ProviderConfig["imageParameters"]["size"];
    }> = isArk
      ? [{ label: "2K", value: "2K" }]
      : [
          { label: "1024x1024", value: "1024x1024" },
          { label: "1536x1024", value: "1536x1024" },
          { label: "1024x1536", value: "1024x1536" },
        ];
    return (
      <Stack gap="sm">
        <TextInput
          description={t("providers.form.default_model_required")}
          disabled={disabled}
          label={t(
            isArk
              ? "providers.form.ark_endpoint_id"
              : "providers.form.image_model",
          )}
          onChange={(event) => updateModel(event.currentTarget.value)}
          placeholder={t(
            isArk
              ? "providers.form.ark_endpoint_id_placeholder"
              : "providers.form.model_placeholder",
          )}
          required
          value={model}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <Select
            allowDeselect={false}
            data={imageSizes}
            disabled={disabled}
            label={t("providers.form.image_size")}
            onChange={(value) => {
              const size = imageSizes.find(
                (candidate) => candidate.value === value,
              )?.value;
              if (!size) return;
              onChange({
                ...draft,
                imageParameters: { ...draft.imageParameters, size },
              });
            }}
            required
            value={draft.imageParameters.size}
          />
          <NumberInput
            allowDecimal={false}
            disabled={disabled}
            label={t("providers.form.timeout_seconds", { stage: stageLabel })}
            max={300}
            min={1}
            onChange={(value) => {
              const seconds = optionalInteger(value);
              if (seconds === null) return;
              onChange({
                ...draft,
                imageParameters: {
                  ...draft.imageParameters,
                  timeoutMs: seconds * 1_000,
                },
              });
            }}
            required
            value={draft.imageParameters.timeoutMs / 1_000}
          />
        </SimpleGrid>
        {isArk ? (
          <Switch
            checked={draft.imageParameters.watermark}
            disabled={disabled}
            label={t("providers.form.ark_watermark")}
            onChange={(event) =>
              onChange({
                ...draft,
                imageParameters: {
                  ...draft.imageParameters,
                  watermark: event.currentTarget.checked,
                },
              })
            }
          />
        ) : null}
        {isArk ? (
          <Alert color="blue">{t("providers.form.ark_contract_notice")}</Alert>
        ) : (
          <Text c="dimmed" size="xs">
            {t("providers.form.image_contract_notice")}
          </Text>
        )}
      </Stack>
    );
  }

  const parameters =
    stage === "article" ? draft.articleParameters : draft.summaryParameters;
  const updateParameters = (
    next: ProviderConfig["articleParameters"],
  ): void => {
    if (stage === "article") {
      onChange({ ...draft, articleParameters: next });
    } else {
      onChange({ ...draft, summaryParameters: next });
    }
  };

  return (
    <Stack gap="sm">
      <TextInput
        description={t("providers.form.default_model_required")}
        disabled={disabled}
        label={t(
          stage === "article"
            ? "providers.form.article_model"
            : "providers.form.summary_model",
        )}
        onChange={(event) => updateModel(event.currentTarget.value)}
        placeholder={t("providers.form.model_placeholder")}
        required
        value={model}
      />
      <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="sm">
        <NumberInput
          allowDecimal
          decimalScale={3}
          disabled={disabled}
          label={t("providers.form.temperature", { stage: stageLabel })}
          max={2}
          min={0}
          onChange={(value) =>
            updateParameters({
              ...parameters,
              temperatureMilli:
                typeof value === "number" ? Math.round(value * 1_000) : null,
            })
          }
          step={0.1}
          value={
            parameters.temperatureMilli === null
              ? ""
              : parameters.temperatureMilli / 1_000
          }
        />
        <NumberInput
          allowDecimal={false}
          disabled={disabled}
          label={t("providers.form.max_output_tokens", { stage: stageLabel })}
          max={128_000}
          min={1}
          onChange={(value) =>
            updateParameters({
              ...parameters,
              maxOutputTokens: optionalInteger(value),
            })
          }
          value={parameters.maxOutputTokens ?? ""}
        />
        <NumberInput
          allowDecimal={false}
          disabled={disabled}
          label={t("providers.form.timeout_seconds", { stage: stageLabel })}
          max={300}
          min={1}
          onChange={(value) => {
            const seconds = optionalInteger(value);
            if (seconds === null) return;
            updateParameters({ ...parameters, timeoutMs: seconds * 1_000 });
          }}
          required
          value={parameters.timeoutMs / 1_000}
        />
      </SimpleGrid>
    </Stack>
  );
}
