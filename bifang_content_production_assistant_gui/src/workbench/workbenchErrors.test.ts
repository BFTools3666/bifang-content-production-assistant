import { createInstance } from "i18next";
import { describe, expect, it } from "vitest";

import enUS from "../i18n/en-US.json";
import zhCN from "../i18n/zh-CN.json";
import {
  commandErrorCode,
  localizedCommandError,
  localizedFailure,
} from "./workbenchErrors";

/** 创建只加载应用文案的隔离 i18n 实例。 */
async function translator(locale: "en-US" | "zh-CN") {
  const instance = createInstance();
  await instance.init({
    defaultNS: "app",
    fallbackLng: "en-US",
    lng: locale,
    ns: ["app"],
    resources: {
      "en-US": { app: enUS },
      "zh-CN": { app: zhCN },
    },
  });
  return instance.getFixedT(locale, "app");
}

describe("localized workbench failures", () => {
  /** 批次配置拒绝必须明确指出阶段和恢复入口，不回显原始凭据错误。 */
  it("explains_each_missing_batch_stage_and_its_configuration_entry", async () => {
    const zh = await translator("zh-CN");
    const en = await translator("en-US");
    for (const [stage, chinese, english] of [
      ["ARTICLE", "正文", "article"],
      ["SUMMARY", "简介", "summary"],
      ["IMAGE", "配图", "image"],
    ]) {
      const error = {
        code: `BATCH_${stage}_PROVIDER_MISSING`,
        message: "raw secret must never be displayed",
      };
      expect(localizedCommandError(error, zh)).toContain(chinese);
      expect(localizedCommandError(error, zh)).toContain("AI 服务");
      expect(localizedCommandError(error, en)).toContain(english);
      expect(localizedCommandError(error, en)).not.toContain("raw secret");
    }
    expect(
      localizedCommandError({ code: "BATCH_IMAGE_CREDENTIAL_MISSING" }, zh),
    ).toContain("API Key");
    expect(
      localizedCommandError({ code: "BATCH_SUMMARY_PROMPT_MISSING" }, zh),
    ).toContain("提示词");
  });

  /** 英文界面只依据稳定 code 翻译，不显示后端固定的中文 message。 */
  it("does_not_leak_backend_chinese_into_the_english_ui", async () => {
    const t = await translator("en-US");
    expect(
      localizedCommandError(
        { code: "PROVIDER_TIMEOUT", message: "服务响应超时" },
        t,
      ),
    ).toBe("The AI service timed out. Try again later.");
    expect(
      localizedFailure(
        {
          code: "PROVIDER_RATE_LIMITED",
          message: "AI 服务繁忙，请稍后重试",
          retryAfterMs: 2_000,
          retryable: true,
        },
        t,
      ),
    ).toBe("The AI service is busy. Try again later.");
  });

  /** 非稳定 code 或协议外错误使用本地兜底，不回显未知拒绝内容。 */
  it("fails_closed_for_unknown_rejection_shapes", async () => {
    const t = await translator("zh-CN");
    expect(commandErrorCode({ code: "not-stable", message: "raw" })).toBeNull();
    expect(localizedCommandError("服务端原始错误", t)).toBe(
      "操作没有完成，请重试。",
    );
    expect(
      localizedCommandError(
        new Error("invalid workbench IPC response at workbench.articles"),
        t,
      ),
    ).toBe("操作没有完成，请重试。");
    expect(
      localizedCommandError({ code: "UNMAPPED_CODE", message: "raw" }, t),
    ).toBe("操作没有完成，请重试。");
  });

  /** 缺失旧包冻结绑定时，两种语言都给出明确的“重新生成”恢复动作。 */
  it("localizes_the_missing_execution_binding_recovery_action", async () => {
    const zh = await translator("zh-CN");
    const en = await translator("en-US");
    const error = {
      code: "EXECUTION_BINDING_MISSING",
      message: "后端固定语言不得直接显示",
    };

    expect(localizedCommandError(error, zh)).toContain("重新生成");
    expect(localizedCommandError(error, en)).toContain("Regenerate");
    expect(localizedCommandError(error, en)).not.toContain("后端固定语言");
  });

  /** Provider 修订耗尽使用双语稳定文案，不退化为泛化错误。 */
  it("localizes_provider_revision_exhaustion", async () => {
    const zh = await translator("zh-CN");
    const en = await translator("en-US");
    const error = {
      code: "PROVIDER_REVISION_EXHAUSTED",
      message: "后端固定语言不得直接显示",
    };

    expect(localizedCommandError(error, zh)).toContain("修订号已达到上限");
    expect(localizedCommandError(error, en)).toContain("revision limit");
  });

  /** CLI 网络错误使用网络修复指引，不能误报为可执行文件未安装。 */
  it("localizes_connector_network_failures_without_an_install_hint", async () => {
    const zh = await translator("zh-CN");
    const en = await translator("en-US");
    const error = {
      code: "CONNECTOR_NETWORK_FAILED",
      message: "后端固定语言不得直接显示",
    };

    expect(localizedCommandError(error, zh)).toContain("检查网络");
    expect(localizedCommandError(error, en)).toContain("Check the network");
    expect(localizedCommandError(error, en)).not.toContain("install");
  });
});
