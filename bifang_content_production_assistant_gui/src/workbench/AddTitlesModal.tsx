import {
  Alert,
  Button,
  Group,
  Modal,
  Stack,
  Text,
  Textarea,
} from "@mantine/core";
import { IconAlertCircle, IconPlus } from "@tabler/icons-react";
import { useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { AddTitlesResult } from "./workbenchTypes";

/** 批量标题录入弹窗，每行创建一条独立任务。 */
export function AddTitlesModal({
  opened,
  onClose,
  onSubmit,
}: {
  opened: boolean;
  onClose: () => void;
  onSubmit: (titles: string) => Promise<AddTitlesResult>;
}): ReactElement {
  const { t } = useTranslation("app");
  const [titles, setTitles] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    try {
      await onSubmit(titles);
      setTitles("");
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : t("common.failed"));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      centered
      onClose={onClose}
      opened={opened}
      title={t("workbench.add_modal.title")}
      transitionProps={{ duration: 0 }}
      withinPortal={false}
    >
      <Stack gap="md">
        <Text c="dimmed" size="sm">
          {t("workbench.add_modal.description")}
        </Text>
        <Textarea
          autosize
          data-autofocus
          label={t("workbench.add_modal.field_label")}
          maxRows={12}
          minRows={7}
          onChange={(event) => setTitles(event.currentTarget.value)}
          placeholder={t("workbench.add_modal.placeholder")}
          value={titles}
        />
        {error ? (
          <Alert
            color="red"
            icon={<IconAlertCircle aria-hidden="true" size={18} />}
            role="alert"
          >
            {error}
          </Alert>
        ) : null}
        <Group justify="flex-end">
          <Button onClick={onClose} variant="default">
            {t("common.cancel")}
          </Button>
          <Button
            disabled={titles.trim().length === 0}
            leftSection={<IconPlus aria-hidden="true" size={18} />}
            loading={submitting}
            onClick={() => void handleSubmit()}
          >
            {t("workbench.add_modal.submit")}
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
