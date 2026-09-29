import { Alert, Button, Center, Loader, Stack, Text } from "@mantine/core";
import { IconAlertCircle, IconRefresh } from "@tabler/icons-react";
import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { useAppMetadata, useInterfaceLanguage } from "../AppShell";
import { SettingsPageTemplate } from "../brand-support/SettingsPageTemplate";
import {
  getAutostartEnabled,
  getSystemNotificationSetting,
  getTelemetryEnabled,
  setAutostartEnabled,
  setSystemNotificationEnabled,
  setTelemetryEnabled,
} from "../tauriCommands";

/** 当前页面内的短暂宿主状态快照；卸载后不会留下 WebView 持久数据。 */
interface HostBooleanSnapshot {
  enabled: boolean | "unknown";
  failed: boolean;
  loading: boolean;
  reread: () => Promise<void>;
}

/** 每次挂载都从 Rust/OS 读取权威布尔值，并仅保存在 React 生命周期内。 */
function useHostBooleanSnapshot(
  getEnabled: () => Promise<boolean>,
): HostBooleanSnapshot {
  const [enabled, setEnabled] = useState<boolean | "unknown">("unknown");
  const [failed, setFailed] = useState(false);
  const [loading, setLoading] = useState(true);

  const reread = useCallback(async (): Promise<void> => {
    setLoading(true);
    try {
      setEnabled(await getEnabled());
      setFailed(false);
    } catch {
      setEnabled("unknown");
      setFailed(true);
    } finally {
      setLoading(false);
    }
  }, [getEnabled]);

  useEffect(() => {
    void reread();
  }, [reread]);

  return { enabled, failed, loading, reread };
}

/** 渲染语言、主题、通知和自启四组设置并保持宿主状态权威。 */
function SettingsRouteComponent(): ReactElement | null {
  const { t } = useTranslation("app");
  const metadataQuery = useAppMetadata();
  const { language, languageFailed, setLanguage } = useInterfaceLanguage();
  const notificationState = useHostBooleanSnapshot(
    getSystemNotificationSetting,
  );
  const autostartState = useHostBooleanSnapshot(getAutostartEnabled);
  const telemetryState = useHostBooleanSnapshot(getTelemetryEnabled);

  if (
    !metadataQuery.data ||
    notificationState.loading ||
    autostartState.loading ||
    telemetryState.loading
  ) {
    return (
      <Center mih={240}>
        <Stack align="center" gap="sm" role="status">
          <Loader aria-label={t("common.loading_application")} size="sm" />
          <Text c="dimmed">{t("common.loading_application")}</Text>
        </Stack>
      </Center>
    );
  }

  const hostStateFailed =
    notificationState.failed || autostartState.failed || telemetryState.failed;

  return (
    <Stack gap="lg">
      {hostStateFailed ? (
        <Alert
          color="yellow"
          icon={<IconAlertCircle aria-hidden="true" size={20} />}
          role="alert"
        >
          <Stack align="flex-start" gap="sm">
            <Text>{t("settings.host_state_failed")}</Text>
            <Button
              leftSection={<IconRefresh aria-hidden="true" size={18} />}
              onClick={() => {
                void Promise.all([
                  notificationState.reread(),
                  autostartState.reread(),
                  telemetryState.reread(),
                ]);
              }}
              variant="default"
            >
              {t("common.retry")}
            </Button>
          </Stack>
        </Alert>
      ) : null}
      {languageFailed ? (
        <Alert color="red" role="alert">
          {t("settings.language_failed")}
        </Alert>
      ) : null}
      <SettingsPageTemplate
        applicationName={metadataQuery.data.applicationName}
        autostart={{
          enabled: autostartState.enabled,
          getEnabled: getAutostartEnabled,
          onChange: setAutostartEnabled,
        }}
        language={language}
        onLanguageChange={setLanguage}
        systemNotification={{
          enabled: notificationState.enabled,
          getEnabled: getSystemNotificationSetting,
          onChange: setSystemNotificationEnabled,
        }}
        telemetry={{
          enabled: telemetryState.enabled,
          getEnabled: getTelemetryEnabled,
          onChange: setTelemetryEnabled,
        }}
        version={metadataQuery.data.version}
      />
    </Stack>
  );
}

export const Route = createFileRoute("/settings")({
  component: SettingsRouteComponent,
});
