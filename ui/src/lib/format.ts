import { intlLocale, t, tn } from './i18n.svelte'
import type { Instance, Loader, LoaderKind } from './types'

/** The locale of numbers and dates: follows the language picked in Settings. */
export function locale(): string {
  return intlLocale()
}

const LOADER_NAMES: Record<LoaderKind, string> = {
  vanilla: 'Vanilla',
  fabric: 'Fabric',
  quilt: 'Quilt',
  neoforge: 'NeoForge',
  forge: 'Forge',
}

export function loaderName(loader: Loader | LoaderKind): string {
  return LOADER_NAMES[typeof loader === 'string' ? loader : loader.kind]
}

/** "Fabric 0.16.5", "Vanilla" */
export function loaderFull(loader: Loader): string {
  return loader.kind === 'vanilla' ? 'Vanilla' : `${LOADER_NAMES[loader.kind]}${loader.version ? ` ${loader.version}` : ''}`
}

/** "1.21.1 · Fabric" */
export function instanceMeta(instance: Instance): string {
  return instance.loader.kind === 'vanilla' ? instance.minecraft : `${instance.minecraft} · ${loaderName(instance.loader)}`
}

// Intl formatters are costly to build: one per locale, built on first use.
const formatters = new Map<string, { nf: Intl.NumberFormat; compact: Intl.NumberFormat }>()
function fmt() {
  const loc = locale()
  let f = formatters.get(loc)
  if (!f) {
    f = { nf: new Intl.NumberFormat(loc), compact: new Intl.NumberFormat(loc, { notation: 'compact', maximumFractionDigits: 1 }) }
    formatters.set(loc, f)
  }
  return f
}

/** "4,096" */
export function num(n: number): string {
  return fmt().nf.format(n)
}

/** "2.3M", "18.4K" */
export function compactNum(n: number): string {
  return fmt().compact.format(n)
}

/** "Never played", "Just now", "12 min ago", "2h ago", "Yesterday", "4 days ago", "3 weeks ago", "2 months ago" */
export function relativeTime(iso: string | null, never = 'Never played'): string {
  if (!iso) return t(never)
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return t('Just now')
  if (diff < 3600) return t('{n} min ago', { n: Math.floor(diff / 60) })
  if (diff < 86400) return t('{n}h ago', { n: Math.floor(diff / 3600) })
  const days = Math.floor(diff / 86400)
  if (days === 1) return t('Yesterday')
  if (days < 7) return t('{n} days ago', { n: days })
  if (days < 30) return days >= 14 ? t('{n} weeks ago', { n: Math.floor(days / 7) }) : t('1 week ago')
  if (days < 365) return days >= 60 ? t('{n} months ago', { n: Math.floor(days / 30) }) : t('1 month ago')
  return days >= 730 ? t('{n} years ago', { n: Math.floor(days / 365) }) : t('1 year ago')
}

/** "41h", "45 min", "3h 12m" */
export function playTime(secs: number): string {
  if (secs < 60) return '0 min'
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h === 0) return `${m} min`
  if (h >= 10 || m === 0) return `${h}h`
  return `${h}h ${m}m`
}

/** "1h 12m" for a running session, from milliseconds */
export function session(ms: number): string {
  return playTime(Math.floor(ms / 1000))
}

/** "00:42:13" */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000))
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(Math.floor(s / 3600))}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`
}

/** "18:42" */
export function timeOfDay(iso: string): string {
  return new Date(iso).toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit', hour12: false })
}

/** "Nov 2, 2026" */
export function date(iso: string): string {
  return new Date(iso).toLocaleDateString(locale(), { year: 'numeric', month: 'short', day: 'numeric' })
}

/** "412 MB", "2.4 GB" (decimal units, like Windows Explorer shows sizes to most people) */
export function bytes(n: number): string {
  const fr = locale().startsWith('fr')
  if (n < 1000) return `${n} ${fr ? 'o' : 'B'}`
  const units = fr ? ['Ko', 'Mo', 'Go', 'To'] : ['KB', 'MB', 'GB', 'TB']
  let v = n / 1000
  let i = 0
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000
    i++
  }
  return `${v.toLocaleString(locale(), { maximumFractionDigits: v < 10 ? 1 : 0 })} ${units[i]}`
}

/** "6,144 MB" */
export function megabytes(mb: number): string {
  return `${num(mb)} ${locale().startsWith('fr') ? 'Mo' : 'MB'}`
}

/** plural(38, 'mod', 'mods') → "38 mods", translated when the catalog has the word. */
export function plural(n: number, one: string, other: string): string {
  return tn(n, one, other)
}

/** Case and accent insensitive comparison key. */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
}
