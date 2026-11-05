// Discover helpers: words per kind, Modrinth categories, and the compatibility rules shared by the
// result cards, the project page and the install panel.

import type { IconName } from '../../cubby'
import { loaderName } from '../../format'
import type { ContentSource, Instance, LoaderKind, ProjectHit, ProjectKind, ProjectVersion, SideSupport } from '../../types'
import { t } from '../../i18n.svelte'

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
    label: t('Mods'),
    one: t('mod'),
    many: t('mods'),
    folder: 'mods',
    tab: 'mods',
    help: t('Mods add things to the game: blocks, mobs, tools, menus or new ways to play. Cubby checks each one against your instance and adds the mods it needs.'),
  },
  modpack: {
    label: t('Modpacks'),
    one: t('modpack'),
    many: t('modpacks'),
    folder: '',
    tab: null,
    help: t('A modpack is a set of mods picked and tested together. Cubby installs it as a new instance with the right Minecraft version and loader, ready to play.'),
  },
  shader: {
    label: t('Shader packs'),
    one: t('shader pack'),
    many: t('shader packs'),
    folder: 'shaderpacks',
    tab: 'shaders',
    help: t('Shader packs change how light, shadows, water and sky are drawn. They run through the Iris mod; blocks and gameplay stay the same.'),
  },
  resourcepack: {
    label: t('Resource packs'),
    one: t('resource pack'),
    many: t('resource packs'),
    folder: 'resourcepacks',
    tab: 'resourcepacks',
    help: t('Resource packs change how blocks, items, mobs and menus look, and sometimes how they sound. They add nothing new, so they are safe in any world.'),
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
      label: t('Categories'),
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
      label: t('Categories'),
      items: ['adventure', 'challenging', 'combat', 'kitchen-sink', 'lightweight', 'magic', 'multiplayer', 'optimization', 'quests', 'technology'],
    },
  ],
  shader: [
    { label: t('Style'), items: ['vanilla-like', 'semi-realistic', 'realistic', 'fantasy', 'cartoon'] },
    { label: t('Features'), items: ['atmosphere', 'bloom', 'colored-lighting', 'foliage', 'path-tracing', 'pbr', 'reflections', 'shadows'] },
    { label: t('Performance impact'), items: ['potato', 'low', 'medium', 'high', 'screenshot'] },
  ],
  resourcepack: [
    {
      label: t('Style'),
      items: ['vanilla-like', 'simplistic', 'realistic', 'themed', 'decoration', 'combat', 'tweaks', 'utility', 'modded'],
    },
    { label: t('Changes'), items: ['blocks', 'items', 'entities', 'gui', 'fonts', 'audio', 'environment', 'equipment', 'models', 'core-shaders', 'locale'] },
    { label: t('Resolution'), items: ['8x-', '16x', '32x', '48x', '64x', '128x', '256x', '512x+'] },
  ],
}

const CATEGORY_LABELS: Record<string, string> = {
  adventure: t('Adventure'),
  challenging: t('Challenging'),
  combat: t('Combat'),
  cursed: t('Cursed'),
  decoration: t('Decoration'),
  economy: t('Economy'),
  equipment: t('Equipment'),
  food: t('Food'),
  'game-mechanics': t('Game mechanics'),
  'kitchen-sink': t('Kitchen sink'),
  library: t('Library'),
  lightweight: t('Lightweight'),
  magic: t('Magic'),
  management: t('Management'),
  minigame: t('Minigames'),
  mobs: t('Mobs'),
  multiplayer: t('Multiplayer'),
  optimization: t('Performance'),
  quests: t('Quests'),
  social: t('Social'),
  storage: t('Storage'),
  technology: t('Technology'),
  transportation: t('Transportation'),
  utility: t('Utility'),
  worldgen: t('World generation'),
  // shader packs
  'vanilla-like': t('Close to the original'),
  'semi-realistic': 'Semi-realistic',
  realistic: t('Realistic'),
  fantasy: t('Fantasy'),
  cartoon: t('Cartoon'),
  atmosphere: t('Atmosphere'),
  bloom: t('Bloom'),
  'colored-lighting': t('Colored lighting'),
  foliage: t('Waving plants'),
  'path-tracing': t('Path tracing'),
  pbr: t('PBR materials'),
  reflections: t('Reflections'),
  shadows: t('Shadows'),
  potato: t('Very light'),
  low: t('Light'),
  medium: t('Medium'),
  high: t('Heavy'),
  screenshot: t('For screenshots'),
  // resource packs
  simplistic: t('Simple'),
  themed: t('Themed'),
  tweaks: t('Tweaks'),
  modded: t('For mods'),
  blocks: t('Blocks'),
  items: t('Items'),
  entities: t('Mobs and entities'),
  gui: t('Menus'),
  fonts: t('Fonts'),
  audio: t('Sounds'),
  environment: t('Sky and weather'),
  models: t('Models'),
  'core-shaders': t('Core shaders'),
  locale: t('Language'),
  '8x-': t('8x or lower'),
  '512x+': t('512x or higher'),
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
  vanilla: t('Core shaders'),
  minecraft: 'Minecraft',
  datapack: t('Data pack'),
  liteloader: 'LiteLoader',
  modloader: 'ModLoader',
  rift: 'Rift',
  babric: 'Babric',
  'legacy-fabric': t('Legacy Fabric'),
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
  return `${words.slice(0, -1).join(', ')} ${t(and)} ${words[words.length - 1]}`
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
  if (onClient && onServer) return t('Client and server')
  if (onClient) return t('Client only')
  if (onServer) return t('Server only')
  return null
}

/** What a player should know about servers, from client_side / server_side. */
export function serverHint(client: SideSupport, server: SideSupport): string | null {
  if (server === 'required') return t('Playing on a server? The server needs it too.')
  if (server === 'optional' && client === 'required') return t('You can still join servers that don\'t have it. Some features may need it on the server too.')
  if (server === 'unsupported' && client !== 'unsupported') return t('It only runs in your game. Servers don\'t need it.')
  if (client === 'unsupported' || client === 'optional') return t('Made for servers. It also works in single player.')
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
    return { tone: 'info', icon: 'plus', word: meta ? t('New instance · {meta}', { meta }) : t('Installs as a new instance') }
  }
  if (!target) return { tone: 'info', icon: 'info', word: t('Pick an instance to check') }
  if (hit.kind === 'mod' && target.loader.kind === 'vanilla') return { tone: 'warn', icon: 'warning', word: t('Needs a mod loader') }
  if (hit.kind === 'shader' && target.loader.kind === 'vanilla') return { tone: 'warn', icon: 'warning', word: t('Needs Iris and a mod loader') }
  if (!gameFits(hit.gameVersions, target)) {
    const mc = newestMc(hit.gameVersions)
    if (hit.kind === 'resourcepack') return { tone: 'warn', icon: 'warning', word: mc ? t('Made for Minecraft {mc}', { mc }) : t('Not made for {minecraft}', { minecraft: target.minecraft }) }
    return { tone: 'warn', icon: 'warning', word: mc ? t('Needs Minecraft {mc}', { mc }) : t('Not for {minecraft}', { minecraft: target.minecraft }) }
  }
  if (hit.kind === 'mod' && !loaderFits(hit.loaders, target)) {
    const names = modLoadersOf(hit.loaders).map(loaderLabel)
    return { tone: 'warn', icon: 'warning', word: names.length ? t('Needs {mods}', { mods: listWords(names, 'or') }) : t('Not for {loader}', { loader: loaderName(target.loader) }) }
  }
  return { tone: 'ok', icon: 'check', word: t('Works with {name}', { name: target.name }) }
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
  release: { word: t('Release'), icon: 'check', hint: t('Finished version') },
  beta: { word: t('Beta'), icon: 'warning', tone: 'snapshot', hint: t('Beta versions can have bugs') },
  alpha: { word: t('Alpha'), icon: 'warning', tone: 'snapshot', hint: t('Alpha versions are early tests and can break worlds') },
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
      { slug: 'oculus', title: 'Oculus', why: t('Runs shader packs on Forge.') },
      { slug: 'embeddium', title: 'Embeddium', why: t('Oculus needs it to draw the game.') },
    ]
  return [
    { slug: 'iris', title: t('Iris Shaders'), why: t('Runs shader packs.') },
    { slug: 'sodium', title: 'Sodium', why: t('Iris needs it to draw the game.') },
  ]
}

/** The installed file of a project, found by slug in the instance's identified content. */
export function sourceBySlug(sources: ContentSource[] | undefined, slug: string): ContentSource | undefined {
  return sources?.find((s) => s.slug === slug)
}
