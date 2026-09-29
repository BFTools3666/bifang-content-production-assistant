import {
  Alert,
  Badge,
  Button,
  Group,
  Loader,
  Paper,
  Progress,
  Stack,
  Table,
  Text,
  Title,
} from "@mantine/core";
import { IconAlertCircle, IconRefresh } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { StatusBadge } from "./StatusBadge";
import { listRunHistory } from "./workbenchCommands";

/** 展示最近批量运行及其已完成、失败和总任务数。 */
export function RunHistoryPage(): ReactElement {
  const { t, i18n } = useTranslation("app");
  const runsQuery = useQuery({
    queryFn: listRunHistory,
    queryKey: ["run-history"],
    refetchInterval: (query) =>
      query.state.data?.some(
        (run) =>
          run.status === "queued" ||
          run.status === "running" ||
          run.status === "paused",
      )
        ? 500
        : false,
  });

  if (runsQuery.isPending) {
    return (
      <Stack align="center" gap="sm" mih={280} justify="center" role="status">
        <Loader aria-label={t("runs.loading")} size="sm" />
        <Text c="dimmed">{t("runs.loading")}</Text>
      </Stack>
    );
  }

  if (runsQuery.isError) {
    return (
      <Alert
        color="red"
        icon={<IconAlertCircle aria-hidden="true" size={20} />}
        role="alert"
      >
        <Stack align="flex-start" gap="sm">
          <Text>{t("runs.load_failed")}</Text>
          <Button
            leftSection={<IconRefresh aria-hidden="true" size={17} />}
            onClick={() => void runsQuery.refetch()}
            variant="default"
          >
            {t("common.retry")}
          </Button>
        </Stack>
      </Alert>
    );
  }

  const runs = runsQuery.data ?? [];
  return (
    <Stack gap="lg">
      <Group align="flex-start" justify="space-between" wrap="wrap">
        <Stack gap={4}>
          <Title order={1}>{t("runs.title")}</Title>
          <Text c="dimmed">{t("runs.description")}</Text>
        </Stack>
        <Button
          leftSection={<IconRefresh aria-hidden="true" size={17} />}
          onClick={() => void runsQuery.refetch()}
          variant="default"
        >
          {t("runs.refresh")}
        </Button>
      </Group>

      <Paper radius="lg" withBorder>
        {runs.length === 0 ? (
          <Stack align="center" gap="xs" p="xl" ta="center">
            <Text fw={600}>{t("runs.empty_title")}</Text>
            <Text c="dimmed" size="sm">
              {t("runs.empty_description")}
            </Text>
          </Stack>
        ) : (
          <Table.ScrollContainer minWidth={760} type="native">
            <Table horizontalSpacing="md" verticalSpacing="md">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>{t("runs.grid.batch")}</Table.Th>
                  <Table.Th>{t("runs.grid.status")}</Table.Th>
                  <Table.Th>{t("runs.grid.progress")}</Table.Th>
                  <Table.Th>{t("runs.grid.started")}</Table.Th>
                  <Table.Th>{t("runs.grid.finished")}</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {runs.map((run) => {
                  const handled = Math.min(
                    run.taskIds.length,
                    run.completedCount + run.failedCount,
                  );
                  const percentage =
                    run.taskIds.length === 0
                      ? 0
                      : (handled / run.taskIds.length) * 100;
                  return (
                    <Table.Tr key={run.id}>
                      <Table.Td>
                        <Stack gap={4}>
                          <Text ff="monospace" size="xs">
                            {run.id.slice(0, 18)}…
                          </Text>
                          {run.isTestFixture ? (
                            <Badge color="violet" size="xs" variant="light">
                              {t("runs.fixture_badge")}
                            </Badge>
                          ) : null}
                          {run.targetStage ? (
                            <Badge color="blue" size="xs" variant="light">
                              {t("runs.single_stage_badge", {
                                stage: t(`workbench.grid.${run.targetStage}`),
                              })}
                            </Badge>
                          ) : null}
                        </Stack>
                      </Table.Td>
                      <Table.Td>
                        <StatusBadge scope="run" status={run.status} />
                      </Table.Td>
                      <Table.Td>
                        <Stack gap={4} miw={190}>
                          <Progress value={percentage} />
                          <Text c="dimmed" size="xs">
                            {t("runs.progress", {
                              completed: run.completedCount,
                              failed: run.failedCount,
                              total: run.taskIds.length,
                            })}
                          </Text>
                        </Stack>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">
                          {new Intl.DateTimeFormat(i18n.language, {
                            dateStyle: "short",
                            timeStyle: "short",
                          }).format(new Date(run.startedAtMs))}
                        </Text>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">
                          {run.finishedAtMs
                            ? new Intl.DateTimeFormat(i18n.language, {
                                dateStyle: "short",
                                timeStyle: "short",
                              }).format(new Date(run.finishedAtMs))
                            : t("runs.not_finished")}
                        </Text>
                      </Table.Td>
                    </Table.Tr>
                  );
                })}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}
      </Paper>
    </Stack>
  );
}
