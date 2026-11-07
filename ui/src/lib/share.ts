// Share codes: a modpack as a short text that holds only the game version, the loader and the Modrinth
// version id of each file. Names, sizes and download links are looked up when someone opens the code, so a
// pack of 100 files still fits in a Discord message. The code travels in the part of the web address after
// "#", which browsers never send to a server: https://www.cubbylauncher.com/p#<code>

import { t } from './i18n.svelte'
import type { Loader, LoaderKind } from './types'

export const SHARE_PAGE = 'https://www.cubbylauncher.com/p'
export const SCHEME = 'cubby://pack/'
export const MAX_FILES = 400

const KINDS: readonly LoaderKind[] = ['vanilla', 'fabric', 'quilt', 'forge', 'neoforge']

export interface SharedPack {
  name: string
  minecraft: string
  loader: Loader
  /** Modrinth version ids. */
  versions: string[]
  /** The ones the sender marked optional (a subset of `versions`). */
  optional: string[]
  note: string
  /** The player who shared it: Minecraft name and UUID (32 hex digits), for their head on the pack's page. */
  creator: { name: string; uuid: string } | null
}

interface Wire {
  v: 1
  n: string
  mc: string
  l: { k: string; v?: string }
  m: string[]
  o?: string[]
  d?: string
  c?: { n: string; u: string }
}

const ID = /^[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}$/
const MC = /^[0-9A-Za-z._+-]{1,40}$/

function toBase64Url(bytes: Uint8Array): string {
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

function fromBase64Url(text: string): Uint8Array {
  const b64 = text.replace(/-/g, '+').replace(/_/g, '/') + '==='.slice((text.length + 3) % 4)
  const bin = atob(b64)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}

async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const out = new Blob([bytes as BlobPart]).stream().pipeThrough(stream as unknown as ReadableWritablePair<Uint8Array, Uint8Array>)
  return new Uint8Array(await new Response(out).arrayBuffer())
}

/** The code for a pack. */
export async function encodePack(pack: SharedPack): Promise<string> {
  if (pack.versions.length > MAX_FILES) throw new Error(`A pack can hold at most ${MAX_FILES} files.`)
  const wire: Wire = {
    v: 1,
    n: pack.name.trim().slice(0, 64),
    mc: pack.minecraft,
    l: pack.loader.version ? { k: pack.loader.kind, v: pack.loader.version } : { k: pack.loader.kind },
    m: pack.versions,
  }
  const optional = pack.optional.filter((o) => pack.versions.includes(o))
  if (optional.length) wire.o = optional
  if (pack.note.trim()) wire.d = pack.note.trim().slice(0, 140)
  if (pack.creator) wire.c = { n: pack.creator.name, u: pack.creator.uuid }
  const json = new TextEncoder().encode(JSON.stringify(wire))
  return toBase64Url(await pipe(json, new CompressionStream('deflate-raw')))
}

/** The pack in a code; throws a readable message when the code is damaged or not ours. */
export async function decodePack(code: string): Promise<SharedPack> {
  let wire: Partial<Wire>
  try {
    const bytes = await pipe(fromBase64Url(code.trim()), new DecompressionStream('deflate-raw'))
    if (bytes.length > 200_000) throw new Error('too big')
    wire = JSON.parse(new TextDecoder().decode(bytes))
  } catch {
    throw new Error(t('This code is damaged. Ask for the link again.'))
  }
  if (wire.v !== 1) throw new Error(t('This pack was made by a newer Cubby. Update Cubby to open it.'))
  const kind = wire.l?.k as LoaderKind
  const ids = (list: unknown): string[] => (Array.isArray(list) ? list.filter((x): x is string => typeof x === 'string' && ID.test(x)) : [])
  const versions = [...new Set(ids(wire.m))]
  if (typeof wire.mc !== 'string' || !MC.test(wire.mc) || !KINDS.includes(kind) || !versions.length || versions.length > MAX_FILES) {
    throw new Error(t('This code doesn’t describe a pack Cubby can install.'))
  }
  const uuid = typeof wire.c?.u === 'string' ? wire.c.u.replace(/-/g, '').toLowerCase() : ''
  const creator = typeof wire.c?.n === 'string' && /^\w{1,16}$/.test(wire.c.n) && /^[0-9a-f]{32}$/.test(uuid) ? { name: wire.c.n, uuid } : null
  const loaderVersion = typeof wire.l?.v === 'string' && /^[0-9A-Za-z._+-]{1,40}$/.test(wire.l.v) ? wire.l.v : undefined
  return {
    name: typeof wire.n === 'string' ? wire.n.trim().slice(0, 64) : '',
    minecraft: wire.mc,
    loader: loaderVersion ? { kind, version: loaderVersion } : { kind },
    versions,
    optional: ids(wire.o).filter((o) => versions.includes(o)),
    note: typeof wire.d === 'string' ? wire.d.slice(0, 140) : '',
    creator,
  }
}

export function linkFor(code: string): string {
  return `${SHARE_PAGE}#${code}`
}

/** The code inside a pasted link (web address or cubby://), or a bare code; null when it is neither. */
export function codeFromText(text: string): string | null {
  const t = text.trim()
  if (!t) return null
  const hash = t.match(/^https?:\/\/(?:www\.)?cubbylauncher\.com\/p\/?#([A-Za-z0-9_-]+)$/i)
  if (hash) return hash[1]
  if (t.toLowerCase().startsWith(SCHEME)) {
    const rest = t.slice(SCHEME.length)
    return /^[A-Za-z0-9_-]+$/.test(rest) ? rest : null
  }
  return /^[A-Za-z0-9_-]{12,}$/.test(t) ? t : null
}
