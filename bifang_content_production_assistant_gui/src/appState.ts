import { atom } from "jotai";

import type { SupportedInterfaceLanguage } from "./brand-support/SettingsPageTemplate";

/** 当前进程中的界面语言镜像；持久事实仍由 Rust GUI 适配器拥有。 */
export const interfaceLanguageAtom = atom<SupportedInterfaceLanguage>("en-US");
