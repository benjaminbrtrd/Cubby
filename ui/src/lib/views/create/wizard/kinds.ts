// What a project is, from its categories, as an icon and a word: the same three types as the instance's Mods tab.
import { t } from '../../../i18n.svelte'
import type { IconName } from '../../../cubby'
import type { ProjectKind } from '../../../types'

export interface ProjectType {
  icon: IconName
  label: string
}

/** Mods only: a library other mods need, a performance mod, or content. Null when nothing says (CurseForge hits). */
export function typeOf(categories: string[], kind: ProjectKind): ProjectType | null {
  if (kind !== 'mod') return null
  const cats = categories.map((c) => c.toLowerCase())
  if (cats.some((c) => c === 'library' || c.includes('api and library'))) return { icon: 'wrench', label: t('Library') }
  if (cats.some((c) => c === 'optimization' || c.includes('performance'))) return { icon: 'memory', label: t('Performance') }
  if (!cats.length) return null
  return { icon: 'cube', label: t('Content') }
}
