import {
  Alert,
  Button,
  Code,
  Group,
  Loader,
  Modal,
  SegmentedControl,
  Select,
  Stack,
  Tabs,
  Text,
} from "@mantine/core";
import { IconCloudUpload, IconCopy, IconRefresh } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useAtom } from "jotai";
import { useEffect, useMemo, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import {
  buildHtmlProjection,
  buildMarkdownProjection,
  buildSafePreviewDocument,
  compactImageDataUrl,
} from "./articleFormatProjection";
import {
  listConnectorConfigs,
  loadArticleImage,
  syncArticleToConnector,
} from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import {
  articleFormatTabAtom,
  articleFormatViewAtom,
  type ArticleFormatTab,
} from "./workbenchState";
import classes from "./workbench.module.css";

/** 受管配图只在格式页活动时读取，不触发任何生成动作。 */
type FormatImageState =
  | { status: "idle" | "loading"; dataUrl: null }
  | { status: "ready"; dataUrl: string | null }
  | { status: "error"; dataUrl: null };

/** 格式查看器只接收当前文章的安全聚合字段与显式复制动作。 */
interface ArticleFormatViewerProps {
  active: boolean;
  formatReady: boolean;
  hasImage: boolean;
  html: string | null;
  imageRefreshKey: string;
  markdown: string | null;
  onCopy: (value: string, format: string) => Promise<void>;
  packageVersion: number;
  rowVersion: number;
  taskId: string;
  title: string;
}

/** 打开确认弹窗时冻结全部外部写入身份，后续刷新只能使后端拒绝而不能偷换上传对象。 */
interface ConnectorSyncCandidate {
  connectorId: string;
  connectorName: string;
  connectorRevision: number;
  folderToken: string;
  format: ArticleFormatTab;
  packageVersion: number;
  rowVersion: number;
  taskId: string;
  title: string;
}

/** 以并列格式页签展示可复制源码和隔离的解释后预览。 */
export function ArticleFormatViewer({
  active,
  formatReady,
  hasImage,
  html,
  imageRefreshKey,
  markdown,
  onCopy,
  packageVersion,
  rowVersion,
  taskId,
  title,
}: ArticleFormatViewerProps): ReactElement {
  const { t } = useTranslation("app");
  const [format, setFormat] = useAtom(articleFormatTabAtom);
  const [view, setView] = useAtom(articleFormatViewAtom);
  const [reloadToken, setReloadToken] = useState(0);
  const connectorsQuery = useQuery({
    enabled: active,
    queryFn: listConnectorConfigs,
    queryKey: ["connector-configs"],
  });
  const [selectedConnectorId, setSelectedConnectorId] = useState<string | null>(
    null,
  );
  const [syncCandidate, setSyncCandidate] =
    useState<ConnectorSyncCandidate | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncFeedback, setSyncFeedback] = useState<string | null>(null);
  const [image, setImage] = useState<FormatImageState>({
    dataUrl: null,
    status: hasImage ? "idle" : "ready",
  });

  useEffect(() => {
    if (!hasImage) {
      setImage({ dataUrl: null, status: "ready" });
      return undefined;
    }
    if (!active) return undefined;

    let current = true;
    setImage({ dataUrl: null, status: "loading" });
    void loadArticleImage(taskId)
      .then((dataUrl) => {
        if (!current) return;
        setImage(
          dataUrl
            ? { dataUrl, status: "ready" }
            : { dataUrl: null, status: "error" },
        );
      })
      .catch(() => {
        if (current) setImage({ dataUrl: null, status: "error" });
      });
    return () => {
      current = false;
    };
  }, [active, hasImage, imageRefreshKey, reloadToken, taskId]);

  const enabledConnectors = useMemo(
    () =>
      (connectorsQuery.data ?? []).filter(
        (connector) =>
          connector.enabled && connector.cliProfile.trim().length > 0,
      ),
    [connectorsQuery.data],
  );
  useEffect(() => {
    if (enabledConnectors.length === 0) {
      setSelectedConnectorId(null);
      return;
    }
    if (
      selectedConnectorId &&
      enabledConnectors.some(
        (connector) => connector.id === selectedConnectorId,
      )
    ) {
      return;
    }
    const nextConnector =
      enabledConnectors.find((connector) => connector.isDefault) ??
      enabledConnectors[0];
    setSelectedConnectorId(nextConnector?.id ?? null);
    if (nextConnector) setFormat(nextConnector.defaultFormat);
  }, [enabledConnectors, selectedConnectorId, setFormat]);

  const selectedConnector =
    enabledConnectors.find(
      (connector) => connector.id === selectedConnectorId,
    ) ?? null;

  /** 同步按钮的确认只冻结当前连接器修订和文章版本，远端写入不自动重试。 */
  const handleSync = async (): Promise<void> => {
    if (!syncCandidate) return;
    setSyncing(true);
    setSyncError(null);
    setSyncFeedback(null);
    try {
      const result = await syncArticleToConnector(
        syncCandidate.taskId,
        syncCandidate.connectorId,
        syncCandidate.format,
        syncCandidate.rowVersion,
        syncCandidate.packageVersion,
        syncCandidate.connectorRevision,
      );
      setSyncFeedback(
        t(
          result.reused
            ? "details.connector_sync.reused"
            : "details.connector_sync.succeeded",
          { fileName: result.fileName },
        ),
      );
      setSyncCandidate(null);
    } catch (reason) {
      setSyncError(localizedCommandError(reason, t));
    } finally {
      setSyncing(false);
    }
  };

  const imageDataUrl = image.status === "ready" ? image.dataUrl : null;
  const compactImageUrl = useMemo(
    () => compactImageDataUrl(imageDataUrl),
    [imageDataUrl],
  );
  const imageRequired = hasImage && image.status !== "ready";
  const previewDocument = useMemo(
    () =>
      view !== "rendered" || html === null || imageRequired
        ? null
        : buildSafePreviewDocument(html, imageDataUrl, title),
    [html, imageDataUrl, imageRequired, title, view],
  );

  const selectFormat = (value: string | null): void => {
    if (value === "markdown" || value === "html") setFormat(value);
  };
  const selectView = (value: string): void => {
    if (value === "source" || value === "rendered") setView(value);
  };
  const selectConnector = (value: string | null): void => {
    setSelectedConnectorId(value);
    setSyncError(null);
    setSyncFeedback(null);
    const connector = enabledConnectors.find((item) => item.id === value);
    if (connector) setFormat(connector.defaultFormat);
  };

  const renderPanel = (
    panelFormat: ArticleFormatTab,
    source: string | null,
  ): ReactElement => {
    const formatLabel = t(
      panelFormat === "markdown"
        ? "details.markdown_title"
        : "details.html_title",
    );
    const compactSource =
      source === null
        ? null
        : panelFormat === "markdown"
          ? buildMarkdownProjection(source, compactImageUrl, title)
          : buildHtmlProjection(source, compactImageUrl, title);
    const previewAvailable = html !== null;
    const copySource = (): string | null => {
      if (source === null) return null;
      return panelFormat === "markdown"
        ? buildMarkdownProjection(source, imageDataUrl, title)
        : buildHtmlProjection(source, imageDataUrl, title);
    };

    return (
      <Tabs.Panel pt="md" value={panelFormat}>
        <Stack gap="sm">
          <Group align="center" justify="space-between" wrap="wrap">
            <SegmentedControl
              aria-label={t("details.format_view_selector", {
                format: formatLabel,
              })}
              data={[
                {
                  label: t("details.format_source"),
                  value: "source",
                },
                {
                  label: t("details.format_rendered"),
                  value: "rendered",
                },
              ]}
              onChange={selectView}
              value={view}
            />
            <Group gap="xs" wrap="wrap">
              <Button
                disabled={source === null || imageRequired}
                leftSection={<IconCopy aria-hidden="true" size={16} />}
                onClick={() => {
                  const projection = copySource();
                  if (projection !== null) void onCopy(projection, formatLabel);
                }}
                size="compact-sm"
                variant="default"
              >
                {t("details.copy_format", { format: formatLabel })}
              </Button>
              <Button
                disabled={
                  source === null ||
                  imageRequired ||
                  connectorsQuery.isPending ||
                  selectedConnector === null ||
                  !formatReady ||
                  syncing
                }
                leftSection={<IconCloudUpload aria-hidden="true" size={16} />}
                onClick={() => {
                  if (!selectedConnector) return;
                  setSyncError(null);
                  setSyncFeedback(null);
                  setSyncCandidate({
                    connectorId: selectedConnector.id,
                    connectorName: selectedConnector.name,
                    connectorRevision: selectedConnector.revision,
                    folderToken: selectedConnector.folderToken,
                    format: panelFormat,
                    packageVersion,
                    rowVersion,
                    taskId,
                    title,
                  });
                }}
                size="compact-sm"
              >
                {t("details.connector_sync.action", { format: formatLabel })}
              </Button>
            </Group>
          </Group>

          {hasImage && image.status === "ready" && image.dataUrl ? (
            <Text c="dimmed" size="xs">
              {t("details.format_image_embedded")}
            </Text>
          ) : null}

          {view === "source" ? (
            <>
              {hasImage && image.status === "ready" && image.dataUrl ? (
                <Text c="dimmed" size="xs">
                  {t("details.format_image_compacted")}
                </Text>
              ) : null}
              <Code block className={classes.artifactCode}>
                {compactSource ?? t("details.format_empty")}
              </Code>
            </>
          ) : imageRequired ? (
            <Alert color={image.status === "error" ? "red" : "gray"}>
              {t(
                image.status === "error"
                  ? "details.format_image_failed_hint"
                  : "details.format_image_loading",
              )}
            </Alert>
          ) : previewAvailable && previewDocument ? (
            <iframe
              className={classes.formatPreviewFrame}
              data-testid={`${panelFormat}-rendered-preview`}
              referrerPolicy="no-referrer"
              sandbox=""
              srcDoc={previewDocument}
              title={t("details.format_preview_title", {
                format: formatLabel,
              })}
            />
          ) : (
            <Alert color="gray">{t("details.format_preview_empty")}</Alert>
          )}
        </Stack>
      </Tabs.Panel>
    );
  };

  return (
    <Stack gap="sm">
      {image.status === "loading" ? (
        <Group gap="xs" role="status">
          <Loader size="xs" />
          <Text c="dimmed" size="sm">
            {t("details.format_image_loading")}
          </Text>
        </Group>
      ) : null}
      {image.status === "error" ? (
        <Alert color="red" title={t("details.format_image_failed")}>
          <Group align="center" justify="space-between" wrap="wrap">
            <Text size="sm">{t("details.format_image_failed_hint")}</Text>
            <Button
              leftSection={<IconRefresh aria-hidden="true" size={16} />}
              onClick={() => setReloadToken((value) => value + 1)}
              size="compact-sm"
              variant="default"
            >
              {t("common.retry")}
            </Button>
          </Group>
        </Alert>
      ) : null}

      <Group align="flex-end" justify="space-between" wrap="wrap">
        <Select
          allowDeselect={false}
          data={enabledConnectors.map((connector) => ({
            label: connector.name,
            value: connector.id,
          }))}
          disabled={connectorsQuery.isPending || enabledConnectors.length === 0}
          label={t("details.connector_sync.target")}
          onChange={selectConnector}
          placeholder={t(
            connectorsQuery.isPending
              ? "details.connector_sync.loading"
              : "details.connector_sync.empty",
          )}
          value={selectedConnectorId}
          w={{ base: "100%", sm: 320 }}
        />
        {enabledConnectors.length === 0 && !connectorsQuery.isPending ? (
          <Text c="dimmed" size="xs">
            {t("details.connector_sync.configure_hint")}
          </Text>
        ) : selectedConnector ? (
          <Text c="dimmed" size="xs">
            {t("details.connector_sync.folder", {
              folderToken: selectedConnector.folderToken,
            })}
          </Text>
        ) : null}
      </Group>
      {connectorsQuery.isError ? (
        <Alert color="red">{t("details.connector_sync.load_failed")}</Alert>
      ) : null}
      {!formatReady ? (
        <Alert color="yellow">
          {t("details.connector_sync.format_not_ready")}
        </Alert>
      ) : null}
      {syncFeedback ? (
        <Alert color="green" role="status">
          {syncFeedback}
        </Alert>
      ) : null}

      <Tabs
        className={classes.formatTabs}
        keepMounted={false}
        onChange={selectFormat}
        value={format}
      >
        <Tabs.List
          aria-label={t("details.format_selector")}
          className={classes.formatTabsList}
          grow
        >
          <Tabs.Tab value="markdown">{t("details.markdown_title")}</Tabs.Tab>
          <Tabs.Tab value="html">{t("details.html_title")}</Tabs.Tab>
        </Tabs.List>
        {renderPanel("markdown", markdown)}
        {renderPanel("html", html)}
      </Tabs>

      <Modal
        centered
        onClose={() => {
          if (!syncing) {
            setSyncCandidate(null);
            setSyncError(null);
          }
        }}
        opened={syncCandidate !== null}
        title={t("details.connector_sync.confirm_title")}
      >
        <Stack gap="md">
          <Text>
            {t("details.connector_sync.confirm_body", {
              connector: syncCandidate?.connectorName ?? "",
              folderToken: syncCandidate?.folderToken ?? "",
              format:
                syncCandidate?.format === "html"
                  ? t("details.html_title")
                  : t("details.markdown_title"),
              title: syncCandidate?.title ?? "",
            })}
          </Text>
          <Alert color="blue">
            {t("details.connector_sync.no_retry_notice")}
          </Alert>
          {syncError ? <Alert color="red">{syncError}</Alert> : null}
          <Group justify="flex-end">
            <Button
              disabled={syncing}
              onClick={() => {
                setSyncCandidate(null);
                setSyncError(null);
              }}
              variant="default"
            >
              {t("common.cancel")}
            </Button>
            <Button
              leftSection={<IconCloudUpload aria-hidden="true" size={17} />}
              loading={syncing}
              onClick={() => void handleSync()}
            >
              {t("details.connector_sync.confirm_action")}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
