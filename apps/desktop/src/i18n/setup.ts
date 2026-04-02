import { createI18n } from "vue-i18n";
import zhCN from "@/i18n/messages/zh-CN.json";
import en from "@/i18n/messages/en.json";

export type Locale = "zh-CN" | "en";

const resolveLocale = (): Locale => {
  if (window.navigator.language.toLowerCase().startsWith("zh")) {
    return "zh-CN";
  }

  return "en";
};

export const i18n = createI18n({
  legacy: false,
  locale: resolveLocale(),
  fallbackLocale: ["zh-CN", "en"],
  messages: {
    "zh-CN": zhCN,
    en
  }
});

export function setLocale(locale: Locale): void {
  i18n.global.locale.value = locale;
}
