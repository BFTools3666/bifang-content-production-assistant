import { Alert, Button, Group, Modal, Stack, Text } from "@mantine/core";
import { IconAlertTriangle } from "@tabler/icons-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { ProviderConfigView } from "./workbenchTypes";

/** 删除确认弹窗所需的服务、忙碌状态与显式确认动作。 */
interface DeleteProviderModalProps {
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onConfirm: () => void;
  provider: ProviderConfigView | null;
}

/** 明确说明引用约束的 AI 服务删除确认，取消始终是默认安全动作。 */
export function DeleteProviderModal({
  busy,
  error,
  onCancel,
  onConfirm,
  provider,
}: DeleteProviderModalProps): ReactElement {
  const { t } = useTranslation("app");
  return (
    <Modal
      centered
      closeOnClickOutside={!busy}
      closeOnEscape={!busy}
      onClose={onCancel}
      opened={provider !== null}
      title={t("providers.delete.title")}
    >
      <Stack gap="md">
        <Text>
          {t("providers.delete.description", { name: provider?.name ?? "" })}
        </Text>
        <Alert
          color="yellow"
          icon={<IconAlertTriangle aria-hidden="true" size={18} />}
        >
          {t("providers.delete.constraint")}
        </Alert>
        {error ? (
          <Alert color="red" role="alert">
            {error}
          </Alert>
        ) : null}
        <Group justify="flex-end">
          <Button disabled={busy} onClick={onCancel} variant="default">
            {t("common.cancel")}
          </Button>
          <Button color="red" loading={busy} onClick={onConfirm}>
            {t("providers.delete.confirm")}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
