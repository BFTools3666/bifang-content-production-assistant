import type { ProviderKind } from "./workbenchTypes";

/** 解析保存后实际使用的协议端点；火山方舟只接受固定官方根地址。 */
export function normalizedProviderEndpoint(
  value: string,
  kind: ProviderKind,
): string | null {
  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:" ||
      url.hostname.length === 0 ||
      url.username.length > 0 ||
      url.password.length > 0 ||
      url.search.length > 0 ||
      url.hash.length > 0
    ) {
      return null;
    }
    if (kind === "volcengineArkImage") {
      if (
        url.hostname !== "ark.cn-beijing.volces.com" ||
        (url.port.length > 0 && url.port !== "443") ||
        !["/api/v3", "/api/v3/"].includes(url.pathname)
      ) {
        return null;
      }
      url.pathname = "/api/v3/";
    }
    const endpointPath =
      kind === "volcengineArkImage" ? "images/generations" : "chat/completions";
    if (!url.pathname.endsWith("/")) url.pathname = `${url.pathname}/`;
    return new URL(endpointPath, url).toString();
  } catch {
    return null;
  }
}
