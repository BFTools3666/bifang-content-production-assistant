import {
  Alert,
  AppShell as MantineAppShell,
  Box,
  Button,
  Center,
  Loader,
  Stack,
  Text,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconHistory,
  IconLayoutGrid,
  IconMessageCode,
  IconPlugConnected,
  IconRefresh,
  IconRobot,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { useAtom } from "jotai";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { interfaceLanguageAtom } from "./appState";
import {
  APP_SIDEBAR_LOGO_PATH,
  APP_SIDEBAR_WIDTHS,
  AppSidebarTemplate,
} from "./brand-support/AppSidebarTemplate";
import { MandatoryUpdateGateTemplate } from "./brand-support/MandatoryUpdateGateTemplate";
import type { SupportedInterfaceLanguage } from "./brand-support/SettingsPageTemplate";
import i18n from "./i18n";
import {
  exitApplication,
  getAppMetadata,
  getSystemLocale,
  openUpdateDownload,
  setInterfaceLanguage,
  startPublicServices,
} from "./tauriCommands";

/** 当前中性 GUI 中可导航的固定本地路由。 */
type AppPath =
  | "/"
  | "/prompts"
  | "/providers"
  | "/connectors"
  | "/runs"
  | "/about"
  | "/settings"
  | "/sponsor";

/** 设置页消费的当前进程语言控制器；持久事实始终由 Rust 命令拥有。 */
interface InterfaceLanguageController {
  language: SupportedInterfaceLanguage;
  languageFailed: boolean;
  setLanguage: (language: SupportedInterfaceLanguage) => void;
}

const InterfaceLanguageContext = createContext<
  InterfaceLanguageController | undefined
>(undefined);

/** 判断侧栏回调是否给出了当前应用允许的本地路由。 */
function isAppPath(path: string): path is AppPath {
  return [
    "/",
    "/prompts",
    "/providers",
    "/connectors",
    "/runs",
    "/about",
    "/settings",
    "/sponsor",
  ].includes(path);
}

/** 复用唯一 QueryClient 缓存读取权威应用元数据。 */
export function useAppMetadata() {
  return useQuery({
    queryFn: getAppMetadata,
    queryKey: ["app-metadata"],
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
}

/** 让设置页复用壳层中唯一的语言读写路径。 */
export function useInterfaceLanguage(): InterfaceLanguageController {
  const controller = useContext(InterfaceLanguageContext);
  if (!controller) {
    throw new Error("interface language controller is unavailable");
  }
  return controller;
}

/** 建立 compact 侧栏、动态标题与路由内容区域。 */
export function AppShell({ children }: { children: ReactNode }): ReactElement {
  const { t } = useTranslation("app");
  const navigate = useNavigate();
  const activePath = useRouterState({
    select: (state) => state.location.pathname,
  });
  const metadataQuery = useAppMetadata();
  const systemLocaleQuery = useQuery({
    queryFn: getSystemLocale,
    queryKey: ["system-locale"],
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const publicServicesQuery = useQuery({
    enabled: metadataQuery.data !== undefined,
    queryFn: startPublicServices,
    queryKey: ["update-presentation"],
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const [language, setLanguage] = useAtom(interfaceLanguageAtom);
  const [languageFailed, setLanguageFailed] = useState(false);

  useEffect(() => {
    if (!metadataQuery.data) return;
    document.title = metadataQuery.data.title;
  }, [metadataQuery.data]);

  useEffect(() => {
    if (!systemLocaleQuery.data) return;
    setLanguage(systemLocaleQuery.data);
    void i18n.changeLanguage(systemLocaleQuery.data);
  }, [setLanguage, systemLocaleQuery.data]);

  /** 先由窄命令写入宿主偏好，再同步本进程的翻译状态。 */
  const handleLanguageChange = useCallback(
    (nextLanguage: SupportedInterfaceLanguage): void => {
      setLanguageFailed(false);
      void setInterfaceLanguage(nextLanguage)
        .then(async (authoritativeLanguage) => {
          await i18n.changeLanguage(authoritativeLanguage);
          setLanguage(authoritativeLanguage);
        })
        .catch(() => {
          setLanguageFailed(true);
        });
    },
    [setLanguage],
  );

  const languageController = useMemo<InterfaceLanguageController>(
    () => ({
      language,
      languageFailed,
      setLanguage: handleLanguageChange,
    }),
    [handleLanguageChange, language, languageFailed],
  );

  if (metadataQuery.isPending) {
    return (
      <Center mih="100dvh">
        <Stack align="center" gap="sm" role="status">
          <Loader aria-label={t("common.loading_application")} size="sm" />
          <Text c="dimmed">{t("common.loading_application")}</Text>
        </Stack>
      </Center>
    );
  }

  if (metadataQuery.isError || !metadataQuery.data) {
    return (
      <Center mih="100dvh" p="lg">
        <Alert
          color="red"
          icon={<IconAlertCircle aria-hidden="true" size={20} />}
          maw={560}
          role="alert"
          title={t("common.load_application_failed")}
        >
          <Button
            leftSection={<IconRefresh aria-hidden="true" size={18} />}
            mt="md"
            onClick={() => {
              void metadataQuery.refetch();
            }}
            variant="default"
          >
            {t("common.retry")}
          </Button>
        </Alert>
      </Center>
    );
  }

  const featureItems = [
    {
      icon: IconLayoutGrid,
      id: "workbench",
      label: t("navigation.workbench"),
      to: "/",
    },
    {
      icon: IconMessageCode,
      id: "prompts",
      label: t("navigation.prompts"),
      to: "/prompts",
    },
    {
      icon: IconRobot,
      id: "providers",
      label: t("navigation.providers"),
      to: "/providers",
    },
    {
      icon: IconPlugConnected,
      id: "connectors",
      label: t("navigation.connectors"),
      to: "/connectors",
    },
    {
      icon: IconHistory,
      id: "runs",
      label: t("navigation.runs"),
      to: "/runs",
    },
  ];

  /** 只接受固定路由并把动作交给 TanStack Router。 */
  const handleNavigate = (path: string): void => {
    if (isAppPath(path)) void navigate({ to: path });
  };

  const update = publicServicesQuery.data ?? {
    currentVersion: metadataQuery.data.version,
    downloadReady: false,
    status: publicServicesQuery.isPending
      ? ("checking" as const)
      : ("failed" as const),
  };

  return (
    <InterfaceLanguageContext.Provider value={languageController}>
      <MandatoryUpdateGateTemplate
        onExitApplication={() => void exitApplication()}
        onOpenUpdateDownload={() => void openUpdateDownload()}
        update={update}
      >
        <MantineAppShell
          data-mode="compact"
          data-navbar-width={APP_SIDEBAR_WIDTHS.compact}
          data-testid="app-shell"
          navbar={{ breakpoint: "sm", width: APP_SIDEBAR_WIDTHS.compact }}
        >
          <MantineAppShell.Navbar p={0}>
            <AppSidebarTemplate
              activePath={activePath}
              applicationName={metadataQuery.data.applicationName}
              featureItems={featureItems}
              logoSrc={APP_SIDEBAR_LOGO_PATH}
              mode="compact"
              onNavigate={handleNavigate}
              supportPages={{ aboutPage: true, sponsorPage: true }}
              version={metadataQuery.data.version}
            />
          </MantineAppShell.Navbar>
          <MantineAppShell.Main>
            <Box p={{ base: "md", sm: "xl" }}>{children}</Box>
          </MantineAppShell.Main>
        </MantineAppShell>
      </MandatoryUpdateGateTemplate>
    </InterfaceLanguageContext.Provider>
  );
}
