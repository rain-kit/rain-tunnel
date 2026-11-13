import { Fragment, useSyncExternalStore, type ReactNode } from "react";
import en from "../locales/en";
import zhCN, { type MessageKey, type Messages } from "../locales/zh-CN";
import zhTW from "../locales/zh-TW";
import type { I18nMsg } from "./types";

export type Locale = "zh-CN" | "zh-TW" | "en";
/** 设置中保存的语言，system 表示跟随系统。 */
export type Language = "system" | Locale;

const dicts: Record<Locale, Messages> = { "zh-CN": zhCN, "zh-TW": zhTW, en };
/** 只用作首屏缓存，以后端设置中的语言为准。 */
const STORAGE_KEY = "language";

/** 繁体地区和 Hant 用繁体中文，其余中文用简体，其他语言用英文。 */
function fromTag(tag: string): Locale {
  const t = tag.toLowerCase();
  if (t === "zh" || t.startsWith("zh-")) return /hant|-(tw|hk|mo)\b/.test(t) ? "zh-TW" : "zh-CN";
  return "en";
}

/** 只看系统首选语言，不在偏好列表里往后找；拿到后端设置后以后端识别的系统语言为准。 */
let systemLocale: Locale = fromTag(navigator.language);

export function resolveLocale(lang: Language): Locale {
  return lang === "system" ? systemLocale : lang;
}

let language: Language = (localStorage.getItem(STORAGE_KEY) as Language | null) ?? "system";
let locale: Locale = resolveLocale(language);
const listeners = new Set<() => void>();

function apply() {
  const next = resolveLocale(language);
  document.documentElement.lang = next;
  if (next === locale) return;
  locale = next;
  listeners.forEach((f) => f());
}

document.documentElement.lang = locale;

/** 切换界面语言；持久化由后端设置负责。system 为后端识别的系统语言。 */
export function setLanguage(lang: Language, system?: Locale) {
  language = lang;
  if (system) systemLocale = system;
  localStorage.setItem(STORAGE_KEY, lang);
  apply();
}

type Params = Record<string, string | number>;

function lookup(key: string, count?: unknown): string | undefined {
  const dict = dicts[locale] as Record<string, string>;
  return (count === 1 && dict[`${key}_one`]) || dict[key] || (zhCN as Record<string, string>)[key];
}

function fill(text: string, params: Params): string {
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in params ? String(params[k]) : m));
}

/** 取当前语言的文案，{name} 由 params 替换；英文在 count 为 1 时优先使用 key_one。 */
export function t(key: MessageKey, params?: Params): string {
  const text = lookup(key, params?.count) ?? key;
  return params ? fill(text, params) : text;
}

/**
 * 翻译后端返回的消息 { i18n, params }：参数可以是嵌套消息；
 * cause 是底层错误原文，文案中没有 {cause} 时追加在末尾。找不到文案时显示 key。
 */
export function tm(msg: I18nMsg): string {
  const params: Params = {};
  for (const [k, v] of Object.entries(msg.params ?? {})) params[k] = typeof v === "object" ? tm(v) : v;
  const template = lookup(msg.i18n) ?? msg.i18n;
  const text = fill(template, params);
  if (params.cause === undefined || template.includes("{cause}")) return text;
  return t("err.withCause", { message: text, cause: params.cause });
}

/** 与 t 相同，但占位符可以替换成 React 节点（例如加粗的名称）。 */
export function tx(key: MessageKey, nodes: Record<string, ReactNode>): ReactNode {
  return t(key)
    .split(/(\{\w+\})/)
    .map((part, i) => {
      const k = /^\{(\w+)\}$/.exec(part)?.[1];
      return <Fragment key={i}>{k && k in nodes ? nodes[k] : part}</Fragment>;
    });
}

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

/** 订阅语言变化；显示文案的组件都应调用，以便切换语言时重新渲染。 */
export function useI18n() {
  const current = useSyncExternalStore(subscribe, () => locale);
  return { t, tm, tx, locale: current };
}
