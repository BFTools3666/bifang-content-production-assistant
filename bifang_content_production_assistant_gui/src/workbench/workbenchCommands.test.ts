import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import {
  deleteConnectorConfig,
  deleteProviderConfig,
  generateArticleStage,
  inspectLarkCliSetup,
  loadArticleImageEditor,
  loadArticleThumbnail,
  newFeishuConnectorConfigDraft,
  newProviderConfigDraft,
  newVolcengineArkImageProviderConfigDraft,
  previewPromptTemplate,
  previewSelectedBatch,
  saveProviderConfig,
  saveArticleImageEdit,
  saveConnectorConfig,
  setProviderEnabled,
  setProviderStageAssignment,
  testProviderImage,
  syncArticleToConnector,
  updateArticleInputs,
  verifyProviderConnection,
  verifyConnectorConnection,
} from "./workbenchCommands";

describe("workbench Tauri command boundary", () => {
  beforeEach(() => invokeMock.mockReset());

  /** Grid 缩略图只按文章 ID 读取，不向 WebView 暴露路径或可变尺寸。 */
  it("loads_a_fixed_grid_thumbnail_by_task_id", async () => {
    const thumbnail = "data:image/png;base64,aGVsbG8=";
    invokeMock.mockResolvedValueOnce(thumbnail);

    await expect(loadArticleThumbnail("article-1")).resolves.toBe(thumbnail);
    expect(invokeMock).toHaveBeenCalledWith("load_article_thumbnail", {
      taskId: "article-1",
    });
  });

  /** 编辑器读取绝不接受路径，保存只发送任务锁与固定编辑规范。 */
  it("routes_the_narrow_image_editor_contract", async () => {
    const spec = {
      crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
      flipHorizontal: false,
      flipVertical: true,
      rotationQuarterTurns: 1 as const,
    };
    invokeMock.mockResolvedValueOnce({
      canSave: true,
      currentSpec: spec,
      editRevision: 2,
      imageAttemptId: "image-attempt-1",
      imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
      packageVersion: 4,
      rowVersion: 9,
      saveBlockedReason: null,
      sourceHeight: 675,
      sourceWidth: 1200,
    });
    invokeMock.mockResolvedValueOnce(null);

    await expect(loadArticleImageEditor("article-1")).resolves.toMatchObject({
      imageAttemptId: "image-attempt-1",
    });
    await expect(
      saveArticleImageEdit("article-1", 9, 4, "image-attempt-1", 2, spec),
    ).rejects.toThrow("articleView");
    expect(invokeMock.mock.calls).toEqual([
      ["load_article_image_editor", { taskId: "article-1" }],
      [
        "save_article_image_edit",
        {
          expectedEditRevision: 2,
          expectedImageAttemptId: "image-attempt-1",
          expectedPackageVersion: 4,
          expectedRowVersion: 9,
          spec,
          taskId: "article-1",
        },
      ],
    ]);

    await expect(
      saveArticleImageEdit("article-1", 9, 4, "image-attempt-1", 2, {
        ...spec,
        rotationQuarterTurns: 4 as never,
      }),
    ).rejects.toThrow("imageEditSpec.rotationQuarterTurns");
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  /** 批次预检和共享单步生成使用精确 Tauri camelCase 参数。 */
  it("uses_exact_batch_and_stage_arguments", async () => {
    invokeMock.mockResolvedValueOnce({
      batchId: null,
      queuedCount: 2,
      queuedTaskIds: ["a", "b"],
      skipped: [],
      skippedCount: 0,
    });
    invokeMock.mockResolvedValueOnce({
      batchId: "batch-2",
      queuedCount: 1,
      queuedTaskIds: ["a"],
      skipped: [],
      skippedCount: 0,
    });

    await previewSelectedBatch(["a", "b"]);
    await generateArticleStage("a", "image", 9, 3, true, "ark-image", 7);

    expect(invokeMock.mock.calls).toEqual([
      ["preview_selected_batch", { taskIds: ["a", "b"] }],
      [
        "generate_article_stage",
        {
          confirmed: true,
          expectedPackageVersion: 3,
          expectedProviderId: "ark-image",
          expectedProviderRevision: 7,
          expectedRowVersion: 9,
          stage: "image",
          taskId: "a",
        },
      ],
    ]);
  });

  /** Prompt 预览只调用本地 Tauri 命令，并精确传递模板和示例变量。 */
  it("routes_prompt_preview_only_to_the_local_command", async () => {
    const template = {
      content: "撰写 {{title}}",
      id: "article",
      kind: "article" as const,
      name: "正文",
      version: 3,
    };
    const values = { title: "本地示例" };
    invokeMock.mockResolvedValueOnce({ rendered: "撰写 本地示例" });

    await expect(previewPromptTemplate(template, values)).resolves.toBe(
      "撰写 本地示例",
    );
    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("preview_prompt_template", {
      template,
      values,
    });
  });

  /** 新建服务表单从 core 读取默认值，允许保存前稳定 ID 为空。 */
  it("loads_the_new_provider_draft_from_core", async () => {
    const draft = {
      articleModel: "",
      articleParameters: {
        maxOutputTokens: null,
        temperatureMilli: null,
        timeoutMs: 60_000,
      },
      baseUrl: "https://api.openai.com/v1",
      defaultForArticle: true,
      defaultForImage: true,
      defaultForSummary: true,
      enabled: true,
      id: "",
      imageModel: "",
      imageParameters: {
        size: "1536x1024",
        timeoutMs: 90_000,
        watermark: false,
      },
      kind: "openAiCompatible",
      name: "OpenAI Compatible",
      revision: 1,
      summaryModel: "",
      summaryParameters: {
        maxOutputTokens: null,
        temperatureMilli: null,
        timeoutMs: 60_000,
      },
      textModel: "",
    };
    invokeMock.mockResolvedValueOnce(draft);

    await expect(newProviderConfigDraft()).resolves.toEqual(draft);
    expect(invokeMock).toHaveBeenCalledWith("new_provider_config_draft");
  });

  /** 火山方舟图片草稿使用独立窄命令并经过严格解码。 */
  it("loads_the_volcengine_ark_image_draft_from_core", async () => {
    const draft = {
      articleModel: "",
      articleParameters: {
        maxOutputTokens: null,
        temperatureMilli: null,
        timeoutMs: 60_000,
      },
      baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
      defaultForArticle: false,
      defaultForImage: true,
      defaultForSummary: false,
      enabled: true,
      id: "",
      imageModel: "ep-20260318093407-x5qm5",
      imageParameters: { size: "2K", timeoutMs: 90_000, watermark: true },
      kind: "volcengineArkImage",
      name: "火山方舟图片",
      revision: 1,
      summaryModel: "",
      summaryParameters: {
        maxOutputTokens: null,
        temperatureMilli: null,
        timeoutMs: 60_000,
      },
      textModel: "",
    };
    invokeMock.mockResolvedValueOnce(draft);

    await expect(newVolcengineArkImageProviderConfigDraft()).resolves.toEqual(
      draft,
    );
    expect(invokeMock).toHaveBeenCalledWith(
      "new_volcengine_ark_image_provider_config_draft",
    );
  });

  /** 连接测试必须携带明确文本阶段，不能退回共享文本模型。 */
  it("routes_provider_verification_to_one_explicit_text_stage", async () => {
    invokeMock.mockResolvedValueOnce({ message: "ok", ok: true });

    await expect(
      verifyProviderConnection("provider-1", "summary"),
    ).resolves.toEqual({ message: "ok", ok: true });
    expect(invokeMock).toHaveBeenCalledWith("verify_provider_connection", {
      providerId: "provider-1",
      stage: "summary",
    });
  });

  /** 图片测试使用独立付费命令，并绑定用户确认时看到的配置修订。 */
  it("routes_one_confirmed_image_test_to_the_saved_provider_revision", async () => {
    const result = {
      byteLength: 8,
      imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
      mimeType: "image/png",
      providerId: "provider-image",
      providerRevision: 7,
    };
    invokeMock.mockResolvedValueOnce(result);

    await expect(testProviderImage("provider-image", 7)).resolves.toEqual(
      result,
    );
    expect(invokeMock).toHaveBeenCalledWith("test_provider_image", {
      confirmed: true,
      expectedRevision: 7,
      providerId: "provider-image",
    });
  });

  /** Provider 生命周期命令必须携带稳定 ID、期望修订和明确删除确认。 */
  it("routes_provider_lifecycle_with_revision_guards", async () => {
    const provider = {
      articleModel: "gpt-4o-mini",
      articleParameters: {
        maxOutputTokens: null,
        temperatureMilli: null,
        timeoutMs: 60_000,
      },
      baseUrl: "https://api.openai.com/v1",
      defaultForArticle: false,
      defaultForImage: false,
      defaultForSummary: false,
      enabled: false,
      hasApiKey: true,
      id: "provider-1",
      imageModel: "gpt-image",
      imageParameters: {
        size: "1536x1024",
        timeoutMs: 180_000,
        watermark: false,
      },
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "OpenAI",
      revision: 4,
      summaryModel: "gpt-4o-mini",
      summaryParameters: {
        maxOutputTokens: null,
        temperatureMilli: null,
        timeoutMs: 60_000,
      },
      textModel: "gpt-4o-mini",
    };
    invokeMock.mockResolvedValueOnce({
      providers: [provider],
      saved: provider,
    });
    invokeMock.mockResolvedValueOnce({
      credentialCleanupPending: false,
      historicalBindingsDetached: 0,
      providers: [],
    });
    invokeMock.mockResolvedValueOnce({
      providers: [provider],
      saved: provider,
    });

    await setProviderEnabled("provider-1", 3, false);
    await deleteProviderConfig("provider-1", 4);
    await setProviderStageAssignment("provider-1", 4, "summary", true);

    expect(invokeMock.mock.calls).toEqual([
      [
        "set_provider_enabled",
        {
          enabled: false,
          expectedRevision: 3,
          providerId: "provider-1",
        },
      ],
      [
        "delete_provider_config",
        {
          confirmed: true,
          expectedRevision: 4,
          providerId: "provider-1",
        },
      ],
      [
        "set_provider_stage_assignment",
        {
          assigned: true,
          expectedRevision: 4,
          providerId: "provider-1",
          stage: "summary",
        },
      ],
    ]);
  });

  /** 行级输入和系统凭据写入只通过窄命令传递明确字段。 */
  it("keeps_inputs_and_credentials_on_narrow_commands", async () => {
    const config = {
      articleModel: "article-model",
      articleParameters: {
        maxOutputTokens: 4_096,
        temperatureMilli: 700,
        timeoutMs: 60_000,
      },
      baseUrl: "https://api.example.com/v1",
      defaultForArticle: true,
      defaultForImage: true,
      defaultForSummary: true,
      enabled: true,
      id: "provider-1",
      imageModel: "image-model",
      imageParameters: {
        size: "1536x1024" as const,
        timeoutMs: 90_000,
        watermark: false,
      },
      kind: "openAiCompatible" as const,
      name: "示例服务",
      revision: 1,
      summaryModel: "summary-model",
      summaryParameters: {
        maxOutputTokens: 512,
        temperatureMilli: 200,
        timeoutMs: 45_000,
      },
      textModel: "text-model",
    };
    invokeMock.mockResolvedValueOnce({});
    const saved = {
      ...config,
      hasApiKey: true,
      isTestFixture: false,
    };
    invokeMock.mockResolvedValueOnce({ providers: [saved], saved });
    await expect(
      updateArticleInputs("a", ["SEO", "AI"], "面向初学者"),
    ).rejects.toThrow("invalid workbench IPC response");
    await saveProviderConfig(config, "secret-value");

    expect(invokeMock).toHaveBeenNthCalledWith(1, "update_article_inputs", {
      keywords: ["SEO", "AI"],
      requirements: "面向初学者",
      taskId: "a",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "save_provider_config", {
      apiKey: "secret-value",
      config,
    });
  });

  /** CLI 环境页只路由显式只读检测，不暴露 profile 写入或登录命令。 */
  it("routes_only_the_read_only_cli_inspection", async () => {
    const inspection = {
      authorizationIssueCode: null,
      profiles: [
        { name: "seo-publisher", tokenStatus: "valid", userName: "测试用户" },
      ],
      selectedProfileAuthorized: true,
      selectedProfileConfigured: true,
      selectedUserName: "测试用户",
      version: "1.0.96",
    };
    invokeMock.mockResolvedValueOnce(inspection);

    await expect(inspectLarkCliSetup("seo-publisher")).resolves.toEqual(
      inspection,
    );
    expect(invokeMock.mock.calls).toEqual([
      ["inspect_lark_cli_setup", { profile: "seo-publisher" }],
    ]);
  });

  /** CLI 发布目标、只读测试与远端写入分别通过窄命令和完整乐观锁路由。 */
  it("routes_connector_configuration_test_and_sync_through_narrow_commands", async () => {
    const config = {
      cliProfile: "team-content",
      defaultFormat: "markdown" as const,
      enabled: true,
      folderToken: "fld_test",
      id: "connector-feishu",
      isDefault: true,
      kind: "feishuDrive" as const,
      name: "飞书内容库",
      revision: 4,
      timeoutMs: 30_000,
    };
    const view = { ...config };
    const draft = {
      ...config,
      cliProfile: "",
      folderToken: "",
      id: "",
      revision: 1,
    };
    const syncResult = {
      connectorId: config.id,
      connectorRevision: config.revision,
      fileName: "文章-v3.md",
      fileToken: "file_test",
      format: "markdown" as const,
      packageVersion: 3,
      reused: false,
      syncedAtMs: 1_700_000_000_000,
      taskId: "article-a",
    };
    invokeMock.mockResolvedValueOnce(draft);
    invokeMock.mockResolvedValueOnce({ connectors: [view], saved: view });
    invokeMock.mockResolvedValueOnce({ message: "ok", ok: true });
    invokeMock.mockResolvedValueOnce(syncResult);
    invokeMock.mockResolvedValueOnce({
      connectors: [],
      credentialCleanupPending: false,
    });

    await newFeishuConnectorConfigDraft();
    await saveConnectorConfig(config);
    await verifyConnectorConnection(config.id);
    await syncArticleToConnector("article-a", config.id, "markdown", 7, 3, 4);
    await deleteConnectorConfig(config.id, 4);

    expect(invokeMock.mock.calls).toEqual([
      ["new_feishu_connector_config_draft"],
      ["save_connector_config", { config }],
      ["verify_connector_connection", { connectorId: config.id }],
      [
        "sync_article_to_connector",
        {
          confirmed: true,
          connectorId: config.id,
          expectedConnectorRevision: 4,
          expectedPackageVersion: 3,
          expectedRowVersion: 7,
          format: "markdown",
          taskId: "article-a",
        },
      ],
      [
        "delete_connector_config",
        {
          confirmed: true,
          connectorId: config.id,
          expectedRevision: 4,
        },
      ],
    ]);
  });
});
