import { isGround, isMotif, pickIcon, type Ground, type Motif } from './cubby'
import type { Instance } from './types'

/** Props for `InstanceIcon`: a motif on a ground, or an image. */
export type IconProps = { motif: Motif; ground: Ground; src?: undefined } | { src: string; motif?: undefined; ground?: undefined }

/** `instance.icon` stores "motif:ground" ("house:moss"), an image URL, or an older block key ("grass"). */
export function iconOf(instance: Pick<Instance, 'icon' | 'name'> | undefined): IconProps {
  const icon = instance?.icon ?? null
  if (icon && /^(https?:|data:|asset:|blob:|\/)/.test(icon)) return { src: icon }
  if (icon && icon.includes(':')) {
    const [motif, ground] = icon.split(':')
    if (isMotif(motif) && isGround(ground)) return { motif, ground }
  }
  // no icon, or an icon key from Cubby's first UI: always the same pair for the same name
  return pickIcon(instance?.name ?? icon ?? '')
}

export function encodeIcon(motif: Motif, ground: Ground): string {
  return `${motif}:${ground}`
}

/** A stable emblem for a Modrinth project that has no icon. */
export function projectArt(slug: string): { motif: Motif; ground: Ground } {
  return pickIcon(slug)
}
