import { t } from '../../i18n.svelte'
// Small helpers shared by the modpack source screens.

/** Compares Minecraft versions like "1.20.1" and "1.7.10" (numbers by value). */
function cmp(a: string, b: string): number {
  const x = a.split('.').map((n) => parseInt(n, 10) || 0)
  const y = b.split('.').map((n) => parseInt(n, 10) || 0)
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0)
    if (d) return d
  }
  return 0
}

/** "1.20.1", "1.19.2 – 1.21.1" */
export function mcSpan(versions: string[]): string {
  if (!versions.length) return ''
  const sorted = [...versions].sort(cmp)
  const lo = sorted[0]
  const hi = sorted[sorted.length - 1]
  return lo === hi ? lo : `${lo} – ${hi}`
}

/** Paragraphs of a plain text description (blank-line separated). */
export function paragraphs(text: string): string[] {
  return text
    .split(/\n\s*\n/)
    .map((p) => p.replace(/\s*\n\s*/g, ' ').trim())
    .filter(Boolean)
}

export const CHANNEL_WORD = { release: t('Release'), beta: t('Beta'), alpha: t('Alpha') } as const
