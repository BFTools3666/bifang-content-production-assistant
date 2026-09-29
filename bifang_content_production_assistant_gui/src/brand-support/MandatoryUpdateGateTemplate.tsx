import { Badge, Button, Group, Paper, Stack, Text, Title } from "@mantine/core";
import type { ReactElement, ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { formatDisplayVersion } from "./displayVersion";
import {
  requiresMandatoryUpdate,
  type UpdatePresentation,
} from "./updatePresentation";

/** 强更阻断页只允许打开可信下载页或完整退出应用。 */
export function MandatoryUpdateGateTemplate({
  update,
  children,
  onOpenUpdateDownload,
  onExitApplication,
}: {
  update: UpdatePresentation;
  children: ReactNode;
  onOpenUpdateDownload: () => void;
  onExitApplication: () => void;
}): ReactElement {
  const { t } = useTranslation("brandSupport");

  if (!requiresMandatoryUpdate(update)) return <>{children}</>;

  return (
    <Paper
      aria-modal="true"
      data-testid="mandatory-update-gate"
      maw={640}
      mx="auto"
      my="xl"
      p="xl"
      radius="lg"
      role="alertdialog"
      shadow="md"
      withBorder
    >
      <Stack gap="lg">
        <Stack gap="xs">
          <Badge color="red" variant="light">
            {t("updater.required_badge")}
          </Badge>
          <Title order={2}>{t("updater.required_title")}</Title>
          <Text>{t("updater.required_description")}</Text>
          {update.title ? <Text fw={600}>{update.title}</Text> : null}
          {update.message ? <Text>{update.message}</Text> : null}
          <Text c="dimmed" size="sm">
            {t("updater.current_version", {
              version: formatDisplayVersion(update.currentVersion),
            })}
          </Text>
        </Stack>
        <Group justify="flex-end">
          <Button onClick={onExitApplication} variant="default">
            {t("updater.exit_application")}
          </Button>
          <Button
            disabled={!update.downloadReady}
            onClick={onOpenUpdateDownload}
          >
            {t("updater.open_download")}
          </Button>
        </Group>
      </Stack>
    </Paper>
  );
}
