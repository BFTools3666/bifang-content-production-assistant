import {
  Alert,
  Badge,
  Button,
  Group,
  Modal,
  Paper,
  Select,
  Stack,
  Stepper,
  Text,
  TextInput,
  Title,
} from "@mantine/core";
import {
  IconArrowLeft,
  IconArrowRight,
  IconCheck,
  IconRefresh,
  IconSettings,
  IconTerminal2,
} from "@tabler/icons-react";
import { useEffect, useRef, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { inspectLarkCliSetup } from "./workbenchCommands";
import { localizedCommandError } from "./workbenchErrors";
import type { LarkCliSetupInspection } from "./workbenchTypes";

const DEFAULT_PROFILE = "seo-publisher";
const PROFILE_SPECIAL = new Set([
  "/",
  "\\",
  '"',
  "'",
  "`",
  "$",
  "#",
  "!",
  "&",
  "|",
  ";",
  "(",
  ")",
  "{",
  "}",
  "[",
  "]",
  "<",
  ">",
  "?",
  "*",
  "~",
]);
const TOKEN_PATTERN = /^[A-Za-z0-9_-]{6,256}$/u;

/** 只读 CLI 检测与已由用户独立配置的发布目标回填入口。 */
interface LarkCliSetupGuideProps {
  disabled?: boolean;
  openInitially: boolean;
  onUseConfiguration: (profile: string, folderToken: string) => void;
}

/** 安全地接受 CLI profile 名称，不将其解释为命令或 shell 参数。 */
export function isSafeLarkCliProfile(profile: string): boolean {
  const trimmed = profile.trim();
  return (
    trimmed === profile &&
    [...profile].length >= 1 &&
    [...profile].length <= 64 &&
    ![...profile].some(
      (character) => /\s/u.test(character) || PROFILE_SPECIAL.has(character),
    ) &&
    !/[\u0000-\u001f\u007f]/u.test(profile)
  );
}

/** 仅从官方云盘文件夹链接或原始 token 提取目标文件夹。 */
export function extractFeishuFolderToken(value: string): string | null {
  const trimmed = value.trim();
  if (TOKEN_PATTERN.test(trimmed)) return trimmed;
  try {
    const url = new URL(trimmed);
    const officialHost =
      url.hostname === "feishu.cn" ||
      url.hostname.endsWith(".feishu.cn") ||
      url.hostname === "larksuite.com" ||
      url.hostname.endsWith(".larksuite.com");
    const match = url.pathname.match(/\/drive\/folder\/([^/?#]+)/u);
    const token = match?.[1] ?? "";
    return officialHost && TOKEN_PATTERN.test(token) ? token : null;
  } catch {
    return null;
  }
}

/** 用户自行完成 CLI 安装与登录；应用只检测既有 profile 和用户身份。 */
export function LarkCliSetupGuide({
  disabled = false,
  openInitially,
  onUseConfiguration,
}: LarkCliSetupGuideProps): ReactElement {
  const { t } = useTranslation("app");
  const [opened, setOpened] = useState(openInitially);
  const autoOpened = useRef(openInitially);
  const [active, setActive] = useState(0);
  const [profile, setProfile] = useState(DEFAULT_PROFILE);
  const [folderInput, setFolderInput] = useState("");
  const [inspection, setInspection] = useState<LarkCliSetupInspection | null>(
    null,
  );
  const [inspectedProfile, setInspectedProfile] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const mounted = useRef(false);
  const currentProfile = useRef(profile);
  const inspectionRequest = useRef(0);

  currentProfile.current = profile;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      inspectionRequest.current += 1;
    };
  }, []);

  useEffect(() => {
    if (openInitially && !autoOpened.current) {
      autoOpened.current = true;
      setOpened(true);
    }
  }, [openInitially]);

  const safeProfile = isSafeLarkCliProfile(profile);
  const folderToken = extractFeishuFolderToken(folderInput);
  const currentInspection = inspectedProfile === profile ? inspection : null;
  const selectedAuthorized =
    currentInspection?.selectedProfileAuthorized ?? false;
  const authorizationIssue =
    currentInspection &&
    !selectedAuthorized &&
    currentInspection.authorizationIssueCode
      ? localizedCommandError(
          { code: currentInspection.authorizationIssueCode },
          t,
        )
      : null;

  /** 每次检查由用户显式触发；旧结果不会回填到新 profile。 */
  const handleInspect = async (): Promise<void> => {
    if (!safeProfile) {
      setError(t("connectors.guide.profile_invalid"));
      return;
    }
    const targetProfile = profile;
    const requestId = inspectionRequest.current + 1;
    inspectionRequest.current = requestId;
    setPending(true);
    setInspectedProfile(null);
    setError(null);
    setFeedback(null);
    try {
      const result = await inspectLarkCliSetup(targetProfile);
      if (
        mounted.current &&
        inspectionRequest.current === requestId &&
        currentProfile.current === targetProfile
      ) {
        setInspection(result);
        setInspectedProfile(targetProfile);
        if (active === 0) setActive(1);
        setFeedback(
          t("connectors.guide.detected", { version: result.version }),
        );
      }
    } catch (reason) {
      if (
        mounted.current &&
        inspectionRequest.current === requestId &&
        currentProfile.current === targetProfile
      ) {
        setError(localizedCommandError(reason, t));
      }
    } finally {
      if (mounted.current && inspectionRequest.current === requestId) {
        setPending(false);
      }
    }
  };

  /** 修改 profile 后使此前的身份检查失效，但保留只读 profile 清单。 */
  const handleProfileChange = (value: string): void => {
    if (pending) return;
    currentProfile.current = value;
    inspectionRequest.current += 1;
    setProfile(value);
    setInspectedProfile(null);
    setError(null);
    setFeedback(null);
  };

  return (
    <>
      <Paper p="lg" radius="md" withBorder>
        <Group align="center" justify="space-between" wrap="wrap">
          <Stack gap={3} maw={720}>
            <Group gap="xs">
              <IconSettings aria-hidden="true" size={20} />
              <Title order={2} size="h3">
                {t("connectors.guide.entry_title")}
              </Title>
              {selectedAuthorized ? (
                <Badge color="green">{t("connectors.guide.ready")}</Badge>
              ) : null}
            </Group>
            <Text c="dimmed" size="sm">
              {t("connectors.guide.entry_description")}
            </Text>
          </Stack>
          <Button
            disabled={disabled}
            leftSection={<IconSettings aria-hidden="true" size={17} />}
            onClick={() => setOpened(true)}
            variant={openInitially ? "filled" : "default"}
          >
            {t("connectors.guide.open")}
          </Button>
        </Group>
      </Paper>

      <Modal
        centered
        closeOnClickOutside={!pending}
        closeOnEscape={!pending}
        onClose={() => setOpened(false)}
        opened={opened}
        size="lg"
        title={t("connectors.guide.title")}
        withCloseButton={!pending}
      >
        <Stack gap="lg">
          <Text c="dimmed" size="sm">
            {t("connectors.guide.description")}
          </Text>
          <Alert color="blue" icon={<IconTerminal2 size={17} />}>
            {t("connectors.guide.external_setup_notice")}
          </Alert>
          {error ? (
            <Alert color="red" role="alert">
              {error}
            </Alert>
          ) : null}
          {feedback ? (
            <Alert color="green" role="status">
              {feedback}
            </Alert>
          ) : null}

          <Stepper active={active} orientation="vertical">
            <Stepper.Step
              description={t("connectors.guide.install_description")}
              label={t("connectors.guide.install_title")}
            >
              <Stack gap="sm" mt="sm">
                <Text size="sm">{t("connectors.guide.install_body")}</Text>
                {currentInspection ? (
                  <Badge color="green" variant="light">
                    {t("connectors.guide.version_ready", {
                      version: currentInspection.version,
                    })}
                  </Badge>
                ) : null}
                <Button
                  disabled={!safeProfile}
                  leftSection={<IconRefresh aria-hidden="true" size={16} />}
                  loading={pending}
                  onClick={() => void handleInspect()}
                  variant="default"
                >
                  {t("connectors.guide.detect")}
                </Button>
              </Stack>
            </Stepper.Step>

            <Stepper.Step
              description={t("connectors.guide.profile_description")}
              label={t("connectors.guide.profile_title")}
            >
              <Stack gap="sm" mt="sm">
                <TextInput
                  description={t("connectors.guide.profile_hint")}
                  disabled={pending}
                  error={
                    profile.length > 0 && !safeProfile
                      ? t("connectors.guide.profile_invalid")
                      : undefined
                  }
                  label={t("connectors.guide.profile_name")}
                  onChange={(event) =>
                    handleProfileChange(event.currentTarget.value)
                  }
                  value={profile}
                />
                {(inspection?.profiles.length ?? 0) > 0 ? (
                  <Select
                    allowDeselect={false}
                    data={inspection?.profiles.map((item) => ({
                      label: item.userName
                        ? `${item.name} · ${item.userName}`
                        : item.name,
                      value: item.name,
                    }))}
                    disabled={pending}
                    label={t("connectors.guide.existing_profile")}
                    onChange={(value) => {
                      if (value) handleProfileChange(value);
                    }}
                    value={
                      inspection?.profiles.some((item) => item.name === profile)
                        ? profile
                        : null
                    }
                  />
                ) : null}
                {selectedAuthorized ? (
                  <Alert color="green" icon={<IconCheck size={17} />}>
                    {t("connectors.guide.authorization_ready", {
                      name:
                        currentInspection?.selectedUserName ??
                        t("connectors.guide.current_user"),
                    })}
                  </Alert>
                ) : authorizationIssue ? (
                  <Alert color="yellow" role="alert">
                    {authorizationIssue}
                  </Alert>
                ) : null}
                <Button
                  disabled={!safeProfile}
                  leftSection={<IconRefresh aria-hidden="true" size={16} />}
                  loading={pending}
                  onClick={() => void handleInspect()}
                  variant="default"
                >
                  {t("connectors.guide.recheck")}
                </Button>
                <Text c="dimmed" size="xs">
                  {t("connectors.guide.profile_external_hint")}
                </Text>
              </Stack>
            </Stepper.Step>

            <Stepper.Step
              description={t("connectors.guide.folder_description")}
              label={t("connectors.guide.folder_title")}
            >
              <Stack gap="sm" mt="sm">
                <TextInput
                  description={t("connectors.guide.folder_hint")}
                  error={
                    folderInput.length > 0 && !folderToken
                      ? t("connectors.guide.folder_invalid")
                      : undefined
                  }
                  label={t("connectors.guide.folder_input")}
                  onChange={(event) =>
                    setFolderInput(event.currentTarget.value)
                  }
                  placeholder="https://example.feishu.cn/drive/folder/fldcn..."
                  value={folderInput}
                />
                {folderToken ? (
                  <Alert color="green">
                    {t("connectors.guide.folder_extracted", { folderToken })}
                  </Alert>
                ) : null}
                <Text c="dimmed" size="xs">
                  {t("connectors.guide.folder_scope")}
                </Text>
                <Alert color="blue">
                  {t("connectors.guide.finish_summary", {
                    folderToken: folderToken ?? "—",
                    profile,
                  })}
                </Alert>
                <Button
                  disabled={!safeProfile || !selectedAuthorized || !folderToken}
                  leftSection={<IconCheck aria-hidden="true" size={17} />}
                  onClick={() => {
                    if (!folderToken || !selectedAuthorized) return;
                    onUseConfiguration(profile, folderToken);
                    setOpened(false);
                  }}
                >
                  {t("connectors.guide.use_configuration")}
                </Button>
              </Stack>
            </Stepper.Step>
          </Stepper>

          <Group justify="space-between">
            <Button
              disabled={active === 0 || pending}
              leftSection={<IconArrowLeft aria-hidden="true" size={16} />}
              onClick={() => setActive((value) => Math.max(0, value - 1))}
              variant="default"
            >
              {t("common.previous")}
            </Button>
            <Button
              disabled={
                pending || active >= 2 || (active === 1 && !selectedAuthorized)
              }
              onClick={() => setActive((value) => Math.min(2, value + 1))}
              rightSection={<IconArrowRight aria-hidden="true" size={16} />}
            >
              {t("common.next")}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
