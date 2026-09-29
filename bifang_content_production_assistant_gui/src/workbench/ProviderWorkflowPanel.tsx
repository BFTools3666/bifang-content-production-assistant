import {
  Alert,
  Badge,
  Button,
  Divider,
  Group,
  Menu,
  Paper,
  Stack,
  Tabs,
  Text,
  Title,
} from "@mantine/core";
import {
  IconArticle,
  IconDeviceFloppy,
  IconPhoto,
  IconPlugConnected,
  IconPlus,
  IconTextCaption,
} from "@tabler/icons-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";

import { ProviderConnectionFields } from "./ProviderConnectionFields";
import {
  ProviderStageCard,
  type ProviderVerificationResult,
  type ProviderVerificationStage,
} from "./ProviderStageCard";
import { ProviderStageFields } from "./ProviderStageFields";
import type {
  PromptTemplateKind,
  ProviderConfig,
  ProviderConfigView,
  ProviderKind,
} from "./workbenchTypes";

/** SOP 当前步骤的 Provider 选择与配置面板，只接收页面拥有的状态和动作。 */
interface ProviderWorkflowPanelProps {
  activeStage: PromptTemplateKind;
  activeStageLabel: string;
  actualImageEndpoint: string | null;
  apiKey: string;
  availableStageProviders: ProviderConfigView[];
  baseUrlInvalid: boolean;
  beginAdd: (template: ProviderConfig) => void;
  beginStageAssignmentEdit: (provider: ProviderConfigView) => void;
  beginStageEdit: (provider: ProviderConfigView) => void;
  clearEditor: () => void;
  currentDefault: ProviderConfigView | undefined;
  draft: ProviderConfig | null;
  editorMode: "stage" | "connection" | "new" | null;
  handleSave: () => Promise<void>;
  handleStageAssignment: (
    provider: ProviderConfigView,
    assigned: boolean,
  ) => Promise<void>;
  interactionLocked: boolean;
  newArkImageProviderDraft: ProviderConfig;
  newProviderDraft: ProviderConfig;
  requestVerification: (
    provider: ProviderConfigView,
    stage: ProviderVerificationStage,
  ) => void;
  saveDisabled: boolean;
  saving: boolean;
  selectedId: string | null;
  setActiveStage: (stage: PromptTemplateKind) => void;
  setApiKey: (value: string) => void;
  setError: (error: string | null) => void;
  setFeedback: (feedback: string | null) => void;
  setVerificationResult: (result: ProviderVerificationResult | null) => void;
  stageValue: (
    provider: ProviderConfig,
    stage: PromptTemplateKind,
  ) => { isDefault: boolean; model: string };
  switchNewImageProtocol: (
    kind: Extract<ProviderKind, "openAiCompatible" | "volcengineArkImage">,
  ) => void;
  updateDraft: (draft: ProviderConfig) => void;
  verificationResult: ProviderVerificationResult | null;
  verifying: { providerId: string; stage: ProviderVerificationStage } | null;
}

/** 当前步骤内保留原有切换、连接确认和保存行为的独立视图。 */
export function ProviderWorkflowPanel({
  activeStage,
  activeStageLabel,
  actualImageEndpoint,
  apiKey,
  availableStageProviders,
  baseUrlInvalid,
  beginAdd,
  beginStageAssignmentEdit,
  beginStageEdit,
  clearEditor,
  currentDefault,
  draft,
  editorMode,
  handleSave,
  handleStageAssignment,
  interactionLocked,
  newArkImageProviderDraft,
  newProviderDraft,
  requestVerification,
  saveDisabled,
  saving,
  selectedId,
  setActiveStage,
  setApiKey,
  setError,
  setFeedback,
  setVerificationResult,
  stageValue,
  switchNewImageProtocol,
  updateDraft,
  verificationResult,
  verifying,
}: ProviderWorkflowPanelProps): ReactElement {
  const { t } = useTranslation("app");
  return (
    <Paper p="lg" radius="lg" withBorder>
      <Stack gap="lg">
        <Stack gap={4}>
          <Title order={2} size="h3">
            {t("providers.workflow_title")}
          </Title>
          <Text c="dimmed" size="sm">
            {t("providers.workflow_description")}
          </Text>
        </Stack>
        <Tabs
          keepMounted={false}
          onChange={(value) => {
            if (
              value !== "article" &&
              value !== "summary" &&
              value !== "image"
            ) {
              return;
            }
            setActiveStage(value);
            clearEditor();
            setVerificationResult(null);
          }}
          value={activeStage}
        >
          <Tabs.List grow>
            <Tabs.Tab
              disabled={interactionLocked}
              leftSection={<IconArticle aria-hidden="true" size={17} />}
              value="article"
            >
              {t("providers.stage_article_ai")}
            </Tabs.Tab>
            <Tabs.Tab
              disabled={interactionLocked}
              leftSection={<IconTextCaption aria-hidden="true" size={17} />}
              value="summary"
            >
              {t("providers.stage_summary_ai")}
            </Tabs.Tab>
            <Tabs.Tab
              disabled={interactionLocked}
              leftSection={<IconPhoto aria-hidden="true" size={17} />}
              value="image"
            >
              {t("providers.stage_image_ai")}
            </Tabs.Tab>
          </Tabs.List>
          <Tabs.Panel pt="lg" value={activeStage}>
            <Stack gap="lg">
              <Group align="flex-start" justify="space-between" wrap="wrap">
                <Stack gap={4}>
                  <Title order={3} size="h4">
                    {t("providers.stage_heading", {
                      stage: activeStageLabel,
                    })}
                  </Title>
                  <Text c="dimmed" size="sm">
                    {t(`providers.stage_description.${activeStage}`)}
                  </Text>
                </Stack>
                <Group gap="sm" wrap="wrap">
                  <Badge
                    color={currentDefault ? "blue" : "gray"}
                    size="lg"
                    variant="light"
                  >
                    {t("providers.current_default", {
                      name:
                        currentDefault?.name ?? t("providers.default_not_set"),
                    })}
                  </Badge>
                  <Menu position="bottom-end" withinPortal>
                    <Menu.Target>
                      <Button
                        disabled={
                          interactionLocked ||
                          availableStageProviders.length === 0
                        }
                        leftSection={
                          <IconPlugConnected aria-hidden="true" size={17} />
                        }
                        variant="default"
                      >
                        {t("providers.choose_existing")}
                      </Button>
                    </Menu.Target>
                    <Menu.Dropdown>
                      <Menu.Label>
                        {t("providers.choose_existing_for_stage", {
                          stage: activeStageLabel,
                        })}
                      </Menu.Label>
                      {availableStageProviders.map((provider) => {
                        const value = stageValue(provider, activeStage);
                        return (
                          <Menu.Item
                            aria-label={t("providers.choose_named", {
                              name: provider.name,
                              stage: activeStageLabel,
                            })}
                            key={provider.id}
                            onClick={() => {
                              if (value.model.trim().length === 0) {
                                beginStageAssignmentEdit(provider);
                                return;
                              }
                              void handleStageAssignment(provider, true);
                            }}
                          >
                            <Stack gap={0}>
                              <Text size="sm">{provider.name}</Text>
                              <Text c="dimmed" size="xs">
                                {value.model.trim() ||
                                  t("providers.configure_before_assigning")}
                              </Text>
                              {!provider.hasApiKey ? (
                                <Text c="orange" size="xs">
                                  {t("providers.key_missing_choose_hint")}
                                </Text>
                              ) : null}
                            </Stack>
                          </Menu.Item>
                        );
                      })}
                    </Menu.Dropdown>
                  </Menu>
                  <Button
                    disabled={interactionLocked}
                    leftSection={<IconPlus aria-hidden="true" size={17} />}
                    onClick={() =>
                      beginAdd(
                        activeStage === "image"
                          ? newArkImageProviderDraft
                          : newProviderDraft,
                      )
                    }
                    variant="default"
                  >
                    {t("providers.add_for_stage", {
                      stage: activeStageLabel,
                    })}
                  </Button>
                </Group>
              </Group>

              {currentDefault === undefined ? (
                <Alert color="gray">
                  <Text>{t("providers.empty")}</Text>
                </Alert>
              ) : (
                <ProviderStageCard
                  busy={interactionLocked}
                  onConfigure={beginStageEdit}
                  onRemove={(provider) =>
                    void handleStageAssignment(provider, false)
                  }
                  onVerify={requestVerification}
                  provider={currentDefault}
                  selected={
                    editorMode === "stage" && selectedId === currentDefault.id
                  }
                  stage={activeStage}
                  verificationResult={verificationResult}
                  verifying={verifying}
                />
              )}

              {activeStage === "image" ? (
                <Alert color="blue">{t("providers.image_test_notice")}</Alert>
              ) : null}

              {(editorMode === "stage" || editorMode === "new") && draft ? (
                <Paper p="lg" radius="md" withBorder>
                  <Stack gap="md">
                    <Stack gap={4}>
                      <Title order={3} size="h4">
                        {t(
                          editorMode === "new"
                            ? "providers.new_editor_title"
                            : "providers.stage_editor_title",
                          { name: draft.name, stage: activeStageLabel },
                        )}
                      </Title>
                      <Text c="dimmed" size="sm">
                        {t(
                          editorMode === "new"
                            ? "providers.new_editor_notice"
                            : "providers.stage_editor_notice",
                          { stage: activeStageLabel },
                        )}
                      </Text>
                    </Stack>
                    {editorMode === "new" ? (
                      <>
                        <ProviderConnectionFields
                          actualImageEndpoint={actualImageEndpoint}
                          apiKey={apiKey}
                          apiKeyDescription={t("providers.form.key_store_hint")}
                          apiKeyRequired={false}
                          baseUrlError={
                            baseUrlInvalid
                              ? t(
                                  draft.kind === "volcengineArkImage"
                                    ? "providers.form.ark_base_url_invalid"
                                    : "providers.form.base_url_invalid",
                                )
                              : undefined
                          }
                          disabled={saving}
                          draft={draft}
                          onApiKeyChange={(value) => {
                            setApiKey(value);
                            setError(null);
                            setFeedback(null);
                          }}
                          onChange={updateDraft}
                          onImageProtocolChange={
                            activeStage === "image"
                              ? switchNewImageProtocol
                              : undefined
                          }
                        />
                        <Divider
                          label={t("providers.stage_settings_divider", {
                            stage: activeStageLabel,
                          })}
                        />
                      </>
                    ) : null}
                    <ProviderStageFields
                      disabled={saving}
                      draft={draft}
                      onChange={updateDraft}
                      stage={activeStage}
                    />
                    <Group justify="flex-end">
                      <Button
                        disabled={saving}
                        onClick={clearEditor}
                        variant="default"
                      >
                        {t("common.cancel")}
                      </Button>
                      <Button
                        disabled={saveDisabled}
                        leftSection={
                          <IconDeviceFloppy aria-hidden="true" size={17} />
                        }
                        loading={saving}
                        onClick={() => void handleSave()}
                      >
                        {t(
                          editorMode === "new"
                            ? "providers.save_new"
                            : "providers.save_stage",
                          { stage: activeStageLabel },
                        )}
                      </Button>
                    </Group>
                  </Stack>
                </Paper>
              ) : null}
            </Stack>
          </Tabs.Panel>
        </Tabs>
      </Stack>
    </Paper>
  );
}
