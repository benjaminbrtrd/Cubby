import type { Instance, Loader, LoaderKind } from './types'

// English first (v1). Every number and date goes through Intl so other locales only need a catalog.
export const LOCALE = 'en-US'

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

const nf = new Intl.NumberFormat(LOCALE)
const compact = new Intl.NumberFormat(LOCALE, { notation: 'compact', maximumFractionDigits: 1 })

/** "4,096" */
export function num(n: number): string {
  return nf.format(n)
}

/** "2.3M", "18.4K" */
export function compactNum(n: number): string {
  return compact.format(n)
}

/** "Never played", "Just now", "12 min ago", "2h ago", "Yesterday", "4 days ago", "3 weeks ago", "2 months ago" */
export function relativeTime(iso: string | null, never = 'Never played'): string {
  if (!iso) return never
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return 'Just now'
  if (diff < 3600) return `${Math.floor(diff / 60)} min ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  const days = Math.floor(diff / 86400)
  if (days === 1) return 'Yesterday'
  if (days < 7) return `${days} days ago`
  if (days < 30) return `${Math.floor(days / 7)} week${days >= 14 ? 's' : ''} ago`
  if (days < 365) return `${Math.floor(days / 30)} month${days >= 60 ? 's' : ''} ago`
  return `${Math.floor(days / 365)} year${days >= 730 ? 's' : ''} ago`
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
  return new Date(iso).toLocaleTimeString(LOCALE, { hour: '2-digit', minute: '2-digit', hour12: false })
}

/** "Nov 2, 2026" */
export function date(iso: string): string {
  return new Date(iso).toLocaleDateString(LOCALE, { year: 'numeric', month: 'short', day: 'numeric' })
}

/** "412 MB", "2.4 GB" (decimal units, like Windows Explorer shows sizes to most people) */
export function bytes(n: number): string {
  if (n < 1000) return `${n} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let v = n / 1000
  let i = 0
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000
    i++
  }
  return `${v.toLocaleString(LOCALE, { maximumFractionDigits: v < 10 ? 1 : 0 })} ${units[i]}`
}

/** "6,144 MB" */
export function megabytes(mb: number): string {
  return `${nf.format(mb)} MB`
}

const pr = new Intl.PluralRules(LOCALE)
/** plural(38, 'mod', 'mods') → "38 mods" */
export function plural(n: number, one: string, other: string): string {
  return `${nf.format(n)} ${pr.select(n) === 'one' ? one : other}`
}

/** Case and accent insensitive comparison key. */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
}

/** Cubby's calendar version as players read it: "2026.11.0" → "2026.11", "2026.11.0-alpha.1" → "2026.11 alpha 1". */
export function appVersion(v: string): string {
  const m = /^(\d+\.\d+)\.(\d+)(?:-([a-z]+)\.?(\d+)?)?/.exec(v)
  if (!m) return v
  const base = m[2] === '0' ? m[1] : `${m[1]}.${m[2]}`
  return m[3] ? `${base} ${m[3]}${m[4] ? ` ${m[4]}` : ''}` : base
}
