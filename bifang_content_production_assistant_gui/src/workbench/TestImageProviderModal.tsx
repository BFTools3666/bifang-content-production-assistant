import { Alert, Button, Group, Image, Modal, Stack, Text } from "@mantine/core";
import { IconAlertTriangle } from "@tabler/icons-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type {
  ProviderConfigView,
  ProviderImageTestResult,
} from "./workbenchTypes";

/** 真实图片连接测试确认所需的服务、状态与动作。 */
interface TestImageProviderModalProps {
  attempted: boolean;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onConfirm: () => void;
  provider: ProviderConfigView | null;
  result: ProviderImageTestResult | null;
}

/** 在可能产生一次图片费用前取得明确确认，并就地保留失败反馈。 */
export function TestImageProviderModal({
  attempted,
  busy,
  error,
  onCancel,
  onConfirm,
  provider,
  result,
}: TestImageProviderModalProps): ReactElement {
  const { i18n, t } = useTranslation("app");
  const byteFormatter = new Intl.NumberFormat(
    i18n.resolvedLanguage ?? i18n.language,
    { maximumFractionDigits: 1 },
  );
  return (
    <Modal
      centered
      closeOnClickOutside={!busy}
      closeOnEscape={!busy}
      onClose={onCancel}
      opened={provider !== null}
      title={t("providers.image_test.title")}
    >
      <Stack gap="md">
        <Text>
          {t("providers.image_test.description", {
            name: provider?.name ?? "",
          })}
        </Text>
        {provider ? (
          <Text c="dimmed" size="sm">
            {t(
              provider.kind === "volcengineArkImage"
                ? "providers.image_test.saved_settings_ark"
                : "providers.image_test.saved_settings",
              {
                model: provider.imageModel,
                size: provider.imageParameters.size,
                watermark: t(
                  provider.imageParameters.watermark
                    ? "providers.status.enabled"
                    : "providers.status.disabled",
                ),
              },
            )}
          </Text>
        ) : null}
        <Alert
          color="yellow"
          icon={<IconAlertTriangle aria-hidden="true" size={18} />}
        >
          {t("providers.image_test.cost_warning")}
        </Alert>
        {error ? (
          <Alert color="red" role="alert">
            {error}
          </Alert>
        ) : null}
        {result ? (
          <Stack gap="sm">
            <Alert color="green" role="status">
              {t("providers.image_test.success")}
            </Alert>
            <Image
              alt={t("providers.image_test.preview_alt", {
                name: provider?.name ?? "",
              })}
              fit="contain"
              mah={360}
              radius="md"
              src={result.imageDataUrl}
            />
            <Text c="dimmed" size="sm">
              {t("providers.image_test.result_details", {
                kilobytes: byteFormatter.format(result.byteLength / 1024),
                mimeType: result.mimeType,
              })}
            </Text>
          </Stack>
        ) : null}
        <Group justify="flex-end">
          <Button disabled={busy} onClick={onCancel} variant="default">
            {attempted && !busy
              ? t("providers.image_test.close")
              : t("common.cancel")}
          </Button>
          {!attempted || busy ? (
            <Button loading={busy} onClick={onConfirm}>
              {t("providers.image_test.confirm")}
            </Button>
          ) : null}
        </Group>
      </Stack>
    </Modal>
  );
}
