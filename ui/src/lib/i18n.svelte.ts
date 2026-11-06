// Translations. The English text is the key: `t('Play offline')` shows the French line from the catalog
// when Cubby is in French, and the English one otherwise (or when the catalog doesn't have it yet).
// Values go in braces: t('Play {name} again', { name }). Plurals: tn(n, 'mod', 'mods') → "3 mods".
// The catalog lives in ./i18n/fr.ts. Settings › Language picks the language ("system" follows Windows).
import { FR } from './i18n/fr'

export type Lang = 'en' | 'fr'
export type Vars = Record<string, string | number>
/** One line, or [singular, plural] for words counted with tn(). */
export type Entry = string | readonly [string, string]

const CATALOGS: Record<Exclude<Lang, 'en'>, Record<string, Entry>> = { fr: FR }

/** The language Windows (or the browser) is set to, among the ones Cubby has. */
function systemLang(): Lang {
  const tags = typeof navigator === 'undefined' ? [] : (navigator.languages ?? [navigator.language])
  return tags.some((l) => l?.toLowerCase().startsWith('fr')) ? 'fr' : 'en'
}

const CACHE = 'cubby.lang'

/** The language picked last time: tables built when a module loads use it, before the settings arrive. */
function cachedLang(): Lang | null {
  try {
    const v = localStorage.getItem(CACHE)
    return v === 'en' || v === 'fr' ? v : null
  } catch {
    return null
  }
}

/** The language the page loaded with: module-level tables were built in it. */
const loaded: Lang = cachedLang() ?? systemLang()
let current = $state<Lang>(loaded)

/** The language in use. Reading it makes a template follow language changes. */
export function lang(): Lang {
  return current
}

/** From Settings: "system", "en" or "fr". Nothing happens before the settings are read (undefined), so a start in another
 *  language than the system's doesn't reload twice. */
export function setLanguage(setting: string | null | undefined) {
  if (setting === undefined) return
  const next: Lang = setting === 'en' || setting === 'fr' ? setting : systemLang()
  let cached = false
  try {
    localStorage.setItem(CACHE, next)
    cached = true
  } catch {
    /* private mode: the system language is used at the next start */
  }
  if (typeof document !== 'undefined') document.documentElement.lang = next
  current = next
  // Tables of labels were built in the language the page loaded with: reload once to rebuild them.
  // Only when the choice is cached, so the next load starts in it and doesn't reload again.
  if (next !== loaded && cached && typeof location !== 'undefined') location.reload()
}

/** Switches language from Settings (the page reloads when the language changes). */
export function switchLanguage(setting: string) {
  setLanguage(setting)
}

/** The locale for numbers and dates (Intl). */
export function intlLocale(): string {
  return current === 'fr' ? 'fr-FR' : 'en-US'
}

const missing = new Set<string>()

function lookup(text: string): Entry | undefined {
  if (current === 'en') return undefined
  const entry = CATALOGS[current][text]
  if (entry === undefined && import.meta.env.DEV && !missing.has(text)) {
    missing.add(text)
    console.debug(`[i18n] no ${current} text for: ${text}`)
  }
  return entry
}

function fill(text: string, vars?: Vars): string {
  return vars ? text.replace(/\{(\w+)\}/g, (all, k: string) => (k in vars ? String(vars[k]) : all)) : text
}

/** The text in the current language, with its {values} filled in. */
export function t(text: string, vars?: Vars): string {
  const entry = lookup(text)
  return fill(entry === undefined ? text : typeof entry === 'string' ? entry : entry[0], vars)
}

/**
 * A count and a word: tn(3, 'mod', 'mods') → "3 mods", "3 mods" in French too ("mod" → ['mod', 'mods']).
 * French uses the singular for 0 and 1. The catalog key is the English singular.
 */
export function tn(n: number, one: string, other: string, vars?: Vars): string {
  const entry = lookup(one)
  const forms: readonly [string, string] = entry === undefined ? [one, other] : typeof entry === 'string' ? [entry, entry] : entry
  const single = current === 'fr' ? Math.abs(n) < 2 : n === 1
  return `${new Intl.NumberFormat(intlLocale()).format(n)} ${fill(single ? forms[0] : forms[1], vars)}`
}
