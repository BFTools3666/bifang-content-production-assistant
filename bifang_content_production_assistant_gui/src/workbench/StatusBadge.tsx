import { Badge } from "@mantine/core";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import type { OverallStatus, RunStatus, StageStatus } from "./workbenchTypes";

/** Mantine 状态色只作为文字状态的辅助线索。 */
const STATUS_COLORS: Record<OverallStatus | RunStatus | StageStatus, string> = {
  approved: "green",
  cancelled: "gray",
  draft: "gray",
  failed: "red",
  interrupted: "orange",
  notStarted: "gray",
  partialFailure: "red",
  paused: "yellow",
  pendingReview: "violet",
  queued: "blue",
  running: "cyan",
  stale: "orange",
  succeeded: "green",
};

/** 用文字与语义色展示任意工作流状态。 */
export function StatusBadge({
  status,
  scope,
}: {
  status: OverallStatus | RunStatus | StageStatus;
  scope: "overall" | "run" | "stage";
}): ReactElement {
  const { t } = useTranslation("app");
  return (
    <Badge color={STATUS_COLORS[status]} size="sm" variant="light">
      {t(`status.${scope}.${status}`)}
    </Badge>
  );
}
