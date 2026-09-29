import {
  Alert,
  PasswordInput,
  Select,
  Stack,
  Text,
  TextInput,
} from "@mantine/core";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { ProviderConfig, ProviderKind } from "./workbenchTypes";

/** 服务连接字段的纯展示与输入契约。 */
interface ProviderConnectionFieldsProps {
  actualImageEndpoint: string | null;
  apiKey: string;
  apiKeyDescription: string;
  apiKeyRequired: boolean;
  baseUrlError: string | undefined;
  disabled: boolean;
  draft: ProviderConfig;
  onApiKeyChange: (value: string) => void;
  onChange: (draft: ProviderConfig) => void;
  onImageProtocolChange?: (
    kind: Extract<ProviderKind, "openAiCompatible" | "volcengineArkImage">,
  ) => void;
}

/** 显示协议化的窄连接表单，密钥始终由父组件的短命状态持有。 */
export function ProviderConnectionFields({
  actualImageEndpoint,
  apiKey,
  apiKeyDescription,
  apiKeyRequired,
  baseUrlError,
  disabled,
  draft,
  onApiKeyChange,
  onChange,
  onImageProtocolChange,
}: ProviderConnectionFieldsProps): ReactElement {
  const { t } = useTranslation("app");
  const isArk = draft.kind === "volcengineArkImage";

  return (
    <Stack gap="md">
      {onImageProtocolChange ? (
        <Select
          allowDeselect={false}
          data={[
            {
              label: t("providers.form.protocol_ark"),
              value: "volcengineArkImage",
            },
            {
              label: t("providers.form.protocol_openai_image"),
              value: "openAiCompatible",
            },
          ]}
          disabled={disabled}
          label={t("providers.form.image_protocol")}
          onChange={(value) => {
            if (
              value === "openAiCompatible" ||
              value === "volcengineArkImage"
            ) {
              onImageProtocolChange(value);
            }
          }}
          required
          value={draft.kind}
        />
      ) : null}
      <TextInput
        disabled={disabled}
        label={t("providers.form.name")}
        onChange={(event) =>
          onChange({ ...draft, name: event.currentTarget.value })
        }
        required
        value={draft.name}
      />
      <TextInput
        description={isArk ? t("providers.form.ark_base_url_hint") : undefined}
        disabled={disabled}
        error={baseUrlError}
        label={t("providers.form.base_url")}
        onChange={(event) =>
          onChange({ ...draft, baseUrl: event.currentTarget.value })
        }
        placeholder={
          isArk
            ? "https://ark.cn-beijing.volces.com/api/v3"
            : "https://api.openai.com/v1"
        }
        required
        value={draft.baseUrl}
      />
      {isArk && actualImageEndpoint ? (
        <Text c="dimmed" size="xs">
          {t("providers.form.actual_image_endpoint", {
            endpoint: actualImageEndpoint,
          })}
        </Text>
      ) : null}
      <PasswordInput
        description={apiKeyDescription}
        disabled={disabled}
        label={t(
          isArk ? "providers.form.ark_api_key" : "providers.form.api_key",
        )}
        onChange={(event) => onApiKeyChange(event.currentTarget.value)}
        placeholder={t("providers.form.key_placeholder")}
        required={apiKeyRequired}
        value={apiKey}
      />
      <Alert color="blue">
        {t(
          isArk ? "providers.ark_external_notice" : "providers.external_notice",
        )}
      </Alert>
    </Stack>
  );
}
