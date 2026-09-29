import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import brandEnUS from "./brand-support/i18n/en-US.json";
import brandZhCN from "./brand-support/i18n/zh-CN.json";
import appEnUS from "./i18n/en-US.json";
import appZhCN from "./i18n/zh-CN.json";
import { getSystemLocale } from "./tauriCommands";
import type { SupportedInterfaceLanguage } from "./brand-support/SettingsPageTemplate";

/** 把当前 i18next 语言同步到文档根，供辅助技术识别。 */
function synchronizeDocumentLanguage(language: string): void {
  document.documentElement.lang = language.toLowerCase().startsWith("zh")
    ? "zh-CN"
    : "en-US";
}

/** 初始化唯一 i18next 实例，宿主探测失败时安全回退英文。 */
export async function initializeI18n(): Promise<SupportedInterfaceLanguage> {
  let language: SupportedInterfaceLanguage = "en-US";
  try {
    language = await getSystemLocale();
  } catch {
    language = "en-US";
  }
  if (!i18n.isInitialized) {
    await i18n.use(initReactI18next).init({
      fallbackLng: "en-US",
      interpolation: { escapeValue: false },
      lng: language,
      ns: ["app", "brandSupport"],
      defaultNS: "app",
      resources: {
        "en-US": { app: appEnUS, brandSupport: brandEnUS },
        "zh-CN": { app: appZhCN, brandSupport: brandZhCN },
      },
      returnNull: false,
    });
  } else {
    await i18n.changeLanguage(language);
  }
  synchronizeDocumentLanguage(language);
  i18n.off("languageChanged", synchronizeDocumentLanguage);
  i18n.on("languageChanged", synchronizeDocumentLanguage);
  return language;
}

export default i18n;
