// Discover helpers: words per kind, Modrinth categories, and the compatibility rules shared by the
// result cards, the project page and the install panel.

import type { IconName } from '../../cubby'
import { loaderName } from '../../format'
import type { ContentSource, Instance, LoaderKind, ProjectHit, ProjectKind, ProjectVersion, SideSupport } from '../../types'

export const PAGE_SIZE = 20

export interface KindText {
  /** Tab label */
  label: string
  /** "mod", "shader pack" */
  one: string
  /** "mods", "shader packs" */
  many: string
  /** Folder of the instance it goes to */
  folder: string
  /** Instance tab where the player manages it */
  tab: 'mods' | 'shaders' | 'resourcepacks' | null
  /** Short sentence for "New to …?" */
  help: string
}

export const KINDS: Record<ProjectKind, KindText> = {
  mod: {
    label: 'Mods',
    one: 'mod',
    many: 'mods',
    folder: 'mods',
    tab: 'mods',
    help: 'Mods add things to the game: blocks, mobs, tools, menus or new ways to play. Cubby checks each one against your instance and adds the mods it needs.',
  },
  modpack: {
    label: 'Modpacks',
    one: 'modpack',
    many: 'modpacks',
    folder: '',
    tab: null,
    help: 'A modpack is a set of mods picked and tested together. Cubby installs it as a new instance with the right Minecraft version and loader, ready to play.',
  },
  shader: {
    label: 'Shader packs',
    one: 'shader pack',
    many: 'shader packs',
    folder: 'shaderpacks',
    tab: 'shaders',
    help: 'Shader packs change how light, shadows, water and sky are drawn. They run through the Iris mod; blocks and gameplay stay the same.',
  },
  resourcepack: {
    label: 'Resource packs',
    one: 'resource pack',
    many: 'resource packs',
    folder: 'resourcepacks',
    tab: 'resourcepacks',
    help: 'Resource packs change how blocks, items, mobs and menus look, and sometimes how they sound. They add nothing new, so they are safe in any world.',
  },
}

export const KIND_ORDER: ProjectKind[] = ['mod', 'modpack', 'shader', 'resourcepack']

/** Path segment of a project on modrinth.com. */
export function modrinthUrl(kind: ProjectKind, slug: string): string {
  return `https://modrinth.com/${kind}/${slug}`
}

// categories
export interface CategoryGroup {
  label: string
  items: string[]
}

/** Modrinth category slugs offered as filters, per kind (the tags Modrinth uses for that project type). */
export const CATEGORIES: Record<ProjectKind, CategoryGroup[]> = {
  mod: [
    {
      label: 'Categories',
      items: [
        'adventure',
        'decoration',
        'economy',
        'equipment',
        'food',
        'game-mechanics',
        'library',
        'magic',
        'management',
        'minigame',
        'mobs',
        'optimization',
        'social',
        'storage',
        'technology',
        'transportation',
        'utility',
        'worldgen',
      ],
    },
  ],
  modpack: [
    {
      label: 'Categories',
      items: ['adventure', 'challenging', 'combat', 'kitchen-sink', 'lightweight', 'magic', 'multiplayer', 'optimization', 'quests', 'technology'],
    },
  ],
  shader: [
    { label: 'Style', items: ['vanilla-like', 'semi-realistic', 'realistic', 'fantasy', 'cartoon'] },
    { label: 'Features', items: ['atmosphere', 'bloom', 'colored-lighting', 'foliage', 'path-tracing', 'pbr', 'reflections', 'shadows'] },
    { label: 'Performance impact', items: ['potato', 'low', 'medium', 'high', 'screenshot'] },
  ],
  resourcepack: [
    {
      label: 'Style',
      items: ['vanilla-like', 'simplistic', 'realistic', 'themed', 'decoration', 'combat', 'tweaks', 'utility', 'modded'],
    },
    { label: 'Changes', items: ['blocks', 'items', 'entities', 'gui', 'fonts', 'audio', 'environment', 'equipment', 'models', 'core-shaders', 'locale'] },
    { label: 'Resolution', items: ['8x-', '16x', '32x', '48x', '64x', '128x', '256x', '512x+'] },
  ],
}

const CATEGORY_LABELS: Record<string, string> = {
  adventure: 'Adventure',
  challenging: 'Challenging',
  combat: 'Combat',
  cursed: 'Cursed',
  decoration: 'Decoration',
  economy: 'Economy',
  equipment: 'Equipment',
  food: 'Food',
  'game-mechanics': 'Game mechanics',
  'kitchen-sink': 'Kitchen sink',
  library: 'Library',
  lightweight: 'Lightweight',
  magic: 'Magic',
  management: 'Management',
  minigame: 'Minigames',
  mobs: 'Mobs',
  multiplayer: 'Multiplayer',
  optimization: 'Performance',
  quests: 'Quests',
  social: 'Social',
  storage: 'Storage',
  technology: 'Technology',
  transportation: 'Transportation',
  utility: 'Utility',
  worldgen: 'World generation',
  // shader packs
  'vanilla-like': 'Close to the original',
  'semi-realistic': 'Semi-realistic',
  realistic: 'Realistic',
  fantasy: 'Fantasy',
  cartoon: 'Cartoon',
  atmosphere: 'Atmosphere',
  bloom: 'Bloom',
  'colored-lighting': 'Colored lighting',
  foliage: 'Waving plants',
  'path-tracing': 'Path tracing',
  pbr: 'PBR materials',
  reflections: 'Reflections',
  shadows: 'Shadows',
  potato: 'Very light',
  low: 'Light',
  medium: 'Medium',
  high: 'Heavy',
  screenshot: 'For screenshots',
  // resource packs
  simplistic: 'Simple',
  themed: 'Themed',
  tweaks: 'Tweaks',
  modded: 'For mods',
  blocks: 'Blocks',
  items: 'Items',
  entities: 'Mobs and entities',
  gui: 'Menus',
  fonts: 'Fonts',
  audio: 'Sounds',
  environment: 'Sky and weather',
  models: 'Models',
  'core-shaders': 'Core shaders',
  locale: 'Language',
  '8x-': '8x or lower',
  '512x+': '512x or higher',
}

/** Readable name of a Modrinth category slug ("game-mechanics" → "Game mechanics"). */
export function categoryLabel(slug: string): string {
  const known = CATEGORY_LABELS[slug]
  if (known) return known
  const text = slug.replace(/-/g, ' ')
  return text.charAt(0).toUpperCase() + text.slice(1)
}

/** Texture resolution from a resource pack's categories ("32x" → 32), if it says one. */
export function resolutionOf(categories: string[]): number | null {
  for (const c of categories) {
    const m = /^(\d+)x[+-]?$/.exec(c)
    if (m) return Number(m[1])
  }
  return null
}

// loaders and versions
const LOADER_LABELS: Record<string, string> = {
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
  iris: 'Iris',
  optifine: 'OptiFine',
  canvas: 'Canvas',
  vanilla: 'Core shaders',
  minecraft: 'Minecraft',
  datapack: 'Data pack',
  liteloader: 'LiteLoader',
  modloader: 'ModLoader',
  rift: 'Rift',
  babric: 'Babric',
  'legacy-fabric': 'Legacy Fabric',
  ornithe: 'Ornithe',
  nilloader: 'NilLoader',
}

/** "neoforge" → "NeoForge" */
export function loaderLabel(slug: string): string {
  return LOADER_LABELS[slug] ?? slug.charAt(0).toUpperCase() + slug.slice(1)
}

const MOD_LOADERS = ['fabric', 'quilt', 'forge', 'neoforge', 'liteloader', 'modloader', 'rift', 'babric', 'legacy-fabric', 'ornithe', 'nilloader']

/** The mod loaders among a project's or version's loaders (drops "minecraft", "datapack", "iris"...). */
export function modLoadersOf(loaders: string[]): string[] {
  return loaders.filter((l) => MOD_LOADERS.includes(l))
}

/** Loaders whose mods run on an instance with this loader (a Quilt instance runs Fabric mods too). */
export function acceptedLoaders(kind: LoaderKind): string[] {
  if (kind === 'vanilla') return []
  if (kind === 'quilt') return ['quilt', 'fabric']
  return [kind]
}

/** "Fabric, Quilt and NeoForge" */
export function listWords(words: string[], and = 'and'): string {
  if (words.length <= 1) return words[0] ?? ''
  return `${words.slice(0, -1).join(', ')} ${and} ${words[words.length - 1]}`
}

const RELEASE = /^(\d+)\.(\d+)(?:\.(\d+))?$/

/** Orders Minecraft release ids ("1.20.1" < "1.21"); anything else (snapshots) goes last, as given. */
export function compareMc(a: string, b: string): number {
  const ma = RELEASE.exec(a)
  const mb = RELEASE.exec(b)
  if (!ma || !mb) return ma ? -1 : mb ? 1 : 0
  for (let i = 1; i <= 3; i++) {
    const d = Number(ma[i] ?? 0) - Number(mb[i] ?? 0)
    if (d) return d
  }
  return 0
}

export function isRelease(id: string): boolean {
  return RELEASE.test(id)
}

/** The newest release in a list of game versions (or the last entry when none is a release). */
export function newestMc(versions: string[]): string | null {
  const releases = versions.filter(isRelease).sort(compareMc)
  return releases[releases.length - 1] ?? versions[versions.length - 1] ?? null
}

/** "1.21.1", "1.20.1, 1.21", "1.19.4–1.21.1" */
export function mcRange(versions: string[]): string {
  const releases = [...new Set(versions.filter(isRelease))].sort(compareMc)
  if (!releases.length) return versions.slice(0, 3).join(', ')
  if (releases.length <= 3) return releases.join(', ')
  return `${releases[0]}–${releases[releases.length - 1]}`
}

/** Where a project runs, in words, or null when Modrinth doesn't say. */
export function sideText(client: SideSupport, server: SideSupport): string | null {
  const onClient = client === 'required' || client === 'optional'
  const onServer = server === 'required' || server === 'optional'
  if (client === 'unknown' && server === 'unknown') return null
  if (onClient && onServer) return 'Client and server'
  if (onClient) return 'Client only'
  if (onServer) return 'Server only'
  return null
}

/** What a player should know about servers, from client_side / server_side. */
export function serverHint(client: SideSupport, server: SideSupport): string | null {
  if (server === 'required') return 'Playing on a server? The server needs it too.'
  if (server === 'optional' && client === 'required') return "You can still join servers that don't have it. Some features may need it on the server too."
  if (server === 'unsupported' && client !== 'unsupported') return "It only runs in your game. Servers don't need it."
  if (client === 'unsupported' || client === 'optional') return 'Made for servers. It also works in single player.'
  return null
}

// compatibility
export type Tone = 'ok' | 'warn' | 'bad' | 'info' | 'busy' | 'have'

export interface Fit {
  tone: Tone
  icon: IconName
  /** Short word or phrase: "Works with Friends SMP", "Needs Minecraft 1.20.1" */
  word: string
  /** One more sentence, for the install checklist. */
  detail?: string
}

type Supported = Pick<ProjectHit, 'gameVersions' | 'loaders'>

function gameFits(gameVersions: string[], target: Instance): boolean {
  return gameVersions.includes(target.minecraft)
}

function loaderFits(loaders: string[], target: Instance): boolean {
  const accepted = acceptedLoaders(target.loader.kind)
  return loaders.some((l) => accepted.includes(l))
}

/** Whether this version (or project) can go into the instance. Modpacks always make their own instance. */
export function fits(v: Supported, kind: ProjectKind, target: Instance | undefined): boolean {
  if (kind === 'modpack' || !target) return true
  if (!gameFits(v.gameVersions, target)) return false
  if (kind === 'mod') return loaderFits(v.loaders, target)
  if (kind === 'shader') return target.loader.kind !== 'vanilla'
  return true
}

/** The compatibility line of a result card, from the project's versions and loaders. */
export function hitFit(hit: Supported & { kind: ProjectKind }, target: Instance | undefined): Fit {
  if (hit.kind === 'modpack') {
    const mc = newestMc(hit.gameVersions)
    const loaders = modLoadersOf(hit.loaders).map(loaderLabel)
    const meta = [mc, loaders[0]].filter(Boolean).join(' · ')
    return { tone: 'info', icon: 'plus', word: meta ? `New instance · ${meta}` : 'Installs as a new instance' }
  }
  if (!target) return { tone: 'info', icon: 'info', word: 'Pick an instance to check' }
  if (hit.kind === 'mod' && target.loader.kind === 'vanilla') return { tone: 'warn', icon: 'warning', word: 'Needs a mod loader' }
  if (hit.kind === 'shader' && target.loader.kind === 'vanilla') return { tone: 'warn', icon: 'warning', word: 'Needs Iris and a mod loader' }
  if (!gameFits(hit.gameVersions, target)) {
    const mc = newestMc(hit.gameVersions)
    if (hit.kind === 'resourcepack') return { tone: 'warn', icon: 'warning', word: mc ? `Made for Minecraft ${mc}` : `Not made for ${target.minecraft}` }
    return { tone: 'warn', icon: 'warning', word: mc ? `Needs Minecraft ${mc}` : `Not for ${target.minecraft}` }
  }
  if (hit.kind === 'mod' && !loaderFits(hit.loaders, target)) {
    const names = modLoadersOf(hit.loaders).map(loaderLabel)
    return { tone: 'warn', icon: 'warning', word: names.length ? `Needs ${listWords(names, 'or')}` : `Not for ${loaderName(target.loader)}` }
  }
  return { tone: 'ok', icon: 'check', word: `Works with ${target.name}` }
}

/** The version to offer first: the newest release that fits the target, else the newest one that fits. */
export function bestVersion(versions: ProjectVersion[], kind: ProjectKind, target: Instance | undefined): ProjectVersion | undefined {
  const ok = versions.filter((v) => fits(v, kind, target))
  return ok.find((v) => v.channel === 'release') ?? ok[0]
}

/** Versions that fit first (newest first), then the others. */
export function orderVersions(versions: ProjectVersion[], kind: ProjectKind, target: Instance | undefined): { fit: ProjectVersion[]; other: ProjectVersion[] } {
  const fit: ProjectVersion[] = []
  const other: ProjectVersion[] = []
  for (const v of versions) (fits(v, kind, target) ? fit : other).push(v)
  return { fit, other }
}

export const CHANNEL: Record<ProjectVersion['channel'], { word: string; icon: IconName; tone?: 'snapshot'; hint: string }> = {
  release: { word: 'Release', icon: 'check', hint: 'Finished version' },
  beta: { word: 'Beta', icon: 'warning', tone: 'snapshot', hint: 'Beta versions can have bugs' },
  alpha: { word: 'Alpha', icon: 'warning', tone: 'snapshot', hint: 'Alpha versions are early tests and can break worlds' },
}

/** Size of the file that gets downloaded for a version. */
export function primarySize(v: ProjectVersion): number {
  return (v.files.find((f) => f.primary) ?? v.files[0])?.size ?? 0
}

// shader packs
export interface Needed {
  slug: string
  title: string
  why: string
}

/** Mods a shader pack needs on this instance (Iris and Sodium, or Oculus and Embeddium on Forge). */
export function shaderMods(target: Instance | undefined): Needed[] {
  if (!target || target.loader.kind === 'vanilla') return []
  if (target.loader.kind === 'forge')
    return [
      { slug: 'oculus', title: 'Oculus', why: 'Runs shader packs on Forge.' },
      { slug: 'embeddium', title: 'Embeddium', why: 'Oculus needs it to draw the game.' },
    ]
  return [
    { slug: 'iris', title: 'Iris Shaders', why: 'Runs shader packs.' },
    { slug: 'sodium', title: 'Sodium', why: 'Iris needs it to draw the game.' },
  ]
}

/** The installed file of a project, found by slug in the instance's identified content. */
export function sourceBySlug(sources: ContentSource[] | undefined, slug: string): ContentSource | undefined {
  return sources?.find((s) => s.slug === slug)
}
