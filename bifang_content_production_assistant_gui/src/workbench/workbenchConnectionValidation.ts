import type {
  ConnectorConfig,
  ConnectorConfigDeleteResult,
  ConnectorConfigSaveResult,
  ConnectorConfigView,
  ConnectorConnectionResult,
  ConnectorSyncResult,
  LarkCliSetupInspection,
  ProviderConfig,
  ProviderConfigDeleteResult,
  ProviderConfigSaveResult,
  ProviderConfigView,
  ProviderConnectionResult,
  ProviderImageTestResult,
} from "./workbenchTypes";
import {
  arrayValue,
  booleanValue,
  canonicalJson,
  enumValue,
  exactRecord,
  invalid,
  nullable,
  stringValue,
  uintValue,
} from "./workbenchValidationPrimitives";
import type { JsonRecord } from "./workbenchValidationPrimitives";

/** 当前后端可以公开的 Provider 类型。 */
const PROVIDER_KINDS = new Set([
  "openAiCompatible",
  "volcengineArkImage",
  "fixture",
]);
/** 首版只开放飞书云空间原始文件连接器。 */
const CONNECTOR_KINDS = new Set(["feishuDrive"]);
/** 最终文件同步允许的两种格式。 */
const CONNECTOR_FORMATS = new Set(["markdown", "html"]);
/** 当前两种图片协议可以从 IPC 返回的尺寸。 */
const IMAGE_SIZES = new Set(["1024x1024", "1536x1024", "1024x1536", "2K"]);

/** 严格解码一个文本阶段的有界生成参数。 */
function decodeTextGenerationParameters(value: unknown, path: string): void {
  const record = exactRecord(
    value,
    ["temperatureMilli", "maxOutputTokens", "timeoutMs"],
    path,
  );
  const temperature = nullable(
    record.temperatureMilli,
    uintValue,
    `${path}.temperatureMilli`,
  );
  const maximum = nullable(
    record.maxOutputTokens,
    (input, itemPath) => uintValue(input, itemPath, true),
    `${path}.maxOutputTokens`,
  );
  const timeout = uintValue(record.timeoutMs, `${path}.timeoutMs`, true);
  if (
    (temperature !== null && temperature > 2_000) ||
    (maximum !== null && maximum > 128_000) ||
    timeout < 1_000 ||
    timeout > 300_000
  ) {
    throw invalid(path);
  }
}

/** 严格解码一个配图阶段的有界生成参数，并返回协议相关校验所需尺寸。 */
function decodeImageGenerationParameters(
  value: unknown,
  path: string,
): { size: string; watermark: boolean } {
  const record = exactRecord(value, ["size", "timeoutMs", "watermark"], path);
  const size = enumValue(record.size, IMAGE_SIZES, `${path}.size`);
  const timeout = uintValue(record.timeoutMs, `${path}.timeoutMs`, true);
  const watermark = booleanValue(record.watermark, `${path}.watermark`);
  if (timeout < 1_000 || timeout > 300_000) throw invalid(path);
  return { size, watermark };
}

const PROVIDER_CONFIG_KEYS = [
  "id",
  "name",
  "revision",
  "kind",
  "enabled",
  "baseUrl",
  "textModel",
  "articleModel",
  "summaryModel",
  "imageModel",
  "articleParameters",
  "summaryParameters",
  "imageParameters",
  "defaultForArticle",
  "defaultForSummary",
  "defaultForImage",
] as const;

/** 校验 Provider 配置共用字段；只有尚未保存的 core 草稿允许空 ID。 */
function validateProviderConfigRecord(
  record: JsonRecord,
  requireId: boolean,
): void {
  stringValue(record.id, "providerConfig.id", requireId);
  stringValue(record.name, "providerConfig.name", true);
  uintValue(record.revision, "providerConfig.revision", true);
  const kind = enumValue(record.kind, PROVIDER_KINDS, "providerConfig.kind");
  booleanValue(record.enabled, "providerConfig.enabled");
  stringValue(record.baseUrl, "providerConfig.baseUrl", kind !== "fixture");
  stringValue(record.textModel, "providerConfig.textModel");
  stringValue(record.articleModel, "providerConfig.articleModel");
  stringValue(record.summaryModel, "providerConfig.summaryModel");
  stringValue(record.imageModel, "providerConfig.imageModel");
  decodeTextGenerationParameters(
    record.articleParameters,
    "providerConfig.articleParameters",
  );
  decodeTextGenerationParameters(
    record.summaryParameters,
    "providerConfig.summaryParameters",
  );
  const imageParameters = decodeImageGenerationParameters(
    record.imageParameters,
    "providerConfig.imageParameters",
  );
  if (
    (kind === "volcengineArkImage" && imageParameters.size !== "2K") ||
    (kind === "openAiCompatible" &&
      (imageParameters.size === "2K" || imageParameters.watermark))
  ) {
    throw invalid(
      kind === "openAiCompatible" && imageParameters.watermark
        ? "providerConfig.imageParameters.watermark"
        : "providerConfig.imageParameters.size",
    );
  }
  booleanValue(record.defaultForArticle, "providerConfig.defaultForArticle");
  booleanValue(record.defaultForSummary, "providerConfig.defaultForSummary");
  booleanValue(record.defaultForImage, "providerConfig.defaultForImage");
}

/** 从未知 IPC 值严格解码 core 定义的新 Provider 草稿。 */
export function decodeProviderConfig(value: unknown): ProviderConfig {
  const record = exactRecord(value, PROVIDER_CONFIG_KEYS, "providerConfig");
  validateProviderConfigRecord(record, false);
  return value as ProviderConfig;
}

/** 从未知 IPC 值严格解码不含密钥的 Provider 配置视图。 */
export function decodeProviderConfigView(value: unknown): ProviderConfigView {
  const record = exactRecord(
    value,
    [...PROVIDER_CONFIG_KEYS, "hasApiKey", "isTestFixture"],
    "providerConfig",
  );
  validateProviderConfigRecord(record, true);
  booleanValue(record.hasApiKey, "providerConfig.hasApiKey");
  booleanValue(record.isTestFixture, "providerConfig.isTestFixture");
  return value as ProviderConfigView;
}

/** 从未知 IPC 值严格解码 Provider 配置列表。 */
export function decodeProviderConfigViews(
  value: unknown,
): ProviderConfigView[] {
  const providers = arrayValue(
    value,
    (item) => decodeProviderConfigView(item),
    "providerConfigs",
  );
  if (
    new Set(providers.map((provider) => provider.id)).size !== providers.length
  ) {
    throw invalid("providerConfigs");
  }
  return providers;
}

/** 保存响应必须包含目标修订以及同一原子更新后的完整 Provider 列表。 */
export function decodeProviderConfigSaveResult(
  value: unknown,
): ProviderConfigSaveResult {
  const record = exactRecord(
    value,
    ["saved", "providers"],
    "providerConfigSaveResult",
  );
  const saved = decodeProviderConfigView(record.saved);
  const providers = decodeProviderConfigViews(record.providers);
  const current = providers.find(
    (provider) =>
      provider.id === saved.id && provider.revision === saved.revision,
  );
  if (
    !current ||
    JSON.stringify(canonicalJson(current)) !==
      JSON.stringify(canonicalJson(saved))
  ) {
    throw invalid("providerConfigSaveResult.saved");
  }
  return { providers, saved };
}

/** 删除响应必须只包含当前 Provider 集合和非秘密凭据生命周期状态。 */
export function decodeProviderConfigDeleteResult(
  value: unknown,
): ProviderConfigDeleteResult {
  const record = exactRecord(
    value,
    ["providers", "credentialCleanupPending", "historicalBindingsDetached"],
    "providerConfigDeleteResult",
  );
  return {
    providers: decodeProviderConfigViews(record.providers),
    credentialCleanupPending: booleanValue(
      record.credentialCleanupPending,
      "providerConfigDeleteResult.credentialCleanupPending",
    ),
    historicalBindingsDetached: uintValue(
      record.historicalBindingsDetached,
      "providerConfigDeleteResult.historicalBindingsDetached",
    ),
  };
}

/** 从未知 IPC 值严格解码 Provider 连接测试结果。 */
export function decodeProviderConnectionResult(
  value: unknown,
): ProviderConnectionResult {
  const record = exactRecord(
    value,
    ["ok", "message"],
    "providerConnectionResult",
  );
  booleanValue(record.ok, "providerConnectionResult.ok");
  stringValue(record.message, "providerConnectionResult.message", true);
  return value as ProviderConnectionResult;
}

const CONNECTOR_CONFIG_KEYS = [
  "id",
  "name",
  "revision",
  "kind",
  "enabled",
  "cliProfile",
  "folderToken",
  "defaultFormat",
  "isDefault",
  "timeoutMs",
] as const;

/** 校验连接器配置共用字段；core 草稿是唯一允许空 ID 的场景。 */
function validateConnectorConfigRecord(
  record: JsonRecord,
  requireId: boolean,
): void {
  stringValue(record.id, "connectorConfig.id", requireId);
  stringValue(record.name, "connectorConfig.name", true);
  uintValue(record.revision, "connectorConfig.revision", true);
  enumValue(record.kind, CONNECTOR_KINDS, "connectorConfig.kind");
  booleanValue(record.enabled, "connectorConfig.enabled");
  const cliProfile = stringValue(
    record.cliProfile,
    "connectorConfig.cliProfile",
    false,
  );
  if (
    cliProfile.trim() !== cliProfile ||
    cliProfile.length > 128 ||
    /[\u0000-\u001f\u007f]/u.test(cliProfile) ||
    (requireId &&
      cliProfile.length === 0 &&
      (record.enabled !== false || record.isDefault !== false))
  ) {
    throw invalid("connectorConfig.cliProfile");
  }
  stringValue(record.folderToken, "connectorConfig.folderToken", requireId);
  enumValue(
    record.defaultFormat,
    CONNECTOR_FORMATS,
    "connectorConfig.defaultFormat",
  );
  booleanValue(record.isDefault, "connectorConfig.isDefault");
  const timeoutMs = uintValue(
    record.timeoutMs,
    "connectorConfig.timeoutMs",
    true,
  );
  if (timeoutMs < 1_000 || timeoutMs > 300_000) {
    throw invalid("connectorConfig.timeoutMs");
  }
}

/** 从未知 IPC 值严格解码 core 定义的新飞书连接器草稿。 */
export function decodeConnectorConfig(value: unknown): ConnectorConfig {
  const record = exactRecord(value, CONNECTOR_CONFIG_KEYS, "connectorConfig");
  validateConnectorConfigRecord(record, false);
  return value as ConnectorConfig;
}

/** 从未知 IPC 值严格解码不含 CLI token 的发布目标视图。 */
export function decodeConnectorConfigView(value: unknown): ConnectorConfigView {
  const record = exactRecord(value, CONNECTOR_CONFIG_KEYS, "connectorConfig");
  validateConnectorConfigRecord(record, true);
  return value as ConnectorConfigView;
}

/** 严格解码连接器配置列表并拒绝重复稳定 ID。 */
export function decodeConnectorConfigViews(
  value: unknown,
): ConnectorConfigView[] {
  const connectors = arrayValue(
    value,
    (item) => decodeConnectorConfigView(item),
    "connectorConfigs",
  );
  if (
    new Set(connectors.map((connector) => connector.id)).size !==
    connectors.length
  ) {
    throw invalid("connectorConfigs");
  }
  return connectors;
}

/** 保存结果必须同时包含目标修订与原子更新后的完整连接器列表。 */
export function decodeConnectorConfigSaveResult(
  value: unknown,
): ConnectorConfigSaveResult {
  const record = exactRecord(
    value,
    ["saved", "connectors"],
    "connectorConfigSaveResult",
  );
  const saved = decodeConnectorConfigView(record.saved);
  const connectors = decodeConnectorConfigViews(record.connectors);
  const current = connectors.find(
    (connector) =>
      connector.id === saved.id && connector.revision === saved.revision,
  );
  if (
    !current ||
    JSON.stringify(canonicalJson(current)) !==
      JSON.stringify(canonicalJson(saved))
  ) {
    throw invalid("connectorConfigSaveResult.saved");
  }
  return { connectors, saved };
}

/** 删除结果只暴露剩余配置和系统凭据清理状态。 */
export function decodeConnectorConfigDeleteResult(
  value: unknown,
): ConnectorConfigDeleteResult {
  const record = exactRecord(
    value,
    ["connectors", "credentialCleanupPending"],
    "connectorConfigDeleteResult",
  );
  return {
    connectors: decodeConnectorConfigViews(record.connectors),
    credentialCleanupPending: booleanValue(
      record.credentialCleanupPending,
      "connectorConfigDeleteResult.credentialCleanupPending",
    ),
  };
}

/** 严格解码一次只读连接测试结果。 */
export function decodeConnectorConnectionResult(
  value: unknown,
): ConnectorConnectionResult {
  const record = exactRecord(
    value,
    ["ok", "message"],
    "connectorConnectionResult",
  );
  booleanValue(record.ok, "connectorConnectionResult.ok");
  stringValue(record.message, "connectorConnectionResult.message", true);
  return value as ConnectorConnectionResult;
}

/** 严格解码官方 CLI 的非秘密 profile 清单与当前授权状态。 */
export function decodeLarkCliSetupInspection(
  value: unknown,
): LarkCliSetupInspection {
  const record = exactRecord(
    value,
    [
      "version",
      "profiles",
      "selectedProfileConfigured",
      "selectedProfileAuthorized",
      "selectedUserName",
      "authorizationIssueCode",
    ],
    "larkCliSetupInspection",
  );
  const version = stringValue(
    record.version,
    "larkCliSetupInspection.version",
    true,
  );
  if (!/^\d+\.\d+\.\d+$/u.test(version)) {
    throw invalid("larkCliSetupInspection.version");
  }
  const profiles = arrayValue(
    record.profiles,
    (item, path) => {
      const profile = exactRecord(
        item,
        ["name", "userName", "tokenStatus"],
        path,
      );
      const name = stringValue(profile.name, `${path}.name`, true);
      if (name.length > 128) throw invalid(`${path}.name`);
      const userName = nullable(
        profile.userName,
        stringValue,
        `${path}.userName`,
      );
      const tokenStatus = nullable(
        profile.tokenStatus,
        stringValue,
        `${path}.tokenStatus`,
      );
      if ((userName?.length ?? 0) > 256 || (tokenStatus?.length ?? 0) > 64) {
        throw invalid(path);
      }
      return { name, tokenStatus, userName };
    },
    "larkCliSetupInspection.profiles",
  );
  if (
    new Set(profiles.map((profile) => profile.name)).size !== profiles.length
  ) {
    throw invalid("larkCliSetupInspection.profiles");
  }
  const selectedProfileConfigured = booleanValue(
    record.selectedProfileConfigured,
    "larkCliSetupInspection.selectedProfileConfigured",
  );
  const selectedProfileAuthorized = booleanValue(
    record.selectedProfileAuthorized,
    "larkCliSetupInspection.selectedProfileAuthorized",
  );
  if (selectedProfileAuthorized && !selectedProfileConfigured) {
    throw invalid("larkCliSetupInspection.selectedProfileAuthorized");
  }
  const selectedUserName = nullable(
    record.selectedUserName,
    stringValue,
    "larkCliSetupInspection.selectedUserName",
  );
  const authorizationIssueCode = nullable(
    record.authorizationIssueCode,
    stringValue,
    "larkCliSetupInspection.authorizationIssueCode",
  );
  if (
    (authorizationIssueCode !== null &&
      !/^[A-Z][A-Z0-9_]*$/u.test(authorizationIssueCode)) ||
    (selectedUserName?.length ?? 0) > 256
  ) {
    throw invalid("larkCliSetupInspection");
  }
  return {
    authorizationIssueCode,
    profiles,
    selectedProfileAuthorized,
    selectedProfileConfigured,
    selectedUserName,
    version,
  };
}

/** 严格解码一次远端文件同步结果，并绑定发起时的文章与连接器修订。 */
export function decodeConnectorSyncResult(
  value: unknown,
  expected: {
    connectorId: string;
    connectorRevision: number;
    packageVersion: number;
    taskId: string;
  },
): ConnectorSyncResult {
  const record = exactRecord(
    value,
    [
      "taskId",
      "packageVersion",
      "connectorId",
      "connectorRevision",
      "format",
      "fileToken",
      "fileName",
      "syncedAtMs",
      "reused",
    ],
    "connectorSyncResult",
  );
  const taskId = stringValue(record.taskId, "connectorSyncResult.taskId", true);
  const packageVersion = uintValue(
    record.packageVersion,
    "connectorSyncResult.packageVersion",
    true,
  );
  const connectorId = stringValue(
    record.connectorId,
    "connectorSyncResult.connectorId",
    true,
  );
  const connectorRevision = uintValue(
    record.connectorRevision,
    "connectorSyncResult.connectorRevision",
    true,
  );
  enumValue(record.format, CONNECTOR_FORMATS, "connectorSyncResult.format");
  stringValue(record.fileToken, "connectorSyncResult.fileToken", true);
  stringValue(record.fileName, "connectorSyncResult.fileName", true);
  uintValue(record.syncedAtMs, "connectorSyncResult.syncedAtMs");
  booleanValue(record.reused, "connectorSyncResult.reused");
  if (
    taskId !== expected.taskId ||
    packageVersion !== expected.packageVersion ||
    connectorId !== expected.connectorId ||
    connectorRevision !== expected.connectorRevision
  ) {
    throw invalid("connectorSyncResult.revision");
  }
  return value as ConnectorSyncResult;
}

/** 严格解码一次图片测试的受限内存预览，并绑定发起测试的服务修订。 */
export function decodeProviderImageTestResult(
  value: unknown,
  expectedProviderId: string,
  expectedRevision: number,
): ProviderImageTestResult {
  const record = exactRecord(
    value,
    [
      "providerId",
      "providerRevision",
      "mimeType",
      "byteLength",
      "imageDataUrl",
    ],
    "providerImageTestResult",
  );
  const providerId = stringValue(
    record.providerId,
    "providerImageTestResult.providerId",
    true,
  );
  const providerRevision = uintValue(
    record.providerRevision,
    "providerImageTestResult.providerRevision",
    true,
  );
  if (
    providerId !== expectedProviderId ||
    providerRevision !== expectedRevision
  ) {
    throw invalid("providerImageTestResult.providerRevision");
  }
  const mimeType = stringValue(
    record.mimeType,
    "providerImageTestResult.mimeType",
    true,
  );
  if (!/^image\/(?:png|jpeg|webp)$/u.test(mimeType)) {
    throw invalid("providerImageTestResult.mimeType");
  }
  const byteLength = uintValue(
    record.byteLength,
    "providerImageTestResult.byteLength",
    true,
  );
  if (byteLength > 25 * 1024 * 1024) {
    throw invalid("providerImageTestResult.byteLength");
  }
  const imageDataUrl = stringValue(
    record.imageDataUrl,
    "providerImageTestResult.imageDataUrl",
    true,
  );
  const match =
    /^data:(image\/(?:png|jpeg|webp));base64,([A-Za-z0-9+/]+={0,2})$/u.exec(
      imageDataUrl,
    );
  const matchedMimeType = match?.[1];
  const encoded = match?.[2];
  if (
    matchedMimeType !== mimeType ||
    encoded === undefined ||
    encoded.length % 4 !== 0
  ) {
    throw invalid("providerImageTestResult.imageDataUrl");
  }
  const padding = encoded.endsWith("==") ? 2 : encoded.endsWith("=") ? 1 : 0;
  const decodedLength = (encoded.length / 4) * 3 - padding;
  if (decodedLength !== byteLength) {
    throw invalid("providerImageTestResult.byteLength");
  }
  let prefix: string;
  try {
    prefix = atob(encoded.slice(0, 24));
  } catch {
    throw invalid("providerImageTestResult.imageDataUrl");
  }
  const bytes = Array.from(prefix, (character) => character.charCodeAt(0));
  const magicMatches =
    (mimeType === "image/png" &&
      bytes.slice(0, 8).join(",") === "137,80,78,71,13,10,26,10") ||
    (mimeType === "image/jpeg" &&
      bytes[0] === 0xff &&
      bytes[1] === 0xd8 &&
      bytes[2] === 0xff) ||
    (mimeType === "image/webp" &&
      String.fromCharCode(...bytes.slice(0, 4)) === "RIFF" &&
      String.fromCharCode(...bytes.slice(8, 12)) === "WEBP");
  if (!magicMatches) {
    throw invalid("providerImageTestResult.imageDataUrl");
  }
  return {
    byteLength,
    imageDataUrl,
    mimeType: mimeType as ProviderImageTestResult["mimeType"],
    providerId,
    providerRevision,
  };
}
