import type { TFunction } from "i18next";

import type { Failure } from "./workbenchTypes";

/** 仅从后端拒绝值读取可供本地翻译分支使用的稳定错误码。 */
export function commandErrorCode(error: unknown): string | null {
  if (typeof error !== "object" || error === null) return null;
  const code = Reflect.get(error, "code");
  return typeof code === "string" && /^[A-Z][A-Z0-9_]*$/u.test(code)
    ? code
    : null;
}

/** 用前端当前语言解释命令错误，不把后端语言文案直接带入其他语言界面。 */
export function localizedCommandError(
  error: unknown,
  t: TFunction<"app">,
): string {
  const code = commandErrorCode(error);
  if (!code) return t("common.failed");
  return t(`errors.${code}`, { defaultValue: t("common.failed") });
}

/** 用稳定失败码展示行级阶段错误，忽略后端携带的固定语言 message。 */
export function localizedFailure(
  failure: Failure,
  t: TFunction<"app">,
): string {
  return t(`errors.${failure.code}`, { defaultValue: t("errors.UNKNOWN") });
}
