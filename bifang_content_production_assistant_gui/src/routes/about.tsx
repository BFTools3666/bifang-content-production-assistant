import { Center, Loader } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { useAppMetadata } from "../AppShell";
import { AboutPageTemplate } from "../brand-support/AboutPageTemplate";
import type { UpdatePresentation } from "../brand-support/updatePresentation";
import { checkForUpdates } from "../tauriCommands";

/** 渲染关于、零出站更新状态与本地候选更新日志入口。 */
function AboutRouteComponent(): ReactElement {
  const { t } = useTranslation("app");
  const metadataQuery = useAppMetadata();
  const updateQuery = useQuery({
    enabled: metadataQuery.data !== undefined,
    placeholderData: metadataQuery.data
      ? {
          currentVersion: metadataQuery.data.version,
          status: "not-configured",
        }
      : undefined,
    queryFn: () => checkForUpdates(metadataQuery.data?.version ?? ""),
    queryKey: ["update-presentation"],
    retry: false,
  });

  if (!metadataQuery.data) {
    return (
      <Center mih={240} role="status">
        <Loader aria-label={t("common.loading_application")} size="sm" />
      </Center>
    );
  }

  const failedUpdate: UpdatePresentation = {
    currentVersion: metadataQuery.data.version,
    status: "failed",
  };
  const update = updateQuery.isError
    ? failedUpdate
    : (updateQuery.data ?? {
        currentVersion: metadataQuery.data.version,
        status: "not-configured",
      });

  return (
    <AboutPageTemplate
      onCheckForUpdates={() => {
        void updateQuery.refetch();
      }}
      productName={metadataQuery.data.applicationName}
      update={update}
      version={metadataQuery.data.version}
    />
  );
}

export const Route = createFileRoute("/about")({
  component: AboutRouteComponent,
});
