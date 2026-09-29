import {
  Badge,
  Button,
  Group,
  Paper,
  Progress,
  Stack,
  Text,
} from "@mantine/core";
import {
  IconPlayerPause,
  IconPlayerPlay,
  IconSquare,
} from "@tabler/icons-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { StatusBadge } from "./StatusBadge";
import type { RunRecord } from "./workbenchTypes";

/** 展示活动批次的持久进度，并将暂停、恢复和取消绑定到各自按钮。 */
export function BatchStatusStrip({
  run,
  busy,
  onCancel,
  onPause,
  onResume,
}: {
  run: RunRecord;
  busy: boolean;
  onCancel: () => void;
  onPause: () => void;
  onResume: () => void;
}): ReactElement {
  const { t } = useTranslation("app");
  const handled = Math.min(
    run.taskIds.length,
    run.completedCount + run.failedCount,
  );
  const percentage =
    run.taskIds.length === 0 ? 0 : (handled / run.taskIds.length) * 100;
  return (
    <Paper aria-live="polite" p="md" radius="lg" role="status" withBorder>
      <Group align="flex-end" justify="space-between" wrap="wrap">
        <Stack gap={6} style={{ flex: "1 1 360px" }}>
          <Group gap="xs">
            <Text fw={600}>{t("workbench.batch.title")}</Text>
            <StatusBadge scope="run" status={run.status} />
            {run.isTestFixture ? (
              <Badge color="violet" variant="light">
                {t("runs.fixture_badge")}
              </Badge>
            ) : null}
          </Group>
          <Progress
            aria-label={t("workbench.batch.progress_label")}
            value={percentage}
          />
          <Text c="dimmed" size="sm">
            {t("workbench.batch.progress", {
              completed: run.completedCount,
              failed: run.failedCount,
              total: run.taskIds.length,
            })}
          </Text>
        </Stack>
        <Group gap="xs">
          {run.status === "paused" ? (
            <Button
              disabled={busy}
              leftSection={<IconPlayerPlay aria-hidden="true" size={17} />}
              onClick={onResume}
              variant="default"
            >
              {t("workbench.actions.resume")}
            </Button>
          ) : (
            <Button
              disabled={busy}
              leftSection={<IconPlayerPause aria-hidden="true" size={17} />}
              onClick={onPause}
              variant="default"
            >
              {t("workbench.actions.pause")}
            </Button>
          )}
          <Button
            color="red"
            disabled={busy}
            leftSection={<IconSquare aria-hidden="true" size={16} />}
            onClick={onCancel}
            variant="light"
          >
            {t("workbench.actions.cancel_batch")}
          </Button>
        </Group>
      </Group>
    </Paper>
  );
}
