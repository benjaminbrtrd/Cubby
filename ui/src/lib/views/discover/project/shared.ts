// Helpers shared by the parts of a project page: dependency status against the target instance, and links.
import { api } from '../../../api'
import type { IconName } from '../../../cubby'
import { app } from '../../../state.svelte'
import type { Dependency, DependencyKind, Instance } from '../../../types'
import { t } from '../../../i18n.svelte'

export const DEP_GROUPS: { kind: DependencyKind; title: string; hint: string }[] = [
  { kind: 'required', title: t('Required'), hint: t('Needed to run. Cubby installs them for you.') },
  { kind: 'optional', title: t('Optional'), hint: t('Adds extras. Install them yourself if you want them.') },
  { kind: 'incompatible', title: t('Incompatible'), hint: t('Don\'t use them together.') },
  { kind: 'embedded', title: t('Included'), hint: t('Already inside the file. Nothing to install.') },
]

export function depName(d: Dependency): string {
  return d.title ?? d.slug ?? t('Unknown project')
}

/** What a dependency means for the target instance, in words. */
export function depStatus(d: Dependency, target: Instance | undefined): { tone: string; icon: IconName; word: string } {
  const have = d.projectId && target ? app.installedIn(target.id, d.projectId) : undefined
  switch (d.kind) {
    case 'required':
      if (!target) return { tone: 'muted', icon: 'download', word: t('Installed with it') }
      return have ? { tone: 'ok', icon: 'check', word: t('Already installed') } : { tone: 'busy', icon: 'download', word: t('Will be installed') }
    case 'optional':
      if (!target) return { tone: 'muted', icon: 'info', word: t('Not installed with it') }
      return have ? { tone: 'ok', icon: 'check', word: t('Installed in {name}', { name: target.name }) } : { tone: 'muted', icon: 'info', word: t('Not installed') }
    case 'incompatible':
      if (!target) return { tone: 'muted', icon: 'warning', word: t('Check your instance') }
      return have ? { tone: 'bad', icon: 'error', word: t('In {name}: remove it first', { name: target.name }) } : { tone: 'ok', icon: 'check', word: t('Not in {name}', { name: target.name }) }
    default:
      return { tone: 'muted', icon: 'check', word: t('Inside this file') }
  }
}

export function openDep(d: Dependency) {
  const id = d.slug ?? d.projectId
  if (id) app.openProject(id, 'mod')
}

/** Opens a web page in the browser. */
export function openUrl(url: string) {
  void api.openUrl(url).catch((e) => app.error(e))
}

/** Click handler for links: opens them in the browser instead of the app window (middle click too). */
export function follow(e: MouseEvent, url: string) {
  e.preventDefault()
  if (e.type === 'auxclick' && e.button !== 1) return
  openUrl(url)
}
