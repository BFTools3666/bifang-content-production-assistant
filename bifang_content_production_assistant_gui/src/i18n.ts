import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import brandEnUS from "./brand-support/i18n/en-US.json";
import brandZhCN from "./brand-support/i18n/zh-CN.json";
import appEnUS from "./i18n/en-US.json";
import appZhCN from "./i18n/zh-CN.json";
import researchEnUS from "./research/i18n/en-US.json";
import researchZhCN from "./research/i18n/zh-CN.json";
import { getSystemLocale } from "./tauriCommands";
import type { SupportedInterfaceLanguage } from "./brand-support/SettingsPageTemplate";

/** 仅替换笔记发行版的可见文案，保留原工作台协议和翻译键。 */
function editionMessages<T>(messages: T): T {
  if (import.meta.env.VITE_BIFANG_EDITION !== "xhs") return messages;
  return JSON.parse(JSON.stringify(messages), (_key, value: unknown) => typeof value === "string"
    ? value.replaceAll("文章", "笔记").replace(/\bArticles\b/g, "Notes").replace(/\bArticle\b/g, "Note").replace(/\barticles\b/g, "notes").replace(/\barticle\b/g, "note")
    : value) as T;
}

/** 把当前 i18next 语言同步到文档根，供辅助技术识别。 */
function synchronizeDocumentLanguage(language: string): void {
  document.documentElement.lang = language.toLowerCase().startsWith("zh")
    ? "zh-CN"
    : "en-US";
}

/** 初始化唯一 i18next 实例；首次启动及宿主读取失败均默认简体中文。 */
export async function initializeI18n(): Promise<SupportedInterfaceLanguage> {
  let language: SupportedInterfaceLanguage = "zh-CN";
  try {
    language = await getSystemLocale();
  } catch {
    language = "zh-CN";
  }
  if (!i18n.isInitialized) {
    await i18n.use(initReactI18next).init({
      fallbackLng: "zh-CN",
      interpolation: { escapeValue: false },
      lng: language,
      ns: ["app", "brandSupport", "research"],
      defaultNS: "app",
      resources: {
        "en-US": { app: editionMessages(appEnUS), brandSupport: brandEnUS, research: researchEnUS },
        "zh-CN": { app: editionMessages(appZhCN), brandSupport: brandZhCN, research: researchZhCN },
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
