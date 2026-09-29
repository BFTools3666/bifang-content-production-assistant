import { Group, Pagination, Select, Text } from "@mantine/core";
import { useAtomValue, useSetAtom } from "jotai";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import {
  WORKBENCH_PAGE_SIZE_OPTIONS,
  workbenchPageSession,
} from "./workbenchState";
import classes from "./workbench.module.css";

/** 只修改进程内页面会话，不改变完整工作台 Query 快照。 */
export function WorkbenchPagination({
  resultCount,
}: {
  resultCount: number;
}): ReactElement | null {
  const { t } = useTranslation("app");
  const session = useAtomValue(workbenchPageSession.stateAtom);
  const setPagination = useSetAtom(workbenchPageSession.setPaginationAtom);
  if (resultCount === 0) return null;

  return (
    <Group
      className={classes.paginationBar}
      justify="space-between"
      p="md"
      wrap="wrap"
    >
      <Text c="dimmed" size="sm">
        {t("workbench.pagination.summary", {
          end: Math.min(session.page * session.pageSize, resultCount),
          start: (session.page - 1) * session.pageSize + 1,
          total: resultCount,
        })}
      </Text>
      <Group wrap="wrap">
        <Select
          aria-label={t("workbench.pagination.page_size_label")}
          data={WORKBENCH_PAGE_SIZE_OPTIONS.map((size) => ({
            label: String(size),
            value: String(size),
          }))}
          label={t("workbench.pagination.page_size_label")}
          onChange={(value) => {
            if (!value) return;
            setPagination({
              page: session.page,
              pageSize: Number(value),
            });
          }}
          size="xs"
          value={String(session.pageSize)}
          w={120}
        />
        <Pagination
          aria-label={t("workbench.pagination.aria_label")}
          onChange={(page) =>
            setPagination({ page, pageSize: session.pageSize })
          }
          total={Math.ceil(resultCount / session.pageSize)}
          value={session.page}
        />
      </Group>
    </Group>
  );
}
