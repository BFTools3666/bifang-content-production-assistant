/** 预览只保留不产生出站或交互副作用的文章结构标签。 */
const SAFE_PREVIEW_TAGS = new Set([
  "a",
  "b",
  "blockquote",
  "br",
  "code",
  "del",
  "em",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "hr",
  "i",
  "li",
  "ol",
  "p",
  "pre",
  "s",
  "strong",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "tr",
  "ul",
]);

/** 这些标签的子树也不得进入预览，避免隐藏内容或可执行载荷。 */
const DROPPED_PREVIEW_TAGS = new Set([
  "audio",
  "base",
  "button",
  "canvas",
  "embed",
  "form",
  "iframe",
  "img",
  "input",
  "link",
  "math",
  "meta",
  "object",
  "picture",
  "script",
  "source",
  "style",
  "svg",
  "template",
  "textarea",
  "video",
]);

/** 转义 Markdown 图片替代文本中的结构字符。 */
function escapeMarkdownAlt(value: string): string {
  return value
    .replaceAll("\\", "\\\\")
    .replaceAll("[", "\\[")
    .replaceAll("]", "\\]");
}

/** 转义 HTML 文本和双引号属性值。 */
function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

/** 把配图紧跟在首个一级标题后；没有一级标题时放在正文前。 */
function insertMarkdownImage(markdown: string, image: string): string {
  const heading = /^(#\s+[^\r\n]+)(\r?\n|$)/m.exec(markdown);
  if (!heading || heading.index === undefined) return `${image}\n\n${markdown}`;
  const insertAt = heading.index + heading[0].length;
  const before = markdown.slice(0, insertAt).trimEnd();
  const after = markdown.slice(insertAt).replace(/^\r?\n/, "");
  return `${before}\n\n${image}${after ? `\n\n${after}` : ""}`;
}

/** 把配图紧跟在首个 h1 后；没有 h1 时放在 HTML 片段前。 */
function insertHtmlImage(html: string, image: string): string {
  const heading = /<\/h1\s*>/i.exec(html);
  if (!heading || heading.index === undefined) return `${image}\n${html}`;
  const insertAt = heading.index + heading[0].length;
  return `${html.slice(0, insertAt)}\n${image}\n${html.slice(insertAt)}`;
}

/** 动态建立含当前配图的自包含 Markdown，不修改数据库中的规范正文。 */
export function buildMarkdownProjection(
  markdown: string,
  imageDataUrl: string | null,
  title: string,
): string {
  if (!imageDataUrl) return markdown;
  const image = `![${escapeMarkdownAlt(title)}](${imageDataUrl})`;
  return insertMarkdownImage(markdown, image);
}

/** 动态建立含当前配图的自包含 HTML，不复制本机文件路径。 */
export function buildHtmlProjection(
  html: string,
  imageDataUrl: string | null,
  title: string,
): string {
  if (!imageDataUrl) return html;
  const escapedTitle = escapeHtml(title);
  const image = `<figure><img src="${imageDataUrl}" alt="${escapedTitle}"><figcaption>${escapedTitle}</figcaption></figure>`;
  return insertHtmlImage(html, image);
}

/** 为源码界面建立折叠后的图片引用；复制动作仍使用完整 Data URL。 */
export function compactImageDataUrl(
  imageDataUrl: string | null,
): string | null {
  if (!imageDataUrl) return null;
  const prefix = /^(data:image\/[a-z0-9.+-]+;base64,)/i.exec(imageDataUrl)?.[1];
  return `${prefix ?? "data:image;base64,"}…`;
}

/** 递归复制白名单结构，丢弃所有原始属性、图片和主动内容。 */
function sanitizePreviewHtml(html: string): string {
  const parsed = new DOMParser().parseFromString(html, "text/html");
  const output = document.implementation.createHTMLDocument("");
  const container = output.createElement("div");

  const appendSafeNode = (node: Node, parent: Node): void => {
    if (node.nodeType === Node.TEXT_NODE) {
      parent.appendChild(output.createTextNode(node.textContent ?? ""));
      return;
    }
    if (!(node instanceof Element)) return;
    const tagName = node.tagName.toLowerCase();
    if (DROPPED_PREVIEW_TAGS.has(tagName)) return;
    if (!SAFE_PREVIEW_TAGS.has(tagName)) {
      for (const child of node.childNodes) appendSafeNode(child, parent);
      return;
    }
    const safeElement = output.createElement(tagName);
    for (const child of node.childNodes) appendSafeNode(child, safeElement);
    parent.appendChild(safeElement);
  };

  for (const child of parsed.body.childNodes) appendSafeNode(child, container);
  return container.innerHTML;
}

/** 在无脚本、无出站的独立文档中解释格式产物。 */
export function buildSafePreviewDocument(
  html: string,
  imageDataUrl: string | null,
  title: string,
): string {
  const safeBody = sanitizePreviewHtml(html);
  const image = imageDataUrl
    ? `<figure class="article-image"><img src="${imageDataUrl}" alt="${escapeHtml(title)}"><figcaption>${escapeHtml(title)}</figcaption></figure>`
    : "";
  const body = insertHtmlImage(safeBody, image);
  return `<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8">
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; script-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'">
    <meta name="referrer" content="no-referrer">
    <style>
      :root { color-scheme: light dark; font-family: system-ui, sans-serif; line-height: 1.7; }
      body { box-sizing: border-box; max-width: 860px; margin: 0 auto; padding: 24px; overflow-wrap: anywhere; }
      h1, h2, h3, h4, h5, h6 { line-height: 1.3; }
      .article-image { margin: 20px 0; }
      .article-image img { display: block; width: 100%; max-height: 520px; object-fit: contain; border-radius: 10px; }
      .article-image figcaption { margin-top: 8px; color: #667085; font-size: 13px; text-align: center; }
      pre { overflow: auto; padding: 14px; border-radius: 8px; background: rgba(127, 127, 127, 0.12); }
      code { font-family: ui-monospace, monospace; }
      blockquote { margin-inline: 0; padding-inline-start: 16px; border-inline-start: 4px solid #8b95a5; color: #667085; }
      table { width: 100%; border-collapse: collapse; }
      th, td { padding: 8px; border: 1px solid #c8cdd5; text-align: start; }
      a { color: inherit; text-decoration: underline; }
    </style>
  </head>
  <body>${body}</body>
</html>`;
}
