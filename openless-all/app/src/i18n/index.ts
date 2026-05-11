// i18n 入口 — 必须在任意 UI 组件 import 之前完成 init。
//
// 设计说明：
// - 资源在打包时静态注入（zh-CN.ts / en.ts）。无需后端推送，无网络请求。
// - LocalStorage key `ol.locale` 持久化用户选择；首次启动按 navigator.language 推断。
// - fallback 永远是 zh-CN：已知的产品权威文案，且 zh-CN.ts 是 source of truth。
// - 不用 LanguageDetector 插件：它的异步 init 在 Tauri WebView 里会让首次渲染拿到的
//   `t()` 返回 key（react-i18next useSuspense 默认 false 时返回 key 而非阻塞）。
//   手写检测 + initImmediate: false 让 init 同步完成，渲染前 t 就能用。

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { en } from './en';
import { zhCN } from './zh-CN';

export const SUPPORTED_LOCALES = ['zh-CN', 'en'] as const;
export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];

export const LOCALE_STORAGE_KEY = 'ol.locale';
const FOLLOW_SYSTEM_VALUE = 'system';

function detectSystemLocale(): SupportedLocale {
  if (typeof navigator === 'undefined') return 'zh-CN';
  const nav = (navigator.language || '').toLowerCase();
  if (nav.startsWith('zh')) return 'zh-CN';
  return 'en';
}

function getStoredLocale(): SupportedLocale | null {
  if (typeof window === 'undefined') return null;
  const raw = window.localStorage.getItem(LOCALE_STORAGE_KEY);
  return raw === 'zh-CN' || raw === 'en' ? raw : null;
}

const initialLng: SupportedLocale = getStoredLocale() ?? detectSystemLocale();

void i18n.use(initReactI18next).init({
  resources: {
    'zh-CN': { translation: zhCN },
    en: { translation: en },
  },
  lng: initialLng,
  fallbackLng: 'zh-CN',
  supportedLngs: SUPPORTED_LOCALES as unknown as string[],
  partialBundledLanguages: true, // 告诉 i18next 我们的内联资源已完整，无需 backend 拉取
  interpolation: { escapeValue: false },
  react: { useSuspense: false }, // 不悬挂；首次渲染必须能拿到译文（无 backend 时 init 同步完成）
});

export default i18n;

/**
 * 当前持久化偏好。'system' 表示跟随系统；具体语言 tag 表示用户已显式选择。
 * 与 i18n.language 不同：i18n.language 永远是已 resolve 的具体语言。
 */
export function getLocalePreference(): SupportedLocale | typeof FOLLOW_SYSTEM_VALUE {
  return getStoredLocale() ?? FOLLOW_SYSTEM_VALUE;
}

/**
 * 写入用户偏好并立即切换 i18n 语言。
 * pref === 'system' 时清除存储项，重新走 navigator 检测。
 */
export async function setLocalePreference(pref: SupportedLocale | typeof FOLLOW_SYSTEM_VALUE): Promise<void> {
  if (pref === FOLLOW_SYSTEM_VALUE) {
    window.localStorage.removeItem(LOCALE_STORAGE_KEY);
    await i18n.changeLanguage(detectSystemLocale());
    return;
  }
  window.localStorage.setItem(LOCALE_STORAGE_KEY, pref);
  await i18n.changeLanguage(pref);
}

export const FOLLOW_SYSTEM = FOLLOW_SYSTEM_VALUE;
