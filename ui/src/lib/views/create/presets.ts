// Ready-made first instances, offered by the first-launch assistant and the empty library.

import { api } from '../../api'
import { t } from '../../i18n.svelte'
import { encodeIcon } from '../../icon'
import { app } from '../../state.svelte'
import type { NewInstance } from '../../types'

export type Preset = 'latest' | 'fast' | 'server'

/** The newest release and its newest stable Fabric loader; null when they can't be found. */
export interface Newest {
  release: string | null
  fabric: string | null
  /** Why the release is missing: Mojang's servers didn't answer. */
  failed: boolean
  /** Why Fabric is missing: its servers didn't answer (else it isn't out for this release yet). */
  fabricFailed: boolean
}

export async function findNewest(): Promise<Newest> {
  const out: Newest = { release: null, fabric: null, failed: false, fabricFailed: false }
  try {
    const newest = (await api.listVersions())
      .filter((v) => v.kind === 'release')
      .sort((a, b) => Date.parse(b.releaseTime) - Date.parse(a.releaseTime))[0]
    out.release = newest?.id ?? null
  } catch {
    out.failed = true
  }
  if (!out.release) return out
  try {
    out.fabric = (await api.listLoaderVersions('fabric', out.release)).find((l) => l.stable)?.version ?? null
  } catch {
    out.fabricFailed = true
  }
  return out
}

/** "play.example.net:25566" → "play.example.net" */
export const hostOf = (address: string) => address.replace(/:\d+$/, '')

/** What is wrong with a server address, or "" when it looks fine. */
export function addressProblem(address: string): string {
  if (!address) return t('Enter the server address.')
  if (/\s/.test(address)) return t('Server addresses have no spaces.')
  const port = /:(\d+)$/.exec(address)
  if (port && (Number(port[1]) < 1 || Number(port[1]) > 65535)) return t('The port goes from 1 to 65535.')
  return ''
}

/** The instance a preset makes. `address` is only read by the server preset. */
export function presetSpec(preset: Preset, release: string, fabric: string | null, address = ''): NewInstance {
  switch (preset) {
    case 'fast':
      return { name: `Fabric ${release}`, group: null, icon: encodeIcon('rocket', 'sky'), minecraft: release, loader: { kind: 'fabric', version: fabric ?? undefined } }
    case 'server':
      return { name: hostOf(address), group: null, icon: encodeIcon('castle', 'amethyst'), minecraft: release, loader: { kind: 'vanilla' } }
    default:
      return { name: `Minecraft ${release}`, group: null, icon: encodeIcon('block', 'moss'), minecraft: release, loader: { kind: 'vanilla' } }
  }
}

/** Opens Discover on performance mods for this instance. */
export function discoverPerformanceMods(id: string) {
  app.setTarget(id)
  Object.assign(app.discover, { kind: 'mod', query: '', categories: ['optimization'], environment: null, compatibleOnly: true, page: 0 })
  app.navigate({ name: 'discover' })
}
