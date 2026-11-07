// Skin and cape of an account. The profile (current skin, capes owned and the one worn) and every change go
// through the backend, which talks to the Minecraft services; the default skins come from the game's own files.
import { api } from './api'
import type { CapeInfo, DefaultSkin, SkinProfile } from './types'

export type Model = 'classic' | 'slim'

export interface SkinEntry {
  id: string
  name: string
  url: string
  model: Model
  /** A skin of the game exists in both arm models: the texture follows the model picked. */
  variants?: Record<Model, string>
}

export type Cape = CapeInfo

const cache = new Map<string, Promise<HTMLCanvasElement>>()

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error('The image could not be loaded.'))
    img.src = url
  })
}

/** A skin as a 64×64 canvas. An old 64×32 skin gets its left arm and leg copied, mirrored, from the right ones. */
export function normalizeSkin(img: HTMLImageElement): HTMLCanvasElement {
  if (img.naturalWidth !== 64 || (img.naturalHeight !== 64 && img.naturalHeight !== 32)) throw new Error('A skin is a 64×64 or 64×32 PNG.')
  const c = document.createElement('canvas')
  c.width = 64
  c.height = 64
  const g = c.getContext('2d')!
  g.imageSmoothingEnabled = false
  g.drawImage(img, 0, 0)
  if (img.naturalHeight === 32) {
    const flip = (sx: number, sy: number, dx: number, dy: number, w: number, h: number) => {
      g.save()
      g.translate(dx + w, dy)
      g.scale(-1, 1)
      g.drawImage(c, sx, sy, w, h, 0, 0, w, h)
      g.restore()
    }
    // right leg (0,16) → left leg (16,48); right arm (40,16) → left arm (32,48): front and back faces
    for (const [sx, sy, dx, dy] of [[0, 16, 16, 48], [40, 16, 32, 48]] as const) {
      flip(sx + 4, sy + 4, dx + 4, dy + 4, 4, 12)
      flip(sx + 12, sy + 4, dx + 12, dy + 4, 4, 12)
    }
  }
  return c
}

export function getSkin(url: string): Promise<HTMLCanvasElement> {
  let p = cache.get(url)
  if (!p) {
    p = loadImage(url).then(normalizeSkin)
    p.catch(() => cache.delete(url))
    cache.set(url, p)
  }
  return p
}

export function drawHead(c: HTMLCanvasElement, skin: HTMLCanvasElement | null, size: number) {
  c.width = size
  c.height = size
  const g = c.getContext('2d')!
  g.clearRect(0, 0, size, size)
  g.imageSmoothingEnabled = false
  if (!skin) return
  g.drawImage(skin, 8, 8, 8, 8, 0, 0, size, size)
  g.drawImage(skin, 40, 8, 8, 8, 0, 0, size, size)
}

const capeImages = new Map<string, Promise<HTMLImageElement>>()

/** A cape texture (64×32, or a larger one with the same layout). */
export function getCapeImage(url: string): Promise<HTMLImageElement> {
  let p = capeImages.get(url)
  if (!p) {
    p = loadImage(url)
    p.catch(() => capeImages.delete(url))
    capeImages.set(url, p)
  }
  return p
}

/** The back of a cape: the part of the texture seen from behind (10×16 of 64×32). */
export function drawCapeBack(c: HTMLCanvasElement, img: HTMLImageElement | null, scale: number) {
  c.width = 10 * scale
  c.height = 16 * scale
  const g = c.getContext('2d')!
  g.clearRect(0, 0, c.width, c.height)
  g.imageSmoothingEnabled = false
  if (!img) return
  const f = img.naturalWidth / 64
  g.drawImage(img, 1 * f, 1 * f, 10 * f, 16 * f, 0, 0, c.width, c.height)
}

export interface Look {
  skinId: string
  model: Model
  capeId: string | null
}

const blobUrl = (bytes: number[]) => URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: 'image/png' }))

function message(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
}

class SkinStudio {
  accountId = ''
  username = ''
  loading = $state(false)
  /** Why the profile could not be read. */
  error = $state<string | null>(null)
  /** The skin the account wears now. */
  current = $state<SkinEntry>({ id: 'current', name: 'Current skin', url: '', model: 'classic' })
  /** Skins added in this session (a file, a name). */
  library = $state<SkinEntry[]>([])
  /** The default skins of the game. */
  defaults = $state<SkinEntry[]>([])
  defaultsError = $state<string | null>(null)
  /** The capes the account owns. */
  owned = $state<Cape[]>([])
  worn = $state<string | null>(null)
  /** What the preview shows: picked, not applied yet. */
  draft = $state<Look>({ skinId: 'current', model: 'classic', capeId: null })
  applying = $state(false)

  /** Reads the profile of the account (skin, capes). */
  async open(accountId: string, username: string) {
    if (this.accountId === accountId && !this.error) return
    this.accountId = accountId
    this.username = username
    this.library = []
    await this.load()
    void this.loadDefaults()
  }

  private show(profile: SkinProfile) {
    const url = profile.skinUrl ?? `https://minotar.net/skin/${this.username}`
    this.current = { id: 'current', name: 'Current skin', url, model: profile.variant }
    this.owned = profile.capes
    this.worn = profile.capes.find((c) => c.active)?.id ?? null
    this.draft = { skinId: 'current', model: profile.variant, capeId: this.worn }
  }

  async load() {
    this.loading = true
    this.error = null
    try {
      this.show(await api.skinProfile(this.accountId))
    } catch (e) {
      this.error = message(e)
    } finally {
      this.loading = false
    }
  }

  async loadDefaults() {
    if (this.defaults.length) return
    try {
      const list: DefaultSkin[] = await api.defaultSkins()
      this.defaults = list.map((s) => {
        const classic = blobUrl(s.wide)
        const slim = blobUrl(s.slim)
        return { id: `def-${s.id}`, name: s.name, url: classic, model: s.id === 'alex' ? 'slim' : 'classic', variants: { classic, slim } }
      })
      this.defaultsError = null
    } catch (e) {
      this.defaultsError = message(e)
    }
  }

  entry(id: string): SkinEntry {
    return id === 'current' ? this.current : (this.library.find((s) => s.id === id) ?? this.defaults.find((s) => s.id === id) ?? this.current)
  }

  get selected(): SkinEntry {
    return this.entry(this.draft.skinId)
  }

  /** The texture to show: a skin of the game follows the arm model picked. */
  get previewUrl(): string {
    const s = this.selected
    return s.variants ? s.variants[this.draft.model] : s.url
  }

  pickSkin(id: string) {
    this.draft = { ...this.draft, skinId: id, model: this.entry(id).model }
  }

  get changed(): boolean {
    return this.draft.skinId !== 'current' || this.draft.model !== this.current.model || this.draft.capeId !== this.worn
  }

  addSkin(name: string, url: string, model: Model = 'classic'): SkinEntry {
    const entry = { id: `own-${Date.now()}`, name, url, model }
    this.library = [entry, ...this.library]
    this.pickSkin(entry.id)
    return entry
  }

  removeSkin(id: string) {
    this.library = this.library.filter((s) => s.id !== id)
    if (this.draft.skinId === id) this.pickSkin('current')
  }

  /** Sends the skin and the cape picked to Mojang; throws a readable message when it fails. */
  async apply() {
    this.applying = true
    try {
      const skinChanged = this.draft.skinId !== 'current' || this.draft.model !== this.current.model
      const capeChanged = this.draft.capeId !== this.worn
      let profile: SkinProfile | null = null
      if (skinChanged) {
        const res = await fetch(this.previewUrl)
        if (!res.ok) throw new Error('The skin could not be read.')
        const png = Array.from(new Uint8Array(await res.arrayBuffer()))
        profile = await api.setSkin(this.accountId, this.draft.model, png)
      }
      if (capeChanged) profile = await api.setActiveCape(this.accountId, this.draft.capeId)
      if (profile) this.show(profile)
    } finally {
      this.applying = false
    }
  }

  revert() {
    this.draft = { skinId: 'current', model: this.current.model, capeId: this.worn }
  }
}

export const studio = new SkinStudio()
