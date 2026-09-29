import { describe, expect, it } from "vitest";

import { normalizedProviderEndpoint } from "./providerEndpoint";

describe("provider endpoints", () => {
  /** 方舟图片协议只接受官方北京根地址，并由界面派生固定生成路径。 */
  it("accepts_only_the_fixed_volcengine_ark_image_base_url", () => {
    for (const baseUrl of [
      "https://ark.cn-beijing.volces.com/api/v3",
      "https://ark.cn-beijing.volces.com/api/v3/",
      "https://ark.cn-beijing.volces.com:443/api/v3",
    ]) {
      expect(normalizedProviderEndpoint(baseUrl, "volcengineArkImage")).toBe(
        "https://ark.cn-beijing.volces.com/api/v3/images/generations",
      );
    }

    for (const baseUrl of [
      "http://ark.cn-beijing.volces.com/api/v3",
      "https://example.com/api/v3",
      "https://ark.cn-beijing.volces.com:444/api/v3",
      "https://ark.cn-beijing.volces.com/api/v3//",
      "https://ark.cn-beijing.volces.com/api/v3/extra",
      "https://ark.cn-beijing.volces.com/api/v3/images/generations",
      "https://ark.cn-beijing.volces.com/api/v3?tenant=1",
      "https://user:secret@ark.cn-beijing.volces.com/api/v3",
    ]) {
      expect(
        normalizedProviderEndpoint(baseUrl, "volcengineArkImage"),
      ).toBeNull();
    }
  });

  /** OpenAI-compatible 连接继续允许用户选择可信任的 HTTPS 根地址。 */
  it("keeps_openai_compatible_base_urls_configurable", () => {
    expect(
      normalizedProviderEndpoint(
        "https://self-hosted.example/v1",
        "openAiCompatible",
      ),
    ).toBe("https://self-hosted.example/v1/chat/completions");
  });
});
