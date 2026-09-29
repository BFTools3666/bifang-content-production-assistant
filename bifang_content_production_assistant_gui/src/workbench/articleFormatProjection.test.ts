import { describe, expect, it } from "vitest";

import {
  buildHtmlProjection,
  buildMarkdownProjection,
  buildSafePreviewDocument,
  compactImageDataUrl,
} from "./articleFormatProjection";

const IMAGE = "data:image/png;base64,aGVsbG8=";

describe("article format projection", () => {
  /** Markdown 投影紧跟标题嵌图并正确转义替代文本。 */
  it("embeds_the_managed_image_in_markdown", () => {
    const projection = buildMarkdownProjection(
      "# 标题\n\n正文",
      IMAGE,
      "标题 ] \\ 测试",
    );

    expect(projection).toBe(
      `# 标题\n\n![标题 \\] \\\\ 测试](${IMAGE})\n\n正文`,
    );
    expect(compactImageDataUrl(IMAGE)).toBe("data:image/png;base64,…");
  });

  /** HTML 投影只使用受管 data URL，并转义图片文本。 */
  it("embeds_the_managed_image_in_html", () => {
    const projection = buildHtmlProjection(
      "<h1>标题</h1><p>正文</p>",
      IMAGE,
      '标题 "<&',
    );

    expect(projection).toContain(`src="${IMAGE}"`);
    expect(projection).toContain('alt="标题 &quot;&lt;&amp;"');
    expect(projection.indexOf("</h1>")).toBeLessThan(
      projection.indexOf("<figure>"),
    );
    expect(projection.indexOf("<figure>")).toBeLessThan(
      projection.indexOf("<p>正文</p>"),
    );
  });

  /** 解释后预览丢弃主动内容和全部原始属性，只留下安全内容结构。 */
  it("builds_a_sandbox_document_without_active_or_remote_content", () => {
    const preview = buildSafePreviewDocument(
      '<h1 onclick="steal()">标题</h1><p>正文 <a href="https://evil.test">链接</a></p><img src="https://evil.test/a.png"><script>steal()</script><form action="https://evil.test"></form>',
      IMAGE,
      "安全标题",
    );

    expect(preview).toContain("Content-Security-Policy");
    expect(preview).toContain(`src="${IMAGE}"`);
    expect(preview).toContain("<h1>标题</h1>");
    expect(preview).toContain("<a>链接</a>");
    expect(preview).not.toContain("onclick");
    expect(preview).not.toContain("https://evil.test");
    expect(preview).not.toContain("<script>");
    expect(preview).not.toContain("<form");
  });
});
