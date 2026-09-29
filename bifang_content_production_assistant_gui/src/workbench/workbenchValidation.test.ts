import { describe, expect, it } from "vitest";

import {
  decodeArticleView,
  decodeArticleImage,
  decodeArticleImageEditorView,
  decodeArticleThumbnail,
  decodeBatchStartResult,
  decodeCommandError,
  decodePromptTemplate,
  decodePromptTemplates,
  decodeWorkbenchView,
  decodeImageEditSpec,
} from "./workbenchValidation";
import {
  decodeConnectorConfig,
  decodeConnectorConfigDeleteResult,
  decodeConnectorConfigSaveResult,
  decodeConnectorConfigView,
  decodeConnectorSyncResult,
  decodeLarkCliSetupInspection,
  decodeProviderConfig,
  decodeProviderConfigDeleteResult,
  decodeProviderConfigSaveResult,
  decodeProviderConfigView,
  decodeProviderConfigViews,
  decodeProviderImageTestResult,
} from "./workbenchConnectionValidation";

const IMAGE_TEST_RESULT = {
  byteLength: 8,
  imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
  mimeType: "image/png",
  providerId: "provider-image",
  providerRevision: 7,
};

const IMAGE_EDIT_SPEC = {
  crop: { height: 1_000_000, width: 1_000_000, x: 0, y: 0 },
  flipHorizontal: false,
  flipVertical: false,
  rotationQuarterTurns: 0,
} as const;

/** 创建与 Rust 当前序列化契约一致的阶段状态。 */
function stageProgress(overrides: Record<string, unknown> = {}) {
  return {
    activeAttemptId: null,
    artifactVersion: null,
    attempts: 0,
    failure: null,
    status: "notStarted",
    ...overrides,
  };
}

/** 创建与 Rust 当前序列化契约一致的空产物。 */
function artifacts(overrides: Record<string, unknown> = {}) {
  return {
    articleLineage: null,
    formatLineage: null,
    html: null,
    imageEditLineage: null,
    imageEditRevision: 0,
    imageLineage: null,
    imageRef: null,
    markdown: null,
    summary: null,
    summaryLineage: null,
    originalImageRef: null,
    ...overrides,
  };
}

/** 创建一行包含新追溯字段的有效文章视图。 */
function articleView(overrides: Record<string, unknown> = {}) {
  return {
    article: {
      approvedHistory: [],
      artifacts: artifacts(),
      attemptHistory: [],
      id: "article-1",
      keywords: ["SEO"],
      packageHistory: [],
      packageVersion: 1,
      requirements: "面向初学者",
      reviewStatus: "draft",
      rowVersion: 1,
      stages: {
        article: stageProgress(),
        format: stageProgress(),
        image: stageProgress(),
        summary: stageProgress(),
      },
      createdAtMs: 1_699_999_000_000,
      title: "SEO 入门",
      updatedAtMs: 1_700_000_000_000,
    },
    hasImage: false,
    overallStatus: "draft",
    ...overrides,
  };
}

/** 创建一条有效运行记录。 */
function runRecord(overrides: Record<string, unknown> = {}) {
  return {
    completedCount: 0,
    failedCount: 0,
    finishedAtMs: null,
    id: "run-1",
    isTestFixture: false,
    startedAtMs: 1_700_000_000_000,
    status: "running",
    targetStage: null,
    taskIds: ["article-1"],
    ...overrides,
  };
}

describe("workbench IPC runtime validation", () => {
  /** 聚合视图递归校验文章、运行记录和新增追溯字段。 */
  it("decodes_a_complete_workbench_view", () => {
    const value = {
      activeRun: runRecord(),
      articles: [articleView()],
      runHistory: [runRecord()],
    };

    expect(decodeWorkbenchView(value)).toBe(value);
  });

  /** 创建时间与逐字段 token 用量按当前 IPC 契约严格解码。 */
  it("decodes_created_time_and_partial_token_usage", () => {
    const view = articleView();
    (view.article as Record<string, unknown>).attemptHistory = [
      {
        failure: null,
        finishedAtMs: 1_700_000_001_250,
        id: "attempt-1",
        lineage: null,
        packageVersion: 1,
        stage: "article",
        startedAtMs: 1_700_000_000_000,
        status: "succeeded",
        tokenUsage: {
          inputTokens: 20,
          outputTokens: null,
          totalTokens: 48,
        },
      },
    ];

    expect(decodeArticleView(view)).toBe(view);
  });

  /** 单步运行只接受固定阶段，且完整 SOP 必须显式返回 null。 */
  it("decodes_and_requires_a_valid_run_target_stage", () => {
    const singleStage = runRecord({ targetStage: "summary" });
    expect(
      decodeWorkbenchView({
        activeRun: null,
        articles: [],
        runHistory: [singleStage],
      }).runHistory[0]?.targetStage,
    ).toBe("summary");

    const missingTarget = runRecord();
    delete (missingTarget as Record<string, unknown>).targetStage;
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [],
        runHistory: [missingTarget],
      }),
    ).toThrow("runRecord");
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [],
        runHistory: [runRecord({ targetStage: "publish" })],
      }),
    ).toThrow("runRecord.targetStage");
  });

  /** 图片血缘必须显式携带它读取的简介版本，缺字段时前端失败关闭。 */
  it("decodes_and_requires_image_summary_lineage_version", () => {
    const view = articleView();
    const article = view.article as Record<string, unknown>;
    const currentArtifacts = article.artifacts as Record<string, unknown>;
    currentArtifacts.imageLineage = {
      attemptId: "image-attempt-2",
      model: "image-v2",
      packageVersion: 3,
      providerId: "provider-image",
      providerRevision: 2,
      sourceArticleVersion: 3,
      sourceSummaryVersion: 3,
      templateId: "prompt-image",
      templateVersion: 4,
    };
    const payload = {
      activeRun: null,
      articles: [view],
      runHistory: [],
    };
    expect(decodeWorkbenchView(payload)).toBe(payload);

    delete (currentArtifacts.imageLineage as Record<string, unknown>)
      .sourceSummaryVersion;
    expect(() => decodeWorkbenchView(payload)).toThrow(
      "articleView.article.artifacts.imageLineage",
    );
  });

  /** 缺少后端新增字段或出现协议外字段时必须失败关闭。 */
  it("rejects_missing_or_extra_nested_workbench_fields", () => {
    const missingHasImage = articleView();
    delete (missingHasImage as Record<string, unknown>).hasImage;
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [missingHasImage],
        runHistory: [],
      }),
    ).toThrow("invalid workbench IPC response at articleView");

    const invalidArticle = articleView();
    const article = invalidArticle.article as Record<string, unknown>;
    const progress = (article.stages as Record<string, unknown>)
      .article as Record<string, unknown>;
    delete progress.activeAttemptId;
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [invalidArticle],
        runHistory: [],
      }),
    ).toThrow("articleView.article.stages.article");

    const missingCreated = articleView();
    delete (missingCreated.article as Record<string, unknown>).createdAtMs;
    expect(() => decodeArticleView(missingCreated)).toThrow(
      "invalid workbench IPC response at articleView.article",
    );

    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [],
        debug: true,
        runHistory: [],
      }),
    ).toThrow("invalid workbench IPC response at workbench");
  });

  /** 不合法枚举、非安全整数和错误失败结构不能进入页面状态。 */
  it("rejects_invalid_nested_values", () => {
    const invalidStatus = articleView({ overallStatus: "complete" });
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [invalidStatus],
        runHistory: [],
      }),
    ).toThrow("articleView.overallStatus");

    expect(() =>
      decodeWorkbenchView({
        activeRun: runRecord({ startedAtMs: Number.MAX_SAFE_INTEGER + 1 }),
        articles: [],
        runHistory: [],
      }),
    ).toThrow("runRecord.startedAtMs");

    const failed = articleView();
    const article = failed.article as Record<string, unknown>;
    const progress = (article.stages as Record<string, unknown>)
      .article as Record<string, unknown>;
    progress.failure = {
      code: "PROVIDER_TIMEOUT",
      message: "服务超时",
      retryable: true,
    };
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [failed],
        runHistory: [],
      }),
    ).toThrow("articleView.article.stages.article.failure");
  });

  /** Rust 保留的 imageRef 占位必须为 null，防止本地路径泄漏到 WebView。 */
  it("rejects_any_article_image_reference_leak", () => {
    const leaked = articleView();
    const article = leaked.article as Record<string, unknown>;
    const currentArtifacts = article.artifacts as Record<string, unknown>;
    currentArtifacts.imageRef = "/Users/example/private/article.png";

    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [leaked],
        runHistory: [],
      }),
    ).toThrow("articleView.article.artifacts.imageRef");

    currentArtifacts.imageRef = null;
    currentArtifacts.originalImageRef = "/Users/example/private/original.png";
    expect(() =>
      decodeWorkbenchView({
        activeRun: null,
        articles: [leaked],
        runHistory: [],
      }),
    ).toThrow("articleView.article.artifacts.originalImageRef");
  });

  /** 编辑规范和只读视图只接受固定字段、整数相对坐标和一致锁状态。 */
  it("strictly_decodes_the_image_editor_contract", () => {
    expect(decodeImageEditSpec(IMAGE_EDIT_SPEC)).toBe(IMAGE_EDIT_SPEC);
    expect(() =>
      decodeImageEditSpec({
        ...IMAGE_EDIT_SPEC,
        crop: { ...IMAGE_EDIT_SPEC.crop, width: 1_000_001 },
      }),
    ).toThrow("imageEditSpec.crop");
    expect(() =>
      decodeImageEditSpec({ ...IMAGE_EDIT_SPEC, rotationQuarterTurns: 4 }),
    ).toThrow("imageEditSpec.rotationQuarterTurns");
    expect(() =>
      decodeImageEditSpec({
        ...IMAGE_EDIT_SPEC,
        filePath: "/private/image.png",
      }),
    ).toThrow("imageEditSpec");

    const editor = {
      canSave: true,
      currentSpec: IMAGE_EDIT_SPEC,
      editRevision: 2,
      imageAttemptId: "image-attempt-1",
      imageDataUrl: "data:image/png;base64,iVBORw0KGgo=",
      packageVersion: 4,
      rowVersion: 9,
      saveBlockedReason: null,
      sourceHeight: 675,
      sourceWidth: 1200,
    };
    expect(decodeArticleImageEditorView(editor)).toBe(editor);
    expect(() =>
      decodeArticleImageEditorView({
        ...editor,
        imageDataUrl: "/private/image.png",
      }),
    ).toThrow("articleImage");
    expect(() =>
      decodeArticleImageEditorView({
        ...editor,
        canSave: false,
      }),
    ).toThrow("articleImageEditor");
    expect(() =>
      decodeArticleImageEditorView({
        ...editor,
        canSave: false,
        saveBlockedReason: { code: "UNEXPECTED", message: "unsafe" },
      }),
    ).toThrow("articleImageEditor.saveBlockedReason.code");
  });

  /** 批次响应除结构外还必须保持数量字段与 ID 列表一致。 */
  it("enforces_batch_result_count_consistency", () => {
    const value = {
      batchId: "batch-1",
      queuedCount: 1,
      queuedTaskIds: ["article-1"],
      skipped: [{ reason: "alreadyRunning", taskId: "article-2" }],
      skippedCount: 1,
    };
    expect(decodeBatchStartResult(value)).toBe(value);
    expect(() => decodeBatchStartResult({ ...value, queuedCount: 2 })).toThrow(
      "batchStartResult.queuedCount",
    );
    expect(() =>
      decodeBatchStartResult({
        ...value,
        skipped: [{ reason: "alreadyRunning", taskId: "article-2", raw: "x" }],
      }),
    ).toThrow("batchStartResult.skipped[0]");
  });

  /** Prompt 模板拒绝未知类型、零版本和额外字段。 */
  it("strictly_decodes_prompt_templates", () => {
    const value = {
      content: "标题：{{title}}",
      id: "prompt-1",
      kind: "article",
      name: "正文",
      version: 1,
    };
    expect(decodePromptTemplate(value)).toBe(value);
    expect(() => decodePromptTemplate({ ...value, kind: "format" })).toThrow(
      "promptTemplate.kind",
    );
    expect(() => decodePromptTemplate({ ...value, version: 0 })).toThrow(
      "promptTemplate.version",
    );
    expect(() => decodePromptTemplate({ ...value, system: "secret" })).toThrow(
      "invalid workbench IPC response at promptTemplate",
    );
  });

  /** Prompt 列表严格接收 core 派生的变量声明，并拒绝漂移或额外字段。 */
  it("strictly_decodes_prompt_template_variable_contracts", () => {
    const value = {
      allowedVariables: ["title", "keywords", "requirements"],
      content: "标题：{{title}}",
      id: "prompt-1",
      kind: "article",
      name: "正文",
      requiredVariables: ["title"],
      version: 1,
    };
    const summary = {
      ...value,
      allowedVariables: ["title", "article"],
      id: "summary",
      kind: "summary",
      requiredVariables: ["article"],
    };
    const image = {
      ...value,
      allowedVariables: ["title", "article", "summary"],
      id: "image",
      kind: "image",
      requiredVariables: ["title", "summary"],
    };
    expect(decodePromptTemplates([value, summary, image])).toEqual([
      value,
      summary,
      image,
    ]);
    expect(() => {
      const missing = { ...value } as Record<string, unknown>;
      delete missing.requiredVariables;
      decodePromptTemplates([missing, summary, image]);
    }).toThrow("promptTemplates[0]");
    expect(() =>
      decodePromptTemplates([{ ...value, debug: true }, summary, image]),
    ).toThrow("promptTemplates[0]");
    expect(() =>
      decodePromptTemplates([
        { ...value, requiredVariables: ["article"] },
        summary,
        image,
      ]),
    ).toThrow("promptTemplates[0].variables");
    expect(() =>
      decodePromptTemplates([
        { ...value, allowedVariables: ["title", "title"] },
        summary,
        image,
      ]),
    ).toThrow("promptTemplates[0].variables");

    expect(() => decodePromptTemplates([value, summary])).toThrow(
      "promptTemplates",
    );
    expect(() =>
      decodePromptTemplates([value, { ...summary, id: value.id }, image]),
    ).toThrow("promptTemplates");
    expect(() =>
      decodePromptTemplates([value, { ...summary, kind: "article" }, image]),
    ).toThrow("promptTemplates");
  });

  /** Provider 视图要求修订号且永不接受潜在密钥字段。 */
  it("strictly_decodes_provider_views_without_secrets", () => {
    const value = {
      articleModel: "article-v1",
      articleParameters: {
        maxOutputTokens: 4_096,
        temperatureMilli: 700,
        timeoutMs: 60_000,
      },
      baseUrl: "https://api.example.com/v1",
      defaultForImage: true,
      defaultForArticle: true,
      defaultForSummary: true,
      enabled: true,
      hasApiKey: true,
      id: "provider-1",
      imageModel: "image-v1",
      imageParameters: {
        size: "1536x1024",
        timeoutMs: 90_000,
        watermark: false,
      },
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "OpenAI Compatible",
      revision: 2,
      summaryModel: "summary-v1",
      summaryParameters: {
        maxOutputTokens: 512,
        temperatureMilli: 200,
        timeoutMs: 45_000,
      },
      textModel: "text-v1",
    };
    expect(decodeProviderConfigView(value)).toBe(value);
    expect(() => {
      const missingRevision = { ...value } as Record<string, unknown>;
      delete missingRevision.revision;
      decodeProviderConfigView(missingRevision);
    }).toThrow("invalid workbench IPC response at providerConfig");
    expect(() => {
      const missingEnabled = { ...value } as Record<string, unknown>;
      delete missingEnabled.enabled;
      decodeProviderConfigView(missingEnabled);
    }).toThrow("invalid workbench IPC response at providerConfig");
    expect(() =>
      decodeProviderConfigView({ ...value, apiKey: "secret" }),
    ).toThrow("invalid workbench IPC response at providerConfig");
    expect(() =>
      decodeProviderConfigView({ ...value, defaultForArticle: "true" }),
    ).toThrow("providerConfig.defaultForArticle");
    expect(() =>
      decodeProviderConfigView({
        ...value,
        imageParameters: { ...value.imageParameters, watermark: "true" },
      }),
    ).toThrow("providerConfig.imageParameters.watermark");
    expect(() =>
      decodeProviderConfigView({
        ...value,
        articleParameters: {
          ...value.articleParameters,
          temperatureMilli: 2_001,
        },
      }),
    ).toThrow("providerConfig.articleParameters");
    expect(() =>
      decodeProviderConfigView({
        ...value,
        imageParameters: { ...value.imageParameters, size: "4096x4096" },
      }),
    ).toThrow("providerConfig.imageParameters.size");
    expect(() =>
      decodeProviderConfigView({
        ...value,
        imageParameters: { ...value.imageParameters, size: "2K" },
      }),
    ).toThrow("providerConfig.imageParameters.size");
    expect(() =>
      decodeProviderConfigView({
        ...value,
        imageParameters: { ...value.imageParameters, watermark: true },
      }),
    ).toThrow("providerConfig.imageParameters.watermark");
    expect(() =>
      decodeProviderConfigView({
        ...value,
        kind: "volcengineArkImage",
        imageParameters: {
          ...value.imageParameters,
          size: "1536x1024",
        },
      }),
    ).toThrow("providerConfig.imageParameters.size");
    for (const maxOutputTokens of [0, 128_001]) {
      expect(() =>
        decodeProviderConfigView({
          ...value,
          articleParameters: {
            ...value.articleParameters,
            maxOutputTokens,
          },
        }),
      ).toThrow("providerConfig.articleParameters");
    }
    for (const timeoutMs of [999, 300_001]) {
      expect(() =>
        decodeProviderConfigView({
          ...value,
          summaryParameters: {
            ...value.summaryParameters,
            timeoutMs,
          },
        }),
      ).toThrow("providerConfig.summaryParameters");
      expect(() =>
        decodeProviderConfigView({
          ...value,
          imageParameters: { ...value.imageParameters, timeoutMs },
        }),
      ).toThrow("providerConfig.imageParameters");
    }
    expect(() => decodeProviderConfigViews([value, { ...value }])).toThrow(
      "providerConfigs",
    );
  });

  /** Core 新建草稿允许空稳定 ID，但仍拒绝秘密或额外 IPC 字段。 */
  it("decodes_an_unsaved_provider_draft_without_accepting_secrets", () => {
    const value = {
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

    expect(decodeProviderConfig(value)).toBe(value);
    expect(() => decodeProviderConfig({ ...value, apiKey: "secret" })).toThrow(
      "invalid workbench IPC response at providerConfig",
    );
    expect(
      decodeProviderConfig({
        ...value,
        baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
        defaultForArticle: false,
        defaultForSummary: false,
        imageModel: "ep-20260318093407-x5qm5",
        imageParameters: { size: "2K", timeoutMs: 90_000, watermark: true },
        kind: "volcengineArkImage",
        name: "火山方舟图片",
      }).kind,
    ).toBe("volcengineArkImage");
  });

  /** 原子保存响应允许对象键顺序不同，但必须在完整列表中包含同一目标修订。 */
  it("decodes_an_atomic_provider_save_result_by_value", () => {
    const saved = {
      articleModel: "article-v2",
      articleParameters: {
        maxOutputTokens: 4_096,
        temperatureMilli: 700,
        timeoutMs: 60_000,
      },
      baseUrl: "https://api.example.com/v1",
      defaultForArticle: true,
      defaultForImage: false,
      defaultForSummary: false,
      enabled: true,
      hasApiKey: true,
      id: "provider-1",
      imageModel: "",
      imageParameters: {
        size: "1536x1024",
        timeoutMs: 90_000,
        watermark: false,
      },
      isTestFixture: false,
      kind: "openAiCompatible",
      name: "正文服务",
      revision: 3,
      summaryModel: "",
      summaryParameters: {
        maxOutputTokens: 512,
        temperatureMilli: 200,
        timeoutMs: 45_000,
      },
      textModel: "article-v2",
    };
    const sameProviderWithDifferentKeyOrder = Object.fromEntries(
      Object.entries(saved).reverse(),
    );

    expect(
      decodeProviderConfigSaveResult({
        providers: [sameProviderWithDifferentKeyOrder],
        saved,
      }),
    ).toEqual({ providers: [sameProviderWithDifferentKeyOrder], saved });
    expect(() =>
      decodeProviderConfigSaveResult({
        providers: [{ ...sameProviderWithDifferentKeyOrder, revision: 4 }],
        saved,
      }),
    ).toThrow("providerConfigSaveResult.saved");
  });

  /** 删除响应拒绝任何秘密或未知字段，并严格验证清理状态。 */
  it("strictly_decodes_provider_delete_results", () => {
    expect(
      decodeProviderConfigDeleteResult({
        credentialCleanupPending: false,
        historicalBindingsDetached: 7,
        providers: [],
      }),
    ).toEqual({
      credentialCleanupPending: false,
      historicalBindingsDetached: 7,
      providers: [],
    });
    expect(() =>
      decodeProviderConfigDeleteResult({
        credentialCleanupPending: "false",
        historicalBindingsDetached: 0,
        providers: [],
      }),
    ).toThrow("providerConfigDeleteResult.credentialCleanupPending");
    expect(() =>
      decodeProviderConfigDeleteResult({
        credentialCleanupPending: false,
        historicalBindingsDetached: "7",
        providers: [],
      }),
    ).toThrow("providerConfigDeleteResult.historicalBindingsDetached");
    expect(() =>
      decodeProviderConfigDeleteResult({
        credentialCleanupPending: false,
        historicalBindingsDetached: 0,
        providers: [],
        apiKey: "secret",
      }),
    ).toThrow("invalid workbench IPC response at providerConfigDeleteResult");
  });

  /** 图片测试预览绑定请求修订，只接受匹配 MIME、大小和魔数的内联图片。 */
  it("strictly_decodes_one_revision_bound_image_test_preview", () => {
    expect(
      decodeProviderImageTestResult(IMAGE_TEST_RESULT, "provider-image", 7),
    ).toEqual(IMAGE_TEST_RESULT);
    expect(() =>
      decodeProviderImageTestResult(
        { ...IMAGE_TEST_RESULT, providerRevision: 8 },
        "provider-image",
        7,
      ),
    ).toThrow("providerImageTestResult.providerRevision");
    expect(() =>
      decodeProviderImageTestResult(
        { ...IMAGE_TEST_RESULT, byteLength: 9 },
        "provider-image",
        7,
      ),
    ).toThrow("providerImageTestResult.byteLength");
    expect(() =>
      decodeProviderImageTestResult(
        {
          ...IMAGE_TEST_RESULT,
          imageDataUrl: "data:image/jpeg;base64,iVBORw0KGgo=",
          mimeType: "image/jpeg",
        },
        "provider-image",
        7,
      ),
    ).toThrow("providerImageTestResult.imageDataUrl");
    expect(() =>
      decodeProviderImageTestResult(
        { ...IMAGE_TEST_RESULT, imageDataUrl: "https://example.com/test.png" },
        "provider-image",
        7,
      ),
    ).toThrow("providerImageTestResult.imageDataUrl");
    expect(() =>
      decodeProviderImageTestResult(
        { ...IMAGE_TEST_RESULT, localPath: "/tmp/test.png" },
        "provider-image",
        7,
      ),
    ).toThrow("invalid workbench IPC response at providerImageTestResult");
  });

  /** 稳定命令错误仅接受 code/message，拒绝附带原始响应。 */
  it("strictly_decodes_safe_command_errors", () => {
    expect(
      decodeCommandError({ code: "PROVIDER_TIMEOUT", message: "服务超时" }),
    ).toEqual({ code: "PROVIDER_TIMEOUT", message: "服务超时" });
    expect(() =>
      decodeCommandError({
        code: "PROVIDER_TIMEOUT",
        message: "服务超时",
        rawResponse: "sensitive",
      }),
    ).toThrow("invalid workbench IPC response at commandError");
  });

  /** 图片命令只接受后端允许的受管图片 data URL 或 null。 */
  it("validates_managed_article_image_data_urls", () => {
    expect(decodeArticleImage(null)).toBeNull();
    expect(decodeArticleImage("data:image/png;base64,aGVsbG8=")).toBe(
      "data:image/png;base64,aGVsbG8=",
    );
    expect(
      decodeArticleImage("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="),
    ).toBe("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=");
    expect(() => decodeArticleImage("https://example.com/image.png")).toThrow(
      "invalid workbench IPC response at articleImage",
    );
    expect(() =>
      decodeArticleImage("data:text/html;base64,PHNjcmlwdD4="),
    ).toThrow("invalid workbench IPC response at articleImage");
  });

  /** Grid 缩略图在 MIME 校验外还拒绝超过原生层上限的 IPC 字符串。 */
  it("bounds_grid_thumbnail_data_urls", () => {
    expect(decodeArticleThumbnail("data:image/png;base64,aGVsbG8=")).toBe(
      "data:image/png;base64,aGVsbG8=",
    );
    expect(() =>
      decodeArticleThumbnail(`data:image/png;base64,${"A".repeat(360_001)}`),
    ).toThrow("invalid workbench IPC response at articleThumbnail");
  });

  /** 连接器 IPC 永不接受凭据字段，原子保存必须包含完全一致的目标修订。 */
  it("strictly_decodes_token_free_cli_target_configuration", () => {
    const config = {
      cliProfile: "team-content",
      defaultFormat: "markdown",
      enabled: true,
      folderToken: "fld_test",
      id: "connector-feishu",
      isDefault: true,
      kind: "feishuDrive",
      name: "飞书内容库",
      revision: 4,
      timeoutMs: 30_000,
    };
    const view = { ...config };

    expect(decodeConnectorConfig({ ...config, id: "" }).id).toBe("");
    expect(decodeConnectorConfigView(view)).toEqual(view);
    expect(
      decodeConnectorConfigSaveResult({ connectors: [view], saved: view }),
    ).toEqual({ connectors: [view], saved: view });
    expect(
      decodeConnectorConfigDeleteResult({
        connectors: [],
        credentialCleanupPending: false,
      }),
    ).toEqual({ connectors: [], credentialCleanupPending: false });
    expect(() =>
      decodeConnectorConfigView({ ...view, accessToken: "secret" }),
    ).toThrow("invalid workbench IPC response at connectorConfig");
    expect(
      decodeConnectorConfigView({
        ...view,
        cliProfile: "",
        enabled: false,
        isDefault: false,
      }).cliProfile,
    ).toBe("");
    expect(() =>
      decodeConnectorConfigView({ ...view, cliProfile: "" }),
    ).toThrow("invalid workbench IPC response at connectorConfig.cliProfile");
    expect(() =>
      decodeConnectorConfigSaveResult({
        connectors: [{ ...view, revision: 5 }],
        saved: view,
      }),
    ).toThrow("connectorConfigSaveResult.saved");
  });

  /** CLI 环境检查只接受非秘密状态，不接受附带 token 的响应。 */
  it("strictly_decodes_safe_lark_cli_inspection_payloads", () => {
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
    expect(decodeLarkCliSetupInspection(inspection)).toEqual(inspection);
    expect(() =>
      decodeLarkCliSetupInspection({ ...inspection, accessToken: "secret" }),
    ).toThrow("larkCliSetupInspection");
    expect(() =>
      decodeLarkCliSetupInspection({
        ...inspection,
        selectedProfileAuthorized: true,
        selectedProfileConfigured: false,
      }),
    ).toThrow("larkCliSetupInspection.selectedProfileAuthorized");
  });

  /** 同步结果绑定文章包和连接器修订，并拒绝响应夹带远端 URL 或令牌。 */
  it("binds_connector_sync_results_to_the_requested_versions", () => {
    const result = {
      connectorId: "connector-feishu",
      connectorRevision: 4,
      fileName: "文章-v3.html",
      fileToken: "file_test",
      format: "html",
      packageVersion: 3,
      reused: false,
      syncedAtMs: 1_700_000_000_000,
      taskId: "article-a",
    };
    const expected = {
      connectorId: "connector-feishu",
      connectorRevision: 4,
      packageVersion: 3,
      taskId: "article-a",
    };
    expect(decodeConnectorSyncResult(result, expected)).toEqual(result);
    expect(() =>
      decodeConnectorSyncResult({ ...result, packageVersion: 4 }, expected),
    ).toThrow("connectorSyncResult.revision");
    expect(() =>
      decodeConnectorSyncResult(
        { ...result, accessToken: "tenant-token" },
        expected,
      ),
    ).toThrow("invalid workbench IPC response at connectorSyncResult");
  });
});
