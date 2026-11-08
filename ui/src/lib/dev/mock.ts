// In-memory backend used when the UI runs outside Tauri (browser preview, `npm run dev`).
// The data matches the Cubby mockup; the Discover catalog is fictional except for a few well-known mods
// that appear as installed. Add `?onboarding` to the URL to see the first-launch assistant.

import { pixelArt, svgUrl } from './pixelart'
import { OPTIONAL_MODS, PACKS, PACK_DESCRIPTION, PACK_MARKDOWN, PROVIDERS, SORTERS, morePacks, type MockPackDef } from './mockPacks'
import type {
  Account,
  AppUpdate,
  BlockedFile,
  ContentSource,
  ContentUpdate,
  GameVersion,
  Instance,
  InstallItem,
  LogLine,
  ModFile,
  PackDetails,
  PackFileInfo,
  PackProvider,
  PackSort,
  PackSummary,
  PackVersion,
  Project,
  ProjectHit,
  ProjectKind,
  ProjectQuery,
  ProjectVersion,
  ProviderId,
  Settings,
  CubbyEvent,
  WorldBackup,
} from '../types'

const listeners = new Set<(e: CubbyEvent) => void>()
const emit = (e: CubbyEvent) => listeners.forEach((l) => l(e))
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

const hoursAgo = (h: number) => new Date(Date.now() - h * 3_600_000).toISOString()
const daysAgo = (d: number) => hoursAgo(d * 24)
const overrides = { executable: null, minMemoryMb: null, maxMemoryMb: null, jvmArgs: null }

// pixel images (data: URLs, never shipped in the build)
const SKINS = {
  nyx: pixelArt(['HHHHHHHH', 'HHHHHHHH', 'HSSSSSSH', 'SSSSSSSS', 'SWESSEWS', 'SSSNNSSS', 'SSMMMMSS', 'SSSSSSSS'], { H: '#3b2a20', S: '#d9a07a', W: '#ffffff', E: '#3a4a7a', N: '#b9805c', M: '#7a4a36' }, 8),
  lou: pixelArt(['YYYYYYYY', 'YYYYYYYY', 'YSSSSSSY', 'SSSSSSSS', 'SWGSSGWS', 'SSSSSSSS', 'SSSMMSSS', 'SSSSSSSS'], { Y: '#e0a33a', S: '#f0c8a0', W: '#ffffff', G: '#3e6b1d', M: '#b9805c' }, 8),
}
const SERVER_ICONS = {
  crew: pixelArt(['BBBBBBBB', 'BBYBBBBB', 'BBBBBBBB', 'BBBTTBBB', 'BBTTTTBB', 'GGTWWTGG', 'DGGGGGGD', 'DDDDDDDD'], { B: '#4cc9d6', Y: '#f6e7a6', T: '#9a9a9a', W: '#5a3a22', G: '#7fc956', D: '#8b5a2b' }, 64),
  factory: pixelArt(['NNNNNNNN', 'NSNNNNSN', 'NNNGGNNN', 'NNGGGGNN', 'NGGOGGGN', 'GGGGGGGG', 'RRGGGGRR', 'RRRRRRRR'], { N: '#2c3150', S: '#f2fbff', G: '#b9bec4', O: '#e0a33a', R: '#80878f' }, 64),
}

function rng(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0
    return s / 4294967296
  }
}

const SCENE_DEFS = {
  plains: ['#7cc4ff', '#9fb7c9', '#5e9a2e', '#7fc956', '#8b5a2b', '#ffd166', '#3e6b1d', 3],
  sunset: ['#f39c3d', '#c56689', '#7b52c4', '#4f8f2a', '#6e4522', '#ffd166', '#2c3150', 7],
  snow: ['#a9dcf5', '#d9e4ec', '#ffffff', '#eef3f7', '#9fb0bf', '#f2fbff', '#3e6b1d', 11],
  desert: ['#ffd9a0', '#e8c07a', '#d4922a', '#eadca4', '#c9ad74', '#ffffff', '#5e9a2e', 5],
  night: ['#1b1e33', '#2c3150', '#3c444d', '#2b4d1a', '#3a2a1a', '#f2ede4', '#15120f', 13],
  ocean: ['#7cc4ff', '#4cc9d6', '#2b9aa6', '#eadca4', '#c9ad74', '#ffd166', '#4f8f2a', 17],
  nether: ['#4a1410', '#7a2a20', '#b02a1c', '#5a1a12', '#2a0d08', '#f39c3d', '#e0a33a', 19],
  factory: ['#9fb7c9', '#80878f', '#5d6874', '#7d7468', '#4f483f', '#ffd166', '#e0a33a', 23],
} as const
type SceneName = keyof typeof SCENE_DEFS

function scene(name: SceneName): string {
  const [sky, far, hill, ground, dirt, sun, trees, seed] = SCENE_DEFS[name]
  const W = 64
  const H = 36
  const r = rng(seed)
  let out = ''
  const rect = (x: number, y: number, w: number, h: number, c: string) => (out += `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${c}"/>`)
  rect(0, 0, W, H, sky)
  rect(48, 5, 5, 5, sun)
  let fy = 17
  for (let x = 0; x < W; x += 2) {
    fy = Math.max(12, Math.min(20, fy + Math.round((r() - 0.5) * 3)))
    rect(x, fy, 2, H - fy, far)
  }
  let hy = 22
  const g: number[] = []
  for (let x = 0; x < W; x++) {
    if (x % 3 === 0) hy = Math.max(19, Math.min(26, hy + Math.round((r() - 0.5) * 3)))
    g.push(hy)
    rect(x, hy, 1, H - hy, hill)
  }
  for (let x = 0; x < W; x++) {
    const gy = g[x] + 3
    rect(x, gy, 1, 1, ground)
    rect(x, gy + 1, 1, H - gy - 1, dirt)
  }
  for (let i = 0; i < 6; i++) {
    const x = 2 + Math.floor(r() * 58)
    const y = g[x] - 4
    rect(x + 1, y + 3, 1, 3, '#6e4522')
    rect(x, y, 3, 3, trees)
  }
  return svgUrl(`0 0 ${W} ${H}`, out, 640, 360)
}
const SCENES = Object.fromEntries(Object.keys(SCENE_DEFS).map((k) => [k, scene(k as SceneName)])) as Record<SceneName, string>
const SCENE_LIST = Object.values(SCENES)

// launcher data
const inst = (p: Partial<Instance> & Pick<Instance, 'id' | 'name' | 'icon' | 'minecraft' | 'loader'>): Instance => ({
  group: null,
  created: daysAgo(90),
  lastPlayed: null,
  totalPlayedSecs: 0,
  installed: true,
  javaOverrides: overrides,
  notes: null,
  modpack: null,
  blockedFiles: 0,
  recommendedMemoryMb: 3072,
  ...p,
})

let instances: Instance[] = [
  inst({ id: 'friends-smp', name: 'Friends SMP', group: 'Survival', icon: 'house:moss', minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' }, created: '2026-03-12T10:00:00Z', lastPlayed: hoursAgo(2), totalPlayedSecs: 41 * 3600, notes: 'Base at -240, 64, 180. Nether hub near spawn.' }),
  inst({ id: 'create-aab', name: 'Create: Above and Beyond', group: 'Modpacks', icon: 'gear:amber', minecraft: '1.20.1', loader: { kind: 'fabric', version: '0.15.11' }, installed: false, blockedFiles: 3, modpack: { projectId: 'p-sky-foundry', versionId: 'v-sky-foundry-2', title: 'Create: Above and Beyond', versionNumber: '1.4.2' } }),
  inst({ id: 'chill-skyblock', name: 'Chill Skyblock', group: 'Survival', icon: 'island:lagoon', minecraft: '1.21.1', loader: { kind: 'quilt', version: '0.27.1' }, lastPlayed: daysAgo(1), totalPlayedSecs: 12 * 3600 }),
  inst({ id: 'snapshot-24w45a', name: 'Snapshot 24w45a', group: 'Testing', icon: 'potion:rose', minecraft: '24w45a', loader: { kind: 'vanilla' }, installed: false }),
  inst({ id: 'better-minecraft', name: 'Better Minecraft', group: 'Modpacks', icon: 'pickaxe:slate', minecraft: '1.20.1', loader: { kind: 'fabric', version: '0.15.11' }, lastPlayed: daysAgo(4), totalPlayedSecs: 63 * 3600 }),
  inst({ id: 'hardcore-pvp', name: 'Hardcore 1.8 PvP', group: 'Testing', icon: 'sword:brick', minecraft: '1.8.9', loader: { kind: 'vanilla' }, lastPlayed: daysAgo(21), totalPlayedSecs: 6 * 3600 }),
  inst({ id: 'medieval-town', name: 'Medieval Town', group: 'Survival', icon: 'castle:night', minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' }, lastPlayed: daysAgo(31), totalPlayedSecs: 22 * 3600 }),
  inst({ id: 'antarctic-ski', name: 'Antarctic Ski Trip', group: 'Testing', icon: 'snowflake:sky', minecraft: '1.21.1', loader: { kind: 'vanilla' }, lastPlayed: daysAgo(62), totalPlayedSecs: 3 * 3600 }),
]

let accounts: Account[] = [
  { id: 'a1', username: 'Nyx_Ember', kind: 'microsoft', isDefault: true, skinUrl: SKINS.nyx },
  { id: 'a2', username: 'Lou_Builds', kind: 'microsoft', isDefault: false, skinUrl: SKINS.lou },
]

/** Browser dev: add `?update` to the address to see an update offered. */
const mockUpdate: AppUpdate | null =
  typeof location !== 'undefined' && location.search.includes('update')
    ? {
        version: '2026.11.0-alpha.2',
        currentVersion: '0.1.0',
        notes: [
          '## New',
          '',
          '- Modpack updates show what changed before you accept them.',
          '- The library remembers your sort order.',
          '',
          '## Fixed',
          '',
          '- A crash when an instance has no Java yet.',
          '- Downloads that stalled behind a proxy.',
        ].join('\n'),
        date: '2026-11-06T10:00:00Z',
        downloaded: false,
      }
    : null

let settings: Settings = {
  language: 'en-US',
  theme: 'night',
  hyperlegible: false,
  reduceMotion: false,
  checkUpdates: true,
  accent: '#8bd450',
  frameMode: 'none',
  frameColor: '#1e1b18',
  java: { auto: true, autoMemory: true, minMemoryMb: 1024, maxMemoryMb: 4096, jvmArgs: '' },
  game: { width: 1280, height: 720, fullscreen: false },
  onLaunch: 'minimize',
  downloadConcurrency: 4,
  msaClientId: null,
  trackPlayTime: true,
  showPlayTime: true,
  proxy: { mode: 'system', url: '' },
  updates: { mode: 'ask', channel: 'preview', skipped: null },
  curseforgeApiKey: null,
  onboarded: typeof location === 'undefined' || !location.search.includes('onboarding'),
}

const shots = (id: string, names: SceneName[], startHours: number) =>
  names.map((n, i) => {
    const t = new Date(Date.now() - (startHours + i * 9) * 3_600_000)
    const pad = (v: number) => String(v).padStart(2, '0')
    const fileName = `${t.getFullYear()}-${pad(t.getMonth() + 1)}-${pad(t.getDate())}_${pad(t.getHours())}.${pad(t.getMinutes())}.${pad(t.getSeconds())}.png`
    return { fileName, url: SCENES[n], taken: t.toISOString(), id }
  })
const screenshots = new Map<string, { fileName: string; url: string; taken: string }[]>([
  ['friends-smp', shots('friends-smp', ['plains', 'sunset', 'night', 'ocean', 'snow', 'desert', 'plains', 'sunset', 'ocean', 'night', 'snow', 'plains'], 1)],
  ['medieval-town', shots('medieval-town', ['night', 'plains', 'sunset', 'desert', 'snow'], 30)],
  ['chill-skyblock', shots('chill-skyblock', ['ocean', 'sunset', 'plains'], 20)],
  ['better-minecraft', shots('better-minecraft', ['nether', 'factory'], 100)],
])

type MockPack = { fileName: string; name: string; isDir: boolean; size: number; modified: string | null; enabled: boolean }
const packs: Record<string, Record<'resource' | 'shader', MockPack[]>> = {}
function packsOf(id: string) {
  return (packs[id] ??= {
    resource: [
      { fileName: 'Crisp-32x-2.1.zip', name: 'Crisp 32x', isDir: false, size: 48e6, modified: daysAgo(3), enabled: true },
      { fileName: 'Bold-UI-1.4.zip', name: 'Bold UI', isDir: false, size: 2e6, modified: daysAgo(9), enabled: true },
      { fileName: 'Soft-Edges-16x.zip', name: 'Soft Edges 16x', isDir: false, size: 6e6, modified: daysAgo(20), enabled: false },
      { fileName: 'Autumn Leaves', name: 'Autumn Leaves', isDir: true, size: 5e6, modified: daysAgo(40), enabled: false },
    ],
    shader: [
      { fileName: 'Glowmoss-Shaders-1.3.zip', name: 'Glowmoss Shaders', isDir: false, size: 4.1e6, modified: daysAgo(5), enabled: true },
      { fileName: 'Clearwater-2.0.zip', name: 'Clearwater', isDir: false, size: 2.6e6, modified: daysAgo(30), enabled: false },
    ],
  })
}
const resourceOrder: Record<string, string[]> = {}

const servers = new Map<string, { name: string; address: string; iconUrl: string | null; acceptTextures: boolean | null }[]>([
  ['friends-smp', [
    { name: 'The Crew', address: 'crew.example:25565', iconUrl: SERVER_ICONS.crew, acceptTextures: true },
    { name: 'Old test server', address: '192.168.1.42', iconUrl: null, acceptTextures: null },
  ]],
  ['create-aab', [{ name: 'Create Factory', address: 'play.createfactory.example', iconUrl: SERVER_ICONS.factory, acceptTextures: null }]],
  ['medieval-town', [{ name: 'Kingsbridge', address: 'kingsbridge.example.net', iconUrl: null, acceptTextures: null }]],
])

const running = new Map<string, { startedAt: string; timer: number }>()
const logs = new Map<string, LogLine[]>()

const versions: GameVersion[] = [
  ['24w45a', 'snapshot'], ['1.21.3', 'release'], ['1.21.2', 'release'], ['1.21.1', 'release'], ['1.21', 'release'], ['1.20.6', 'release'],
  ['1.20.4', 'release'], ['1.20.2', 'release'], ['1.20.1', 'release'], ['1.19.4', 'release'], ['1.19.2', 'release'], ['1.18.2', 'release'],
  ['1.17.1', 'release'], ['1.16.5', 'release'], ['1.12.2', 'release'], ['1.8.9', 'release'], ['b1.7.3', 'old_beta'], ['a1.2.6', 'old_alpha'],
].map(([id, kind], i) => ({ id, kind: kind as GameVersion['kind'], releaseTime: daysAgo(i * 60 + 10) }))

const mod = (fileName: string, name: string, version: string, authors: string, description: string, enabled = true, size = 900_000): ModFile => ({
  fileName: enabled ? fileName : `${fileName}.disabled`, enabled, size, name, version, description, authors: authors.split(', '),
})

const FRIENDS_MODS: ModFile[] = [
  mod('fabric-api-0.102.0+1.21.1.jar', 'Fabric API', '0.102.0', 'FabricMC', 'Core library that most Fabric mods need.', true, 2_100_000),
  mod('sodium-fabric-0.6.0+mc1.21.1.jar', 'Sodium', '0.6.0', 'CaffeineMC', 'A modern rendering engine: more frames, less stutter.', true, 1_200_000),
  mod('lithium-fabric-0.13.0+mc1.21.1.jar', 'Lithium', '0.13.0', 'CaffeineMC', 'Speeds up game logic without changing how the game plays.'),
  mod('iris-fabric-1.7.3+mc1.21.1.jar', 'Iris Shaders', '1.7.3', 'coderbot, IMS', 'Runs shader packs on top of Sodium.', true, 2_400_000),
  mod('modmenu-11.0.2.jar', 'Mod Menu', '11.0.2', 'Prospector', 'Lists your mods in game and opens their settings.'),
  mod('cloth-config-15.0.140-fabric.jar', 'Cloth Config', '15.0.140', 'shedaniel', 'Settings screens for other mods.'),
  mod('Xaeros_Minimap_24.4.0_Fabric_1.21.jar', "Xaero's Minimap", '24.4.0', 'xaero96', 'A minimap with waypoints in the corner of the screen.'),
  mod('Jade-1.21.1-Fabric-15.1.3.jar', 'Jade', '15.1.3', 'Snownee', 'Shows what you are looking at: block, mob, owner.'),
  mod('appleskin-fabric-mc1.21-3.0.5.jar', 'AppleSkin', '3.0.5', 'squeek502', 'Shows how much food restores in the hunger bar.'),
  mod('entityculling-fabric-1.7.1-mc1.21.jar', 'Entity Culling', '1.7.1', 'tr7zw', 'Skips drawing mobs and chests you cannot see.'),
  mod('ferritecore-7.0.0-fabric.jar', 'FerriteCore', '7.0.0', 'malte0811', 'Uses less memory.'),
  mod('continuity-3.0.0+1.21.jar', 'Continuity', '3.0.0', 'PepperCode1', 'Connected textures for glass and bookshelves.'),
  mod('zoomify-2.14.0+1.21.jar', 'Zoomify', '2.14.0', 'isXander', 'Zoom in with a key, like a spyglass.'),
  mod('yet_another_config_lib_v3-3.5.0+1.21-fabric.jar', 'YetAnotherConfigLib', '3.5.0', 'isXander', 'Settings screens for other mods.'),
  mod('create-fabric-0.5.1j+mc1.21.1.jar', 'Create', '0.5.1j', 'simibubi', 'Gears, belts and machines to automate your world.', true, 14_000_000),
  mod('OptiFine_1.21.1_HD_U_J1.jar', 'OptiFine', 'HD U J1', 'sp614x', 'Replaced by Sodium and Iris.', false, 7_000_000),
]
const modsByInstance = new Map<string, ModFile[]>([['friends-smp', FRIENDS_MODS]])
const modsOf = (id: string) => {
  if (!modsByInstance.has(id)) {
    const i = instances.find((x) => x.id === id)
    modsByInstance.set(id, i && i.loader.kind !== 'vanilla' ? structuredClone(FRIENDS_MODS.slice(0, 6)) : [])
  }
  return modsByInstance.get(id)!
}

const worlds = [
  { folder: 'Lake Village', name: 'Lake Village', lastPlayed: hoursAgo(2), sizeBytes: 412e6, iconUrl: SCENES.plains },
  { folder: 'Creative test', name: 'Creative test', lastPlayed: daysAgo(6), sizeBytes: 60e6, iconUrl: SCENES.desert },
  { folder: 'Old base', name: 'Old base', lastPlayed: daysAgo(120), sizeBytes: 238e6, iconUrl: SCENES.night },
  { folder: 'Parkour Run', name: 'Parkour Run', lastPlayed: daysAgo(14), sizeBytes: 86e6, iconUrl: SCENES.ocean },
]
let backups: WorldBackup[] = [
  { fileName: 'Lake Village-20261101-180200.zip', world: 'Lake Village', created: daysAgo(1), sizeBytes: 380e6 },
  { fileName: 'Lake Village-20261026-210500.zip', world: 'Lake Village', created: daysAgo(7), sizeBytes: 362e6 },
  { fileName: 'Lake Village-20261012-090000.zip', world: 'Lake Village', created: daysAgo(21), sizeBytes: 340e6 },
]

// Discover catalog
type CatalogEntry = Omit<Project, 'versionCount'> & { versions: ProjectVersion[] }

const ver = (projectId: string, n: number, versionNumber: string, gameVersions: string[], loaders: string[], fileName: string, size: number, deps: { slug: string; kind: 'required' | 'optional' | 'incompatible' }[] = [], channel: 'release' | 'beta' = 'release', changelog = ''): ProjectVersion => ({
  id: `v-${projectId.slice(2)}-${n}`,
  projectId,
  name: versionNumber,
  versionNumber,
  channel,
  gameVersions,
  loaders,
  published: daysAgo(12 + (3 - n) * 40),
  downloads: Math.round(120_000 / (4 - n)),
  changelog: changelog || `- Fixes and small improvements for ${gameVersions[gameVersions.length - 1]}.`,
  files: [{ url: `https://cdn.example/${fileName}`, fileName, size, primary: true, sha1: `${projectId}-${n}` }],
  dependencies: deps.map((d) => ({ projectId: `p-${d.slug}`, versionId: null, kind: d.kind, title: null, slug: d.slug, iconUrl: null })),
})

const project = (p: Partial<CatalogEntry> & Pick<CatalogEntry, 'slug' | 'kind' | 'title' | 'author' | 'summary' | 'categories' | 'downloads' | 'follows'>): CatalogEntry => {
  const id = `p-${p.slug}`
  return {
    iconUrl: null,
    updated: daysAgo(12),
    published: daysAgo(700),
    gameVersions: ['1.20.1', '1.20.4', '1.21', '1.21.1'],
    loaders: p.kind === 'shader' ? ['iris', 'optifine'] : p.kind === 'resourcepack' ? ['minecraft'] : ['fabric', 'quilt', 'neoforge'],
    clientSide: 'required',
    serverSide: p.kind === 'mod' ? 'optional' : 'unsupported',
    license: 'MIT',
    featuredImage: null,
    body: `${p.summary}\n\nMore details soon.`,
    gallery: [],
    links: { source: `https://github.com/example/${p.slug}`, issues: `https://github.com/example/${p.slug}/issues`, wiki: null, discord: null, donation: [] },
    versions: [],
    ...p,
    id,
  }
}

const TRAILMARK_BODY = `Trailmark adds **trail markers**, **waypoint signs** and a **pocket map**, so you always find your way home, even after a long trip through the caves.

## What it adds

- **Trail markers.** Small posts you place along a path. They glow faintly at night and point to the next marker.
- **Waypoint signs.** Name a place once and it shows up on the pocket map and in the compass bar.
- **Pocket map.** A light map you open with a key, without holding an item. It shows your markers and signs.

## How to use it

1. Craft a marker with a stick and any dye. The dye picks its color.
2. Place markers every few blocks along your path.
3. Press **M** to open the pocket map.

## Good to know

**Does it work on servers?** Install it on the server too. Players without the mod can still join, but they won't see the markers.

**Can I remove it later?** Yes. Markers placed in your world disappear; the world itself stays safe.`

const catalog: CatalogEntry[] = [
  // mods
  project({ slug: 'trailmark', kind: 'mod', title: 'Trailmark', author: 'kestrel_dev', summary: 'Trail markers, waypoint signs and a pocket map, so you always find your way home.', categories: ['adventure', 'utility'], downloads: 2_300_000, follows: 18_400, body: TRAILMARK_BODY, gallery: [SCENES.plains, SCENES.sunset, SCENES.snow, SCENES.ocean, SCENES.night, SCENES.desert].map((url, i) => ({ url, title: ['A marked trail', 'Signs at dusk', 'Through the snow', 'Coastal path', 'Markers at night', 'Desert route'][i], description: null, featured: i === 0 })), featuredImage: SCENES.plains }),
  project({ slug: 'hearthside-kitchens', kind: 'mod', title: 'Hearthside Kitchens', author: 'oven_mitt', summary: 'Cooking stations, cutting boards and forty new foods that restore more hunger.', categories: ['food', 'decoration'], downloads: 1_100_000, follows: 9_800, featuredImage: SCENES.sunset }),
  project({ slug: 'quiet-nights', kind: 'mod', title: 'Quiet Nights', author: 'hushcraft', summary: 'Softer ambient sounds and calmer cave noises, with a volume slider for each.', categories: ['utility'], downloads: 640_000, follows: 4_100, serverSide: 'unsupported' }),
  project({ slug: 'rail-works', kind: 'mod', title: 'Rail Works', author: 'signalbox', summary: 'Signals, switches and stations for minecart networks that do not crash into each other.', categories: ['transportation', 'technology'], downloads: 820_000, follows: 6_300, featuredImage: SCENES.factory }),
  project({ slug: 'stack-sorter', kind: 'mod', title: 'Stack Sorter', author: 'tidybits', summary: 'Sort any inventory with one key, and move matching items into chests in one click.', categories: ['utility', 'storage'], downloads: 3_900_000, follows: 22_000, serverSide: 'optional' }),
  project({ slug: 'lantern-lights', kind: 'mod', title: 'Lantern Lights', author: 'glowworm', summary: 'Thirty decorative lamps, lanterns and string lights in every wood and metal.', categories: ['decoration'], downloads: 410_000, follows: 3_200 }),
  project({ slug: 'copper-tools', kind: 'mod', title: 'Copper Tools', author: 'oxide', summary: 'A copper tool set between stone and iron that slowly turns green.', categories: ['equipment'], downloads: 290_000, follows: 1_900 }),
  project({ slug: 'biome-atlas', kind: 'mod', title: 'Biome Atlas', author: 'cartograph', summary: 'An in-game guide to every biome: what grows there, what spawns and where to find it.', categories: ['utility', 'adventure'], downloads: 150_000, follows: 1_200, gameVersions: ['1.19.4', '1.20.1'] }),
  project({ slug: 'fabric-api', kind: 'mod', title: 'Fabric API', author: 'FabricMC', summary: 'Core library that most Fabric mods need.', categories: ['library'], downloads: 99_000_000, follows: 30_000, loaders: ['fabric', 'quilt'] }),
  project({ slug: 'cloth-config', kind: 'mod', title: 'Cloth Config', author: 'shedaniel', summary: 'Settings screens for other mods.', categories: ['library'], downloads: 60_000_000, follows: 12_000 }),
  project({ slug: 'sodium', kind: 'mod', title: 'Sodium', author: 'CaffeineMC', summary: 'A modern rendering engine: more frames, less stutter.', categories: ['optimization'], downloads: 50_000_000, follows: 25_000, loaders: ['fabric', 'quilt', 'neoforge'], serverSide: 'unsupported' }),
  project({ slug: 'iris', kind: 'mod', title: 'Iris Shaders', author: 'coderbot', summary: 'Runs shader packs on top of Sodium.', categories: ['decoration', 'optimization'], downloads: 40_000_000, follows: 20_000, serverSide: 'unsupported' }),
  project({ slug: 'mod-menu', kind: 'mod', title: 'Mod Menu', author: 'Prospector', summary: 'Lists your mods in game and opens their settings.', categories: ['utility'], downloads: 45_000_000, follows: 15_000, serverSide: 'unsupported' }),
  project({ slug: 'old-waypoints', kind: 'mod', title: 'Old Waypoints', author: 'legacy_dev', summary: 'Waypoint signs for older versions.', categories: ['utility'], downloads: 90_000, follows: 400, gameVersions: ['1.18.2', '1.19.2'] }),
  // modpacks
  project({ slug: 'hollow-valleys', kind: 'modpack', title: 'Hollow Valleys', author: 'the Hollow team', summary: 'A calm exploration and farming pack with deep valleys, cozy villages and light machines.', categories: ['adventure', 'technology'], downloads: 480_000, follows: 7_700, gameVersions: ['1.21.1'], loaders: ['fabric'], featuredImage: SCENES.plains, gallery: [SCENES.plains, SCENES.sunset, SCENES.snow, SCENES.night].map((url, i) => ({ url, title: ['Valley farms', 'Evening market', 'Mountain pass', 'Lantern festival'][i], description: null, featured: i === 0 })), body: 'A calm pack about exploring deep valleys, farming and building cozy villages, with a few light machines to save time.\n\n## What you will do\n\n- Explore valleys carved by rivers, with villages hidden at the bottom.\n- Farm forty new crops and cook them in proper kitchens.\n- Build water wheels and small mills that do the boring work.' }),
  project({ slug: 'sky-foundry', kind: 'modpack', title: 'Sky Foundry', author: 'aerie', summary: 'Start on a single block in the sky and automate your way to a floating factory.', categories: ['technology'], downloads: 310_000, follows: 5_200, gameVersions: ['1.20.1'], loaders: ['fabric'], featuredImage: SCENES.factory }),
  project({ slug: 'pocket-kingdoms', kind: 'modpack', title: 'Pocket Kingdoms', author: 'crownworks', summary: 'Light kingdom building with villagers who work, trade and grow your town.', categories: ['adventure'], downloads: 190_000, follows: 2_800, gameVersions: ['1.21.1'], loaders: ['fabric'], featuredImage: SCENES.sunset }),
  project({ slug: 'rails-and-rivers', kind: 'modpack', title: 'Rails & Rivers', author: 'signalbox', summary: 'Trains, boats and long journeys between hand-built towns.', categories: ['transportation'], downloads: 120_000, follows: 1_700, gameVersions: ['1.20.1'], loaders: ['fabric'], featuredImage: SCENES.ocean }),
  // shader packs
  project({ slug: 'glowmoss-shaders', kind: 'shader', title: 'Glowmoss Shaders', author: 'mossy_light', summary: "Soft shadows, waving plants and morning fog that keeps the game's look.", categories: ['realistic', 'atmosphere'], downloads: 900_000, follows: 11_000, featuredImage: SCENES.sunset, gallery: [SCENES.sunset, SCENES.plains, SCENES.night, SCENES.ocean, SCENES.snow, SCENES.desert].map((url, i) => ({ url, title: null, description: null, featured: i === 0 })) }),
  project({ slug: 'clearwater', kind: 'shader', title: 'Clearwater', author: 'tidepool', summary: 'Light shaders focused on clear water and reflections, gentle on older graphics cards.', categories: ['lightweight'], downloads: 400_000, follows: 3_900, featuredImage: SCENES.ocean }),
  project({ slug: 'paper-sun', kind: 'shader', title: 'Paper Sun', author: 'inkwell', summary: 'A hand-painted look with soft outlines and warm light.', categories: ['cartoon'], downloads: 210_000, follows: 2_600, featuredImage: SCENES.desert }),
  project({ slug: 'night-owl', kind: 'shader', title: 'Night Owl', author: 'lumen', summary: 'Darker, starry nights where torches really matter.', categories: ['atmosphere'], downloads: 150_000, follows: 1_500, featuredImage: SCENES.night }),
  // resource packs
  project({ slug: 'crisp-32x', kind: 'resourcepack', title: 'Crisp 32x', author: 'pixelwright', summary: "Minecraft's look, twice as sharp. Every block redrawn at 32 pixels.", categories: ['32x', 'realistic'], downloads: 1_700_000, follows: 14_000, gameVersions: ['1.20', '1.20.1', '1.20.4', '1.21', '1.21.1'], featuredImage: SCENES.snow, gallery: [SCENES.snow, SCENES.plains, SCENES.desert, SCENES.night].map((url, i) => ({ url, title: null, description: null, featured: i === 0 })) }),
  project({ slug: 'soft-edges-16x', kind: 'resourcepack', title: 'Soft Edges 16x', author: 'roundpix', summary: 'The default textures with softer edges and less noise.', categories: ['16x', 'simplistic'], downloads: 520_000, follows: 4_400, featuredImage: SCENES.plains }),
  project({ slug: 'bold-ui', kind: 'resourcepack', title: 'Bold UI', author: 'contrastkit', summary: 'A clearer, higher contrast interface for menus and inventories.', categories: ['gui'], downloads: 380_000, follows: 3_100 }),
  project({ slug: 'autumn-leaves', kind: 'resourcepack', title: 'Autumn Leaves', author: 'maplegrove', summary: 'Orange and red foliage all year round.', categories: ['themed'], downloads: 160_000, follows: 1_300, gameVersions: ['1.20', '1.20.1'], featuredImage: SCENES.sunset }),
]

for (const p of catalog) {
  const file = (v: string) => `${p.slug}-${v}.${p.kind === 'mod' ? 'jar' : p.kind === 'modpack' ? 'mrpack' : 'zip'}`
  const deps = p.slug === 'trailmark'
    ? [{ slug: 'fabric-api', kind: 'required' as const }, { slug: 'cloth-config', kind: 'required' as const }, { slug: 'mod-menu', kind: 'optional' as const }, { slug: 'old-waypoints', kind: 'incompatible' as const }]
    : p.kind === 'shader' ? [] : p.kind === 'mod' && !['fabric-api', 'cloth-config'].includes(p.slug) ? [{ slug: 'fabric-api', kind: 'required' as const }] : []
  const size = p.kind === 'modpack' ? 780e6 : p.kind === 'resourcepack' ? 48e6 : p.kind === 'shader' ? 4.1e6 : 1.2e6
  p.versions = [
    ver(p.id, 3, p.slug === 'trailmark' ? '2.4.0' : '1.3.0', p.gameVersions.slice(-2), p.loaders, file('3'), size, deps, 'release', p.slug === 'trailmark' ? '- Waypoint signs can now be renamed with a right click.\n- The pocket map remembers its zoom.\n- Fixed markers flickering under Sodium.' : ''),
    ver(p.id, 2, p.slug === 'trailmark' ? '2.4.0-beta.2' : '1.3.0-beta', p.gameVersions.slice(-2), p.loaders, file('2'), size, deps, 'beta'),
    ver(p.id, 1, p.slug === 'trailmark' ? '2.3.1' : '1.2.0', p.gameVersions.slice(0, 2), p.loaders, file('1'), size, deps),
  ]
}

const bySlug = (s: string) => catalog.find((p) => p.slug === s || p.id === s)
const hit = (p: CatalogEntry): ProjectHit => {
  const { body: _b, gallery: _g, links: _l, versions: _v, ...h } = p
  return h
}

/** Which mod files of an instance are which catalog project. */
const FILE_PROJECT: [RegExp, string][] = [
  [/^fabric-api/, 'fabric-api'], [/^sodium/, 'sodium'], [/^iris/, 'iris'], [/^modmenu/, 'mod-menu'], [/^cloth-config/, 'cloth-config'],
  [/^trailmark/, 'trailmark'], [/^hearthside/, 'hearthside-kitchens'], [/^stack-sorter/, 'stack-sorter'],
  [/^Glowmoss/, 'glowmoss-shaders'], [/^Clearwater/, 'clearwater'], [/^Crisp-32x/, 'crisp-32x'], [/^Soft-Edges/, 'soft-edges-16x'], [/^Bold-UI/, 'bold-ui'],
]

function sourcesOf(id: string): ContentSource[] {
  const files = [...modsOf(id).map((m) => ({ f: m.fileName, v: m.version ?? '' })), ...packsOf(id).resource.map((p) => ({ f: p.fileName, v: '2.1' })), ...packsOf(id).shader.map((p) => ({ f: p.fileName, v: '1.3' }))]
  const out: ContentSource[] = []
  for (const { f, v } of files) {
    const match = FILE_PROJECT.find(([re]) => re.test(f))
    const p = match && bySlug(match[1])
    if (p) out.push({ fileName: f, projectId: p.id, versionId: p.versions[0].id, slug: p.slug, title: p.title, iconUrl: p.iconUrl, versionNumber: v })
  }
  return out
}

let pendingUpdates: Record<string, ContentUpdate[]> = {
  'friends-smp': [
    { fileName: 'iris-fabric-1.7.3+mc1.21.1.jar', projectId: 'p-iris', title: 'Iris Shaders', currentVersion: '1.7.3', latestVersionId: 'v-iris-3', latestVersion: '1.8.0', changelog: '- Faster shader compilation.\n- Fixed water reflections with Sodium 0.6.' },
    { fileName: 'Jade-1.21.1-Fabric-15.1.3.jar', projectId: 'p-jade', title: 'Jade', currentVersion: '15.1.3', latestVersionId: 'v-jade-3', latestVersion: '15.2.0', changelog: '- Shows crop growth stages.' },
  ],
}

function compatible(v: ProjectVersion, i: Instance | undefined, kind: ProjectKind) {
  if (!i) return true
  if (!v.gameVersions.some((g) => g === i.minecraft || i.minecraft.startsWith(g + '.'))) return false
  if (kind !== 'mod') return true
  const loaders = i.loader.kind === 'quilt' ? ['quilt', 'fabric'] : [i.loader.kind]
  return v.loaders.some((l) => loaders.includes(l as never))
}

async function runTask(task: string, steps: [string, number][], stepMs = 70) {
  for (const [label, total] of steps) {
    for (let done = 0; done <= total; done += Math.max(1, Math.round(total / 14))) {
      emit({ type: 'progress', task, label, done: Math.min(done, total), total })
      await sleep(stepMs)
    }
  }
  emit({ type: 'taskFinished', task, error: null })
}

// game sessions
function pushLog(id: string, line: string, stream: 'stdout' | 'stderr' = 'stdout') {
  const list = logs.get(id) ?? []
  list.push({ line, stream, time: new Date().toISOString() })
  logs.set(id, list.slice(-5000))
  emit({ type: 'gameLog', instanceId: id, line, stream })
}

const stamp = () => new Date().toLocaleTimeString('en-GB', { hour12: false })

async function launch(id: string, server: string | null) {
  const i = instances.find((x) => x.id === id)
  if (!i || running.has(id)) return
  const task = `launch:${id}`
  if (!i.installed) {
    await runTask(task, [['Checking the version', 1], ['Downloading libraries', 304], ['Downloading assets', 412]], 60)
    i.installed = true
  } else {
    emit({ type: 'progress', task, label: 'Starting Minecraft', done: 1, total: 1 })
    await sleep(700)
    emit({ type: 'taskFinished', task, error: null })
  }
  const startedAt = new Date().toISOString()
  emit({ type: 'gameState', instanceId: id, running: true, exitCode: null })
  logs.set(id, [])
  const loader = i.loader.kind === 'vanilla' ? 'vanilla' : `${i.loader.kind} Loader ${i.loader.version ?? ''}`.trim()
  const crashes = id === 'better-minecraft'
  const lines: [string, 'stdout' | 'stderr'][] = [
    [`[main/INFO]: Loading Minecraft ${i.minecraft} with ${loader}`, 'stdout'],
    [`[main/INFO]: Loading ${modsOf(id).filter((m) => m.enabled).length} mods`, 'stdout'],
    ['[main/INFO]: Java 21.0.5 (Microsoft) · max memory 6144 MB', 'stdout'],
    ['[main/WARN]: Mod jade provides a broken optional dependency on wthit', 'stderr'],
    ['[Render thread/INFO]: Setting user: Nyx_Ember', 'stdout'],
    ['[Render thread/INFO]: Backend library: LWJGL version 3.3.3+5', 'stdout'],
    ['[Render thread/INFO]: [Sodium] Using renderer: OpenGL 4.6 (NVIDIA GeForce RTX 3060)', 'stdout'],
    ['[Render thread/INFO]: Reloading ResourceManager: vanilla, fabric, Crisp 32x, Bold UI', 'stdout'],
    ['[Render thread/INFO]: Created: 4096x2048x4 minecraft:textures/atlas/blocks.png-atlas', 'stdout'],
    ['[Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play', 'stderr'],
    [server ? `[Render thread/INFO]: Connecting to ${server}` : '[Server thread/INFO]: Preparing level "Lake Village"', 'stdout'],
    ['[Server thread/INFO]: Preparing spawn area: 84%', 'stdout'],
    ['[Server thread/INFO]: Done (4.812s)! For help, type "help"', 'stdout'],
    ['[Render thread/INFO]: Joined world Lake Village', 'stdout'],
    ['[Render thread/INFO]: [CHAT] Lou_Builds joined the game', 'stdout'],
    ['[Render thread/INFO]: Saved screenshot as 2026-11-02_18.42.11.png', 'stdout'],
  ]
  let n = 0
  const timer = window.setInterval(() => {
    if (crashes && n === 7) {
      for (const l of [
        '[Render thread/ERROR]: Unreported exception thrown!',
        'java.lang.NoSuchMethodError: \'void me.jellysquid.mods.sodium.client.render.chunk.ChunkRenderList.add(int)\'',
        '\tat com.simibubi.create.foundation.render.SuperByteBuffer.renderInto(SuperByteBuffer.java:142)',
        '\tat com.simibubi.create.content.kinetics.base.KineticBlockEntityRenderer.renderSafe(KineticBlockEntityRenderer.java:58)',
        '[Render thread/INFO]: Stopping!',
      ]) pushLog(id, `[${stamp()}] ${l}`, 'stderr')
      clearInterval(timer)
      running.delete(id)
      emit({ type: 'gameState', instanceId: id, running: false, exitCode: -1 })
      return
    }
    const [line, stream] = lines[n % lines.length]
    pushLog(id, `[${stamp()}] ${line}`, stream)
    n++
  }, 900)
  running.set(id, { startedAt, timer })
  i.lastPlayed = startedAt
}

function kill(id: string) {
  const game = running.get(id)
  if (!game) return
  clearInterval(game.timer)
  running.delete(id)
  pushLog(id, `[${stamp()}] [Cubby] The game was stopped.`, 'stderr')
  emit({ type: 'gameState', instanceId: id, running: false, exitCode: 1 })
}

const slug = (name: string) =>
  name.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'instance'

// modpack providers
const BLOCKED_SAMPLE: BlockedFile[] = [
  { fileName: 'create-aeronautics-1.20.1-0.9.2.jar', pageUrl: 'https://example.org/mc-mods/create-aeronautics/files/5530001', path: 'mods/create-aeronautics-1.20.1-0.9.2.jar', found: false },
  { fileName: 'FancyMenu_forge_3.3.0_MC_1.20.1.jar', pageUrl: 'https://example.org/mc-mods/fancymenu/files/5530002', path: 'mods/FancyMenu_forge_3.3.0_MC_1.20.1.jar', found: false },
  { fileName: 'Soft-Light-Shaders-v7.zip', pageUrl: 'https://example.org/shaders/soft-light/files/5530003', path: 'shaderpacks/Soft-Light-Shaders-v7.zip', found: false },
]
const blockedByInstance = new Map<string, BlockedFile[]>([['create-aab', structuredClone(BLOCKED_SAMPLE)]])

function findPack(provider: ProviderId, id: string): { id: ProviderId; def: MockPackDef } {
  const def = morePacks(provider, PACKS[provider], 45).find((d) => d.id === id)
  if (!def) throw `Pack not found: ${id}`
  return { id: provider, def }
}

function packSummary(provider: ProviderId, d: MockPackDef): PackSummary {
  return {
    id: d.id,
    name: d.name,
    summary: d.summary,
    authors: d.authors,
    iconUrl: null,
    websiteUrl: `https://example.org/${provider}/${d.id}`,
    downloads: d.downloads,
    updated: daysAgo(d.daysOld),
    minecraft: d.minecraft,
    loaders: d.loaders,
  }
}

// command handler
const capeState = { active: 'cape-migrator' as string | null }

export async function handle(command: string, args: Record<string, any>): Promise<unknown> {
  await sleep(60)
  switch (command) {
    case 'app_info':
      return { version: '0.1.0', dataDir: 'C:\\Users\\Nyx\\AppData\\Roaming\\Cubby', msaConfigured: true, msaSource: 'builtIn', curseforgeKey: settings.curseforgeApiKey ? 'settings' : null, totalMemoryMb: 32768, devBuild: true }
    case 'list_instances':
      return structuredClone(instances)
    case 'create_instance': {
      let id = slug(args.spec.name)
      while (instances.some((i) => i.id === id)) id += '-2'
      const created = inst({ ...args.spec, id, created: new Date().toISOString(), installed: false })
      instances = [...instances, created]
      return created
    }
    case 'update_instance':
      instances = instances.map((i) => (i.id === args.instance.id ? args.instance : i))
      return args.instance
    case 'duplicate_instance': {
      const src = instances.find((i) => i.id === args.id)!
      let id = slug(args.name)
      while (instances.some((i) => i.id === id)) id += '-2'
      const copy = { ...structuredClone(src), id, name: args.name, lastPlayed: null, totalPlayedSecs: 0 }
      instances = [...instances, copy]
      return copy
    }
    case 'delete_instance':
      instances = instances.filter((i) => i.id !== args.id)
      return
    case 'launch_instance':
      void launch(args.id, args.server)
      return
    case 'kill_instance':
      kill(args.id)
      return
    case 'running_games':
      return [...running.entries()].map(([instanceId, g]) => ({ instanceId, startedAt: g.startedAt }))
    case 'game_log':
      return logs.get(args.id) ?? []
    case 'open_folder':
    case 'open_url':
      return
    case 'list_versions':
      return versions
    case 'list_loader_versions':
      return ['0.16.5', '0.16.4', '0.16.3', '0.15.11'].map((version, i) => ({ version, stable: i !== 1 }))
    case 'list_mods':
      return structuredClone(modsOf(args.id))
    case 'set_mod_enabled': {
      const m = modsOf(args.id).find((x) => x.fileName === args.fileName)
      if (m) {
        m.enabled = args.enabled
        m.fileName = args.enabled ? m.fileName.replace(/\.disabled$/, '') : `${m.fileName}.disabled`
      }
      return
    }
    case 'remove_mod':
      modsByInstance.set(args.id, modsOf(args.id).filter((m) => m.fileName !== args.fileName))
      return
    case 'list_worlds':
      return args.id === 'friends-smp' ? structuredClone(worlds) : structuredClone(worlds.slice(0, 1))
    case 'list_screenshots':
      return structuredClone(screenshots.get(args.id) ?? [])
    case 'delete_screenshot':
      screenshots.set(args.id, (screenshots.get(args.id) ?? []).filter((s) => s.fileName !== args.fileName))
      return
    case 'repair_instance':
      await runTask(`repair:${args.id}`, [['Checking game files', 40]], 120)
      return
    case 'list_packs': {
      const list = structuredClone(packsOf(args.id)[args.kind as 'resource' | 'shader'])
      const order = resourceOrder[args.id]
      if (args.kind === 'resource' && order) {
        for (const p of list) p.enabled = order.includes(p.fileName)
        list.sort((a, b) => (order.indexOf(a.fileName) + 1 || 99) - (order.indexOf(b.fileName) + 1 || 99))
      }
      return list
    }
    case 'set_resource_packs':
      resourceOrder[args.id] = args.enabled
      return
    case 'remove_pack': {
      const list = packsOf(args.id)[args.kind as 'resource' | 'shader']
      const i = list.findIndex((p) => p.fileName === args.fileName)
      if (i < 0) throw `pack ${args.fileName} not found`
      list.splice(i, 1)
      return
    }
    case 'set_shader_pack':
      for (const p of packsOf(args.id).shader) p.enabled = p.fileName === args.fileName
      return
    case 'list_servers':
      return structuredClone(servers.get(args.id) ?? [])
    case 'save_servers':
      servers.set(args.id, args.servers)
      return
    case 'ping_server': {
      await sleep(250 + Math.random() * 350)
      const host = String(args.address)
      if (host.startsWith('192.168.1.42') || host.startsWith('localhost'))
        return { online: false, motd: '', playersOnline: 0, playersMax: 0, version: null, pingMs: null, iconUrl: null }
      if (host.startsWith('crew')) return { online: true, motd: 'Season 4 · back on Saturday', playersOnline: 5, playersMax: 20, version: '1.21.1', pingMs: 24, iconUrl: SERVER_ICONS.crew }
      if (host.includes('createfactory')) return { online: true, motd: 'Create Factory · 24/7 · whitelist on Discord', playersOnline: 87, playersMax: 100, version: '1.20.1', pingMs: 212, iconUrl: SERVER_ICONS.factory }
      return { online: true, motd: 'A Minecraft Server', playersOnline: host.length % 9, playersMax: 20, version: '1.21.1', pingMs: 40 + host.length, iconUrl: null }
    }
    case 'data_folders': {
      const root = 'C:\\Users\\Nyx\\AppData\\Roaming\\Cubby'
      return [
        { key: 'instances', label: 'Instances', path: `${root}\\instances`, sizeBytes: 6.2e9 },
        { key: 'libraries', label: 'Libraries', path: `${root}\\libraries`, sizeBytes: 1.1e9 },
        { key: 'assets', label: 'Game assets', path: `${root}\\assets`, sizeBytes: 1.8e9 },
        { key: 'java', label: 'Java runtimes', path: `${root}\\java`, sizeBytes: 0.5e9 },
        { key: 'meta', label: 'Metadata cache', path: `${root}\\meta`, sizeBytes: 0.2e9 },
        { key: 'logs', label: 'Launcher logs', path: `${root}\\logs`, sizeBytes: 12e6 },
        { key: 'trash', label: 'Trash', path: `${root}\\trash`, sizeBytes: 640e6 },
      ]
    }
    case 'open_data_folder':
    case 'clear_cache':
      return
    case 'clear_trash':
      return
    case 'skin_profile': {
      const url = (h: string) => `https://textures.minecraft.net/texture/${h}`
      return {
        skinUrl: `https://minotar.net/skin/MHF_Steve`,
        variant: 'classic',
        capes: [
          { id: 'cape-migrator', name: 'Migrator Cape', url: url('2340c0e03dd24a11b15a8b33c2a7e9e32abb2051b2481d0ba7defd635ca7a933'), active: capeState.active === 'cape-migrator' },
          { id: 'cape-vanilla', name: 'Vanilla Cape', url: url('f9a76537647989f9a0b6d001e320dac591c359e9e61a31f4ce11c88f207f0ad4'), active: capeState.active === 'cape-vanilla' },
          { id: 'cape-mojang', name: 'Mojang Studios Cape', url: url('9e507afc56359978a3eb3e32367042b853cddd0995d17d0da995662913fb00f7'), active: capeState.active === 'cape-mojang' },
        ],
      }
    }
    case 'set_active_cape':
      capeState.active = args.capeId
      return handle('skin_profile', args)
    case 'set_skin':
      return handle('skin_profile', args)
    case 'default_skins': {
      // dev only: the textures are read from ui/dev-skins when they were extracted there (never committed)
      const ids = ['steve', 'alex', 'sunny', 'noor', 'makena', 'kai', 'efe', 'zuri', 'ari']
      const bytes = async (path: string) => {
        const r = await fetch(path)
        if (!r.ok || !(r.headers.get('content-type') ?? '').includes('image/png')) throw new Error('missing')
        return [...new Uint8Array(await r.arrayBuffer())]
      }
      try {
        return await Promise.all(ids.map(async (id) => ({ id, name: id[0].toUpperCase() + id.slice(1), wide: await bytes(`/dev-skins/${id}-wide.png`), slim: await bytes(`/dev-skins/${id}-slim.png`) })))
      } catch {
        throw 'Cubby reads the default skins from the game files. Create an instance and start it once, then come back.'
      }
    }
    case 'list_accounts':
      return structuredClone(accounts)
    case 'start_microsoft_login':
      return { userCode: 'K7QX-M2PL', verificationUri: 'https://www.microsoft.com/link', expiresIn: 900, interval: 5, message: '' }
    case 'finish_microsoft_login':
      await sleep(4000)
      throw 'Microsoft sign-in is not available in the browser preview.'
    case 'cancel_microsoft_login':
      return
    case 'remove_account':
      accounts = accounts.filter((a) => a.id !== args.id)
      return
    case 'set_default_account':
      accounts = accounts.map((a) => ({ ...a, isDefault: a.id === args.id }))
      return
    case 'get_settings':
      return structuredClone(settings)
    case 'check_update':
      await sleep(700)
      return mockUpdate ? { ...mockUpdate } : null
    case 'download_update': {
      const total = 61 * 1024 * 1024
      for (let done = 0; done <= total; done += total / 20) {
        emit({ type: 'progress', task: 'self-update', label: 'Downloading Cubby 2026.11 alpha 2', done: Math.min(done, total), total })
        await sleep(150)
      }
      emit({ type: 'taskFinished', task: 'self-update', error: null })
      if (mockUpdate) mockUpdate.downloaded = true
      return
    }
    case 'install_update':
      if (running.size) throw 'Minecraft is still running. Close the game first.'
      return
    case 'save_settings':
      settings = args.settings
      return settings
    case 'list_java':
      return [
        { executable: 'C:\\Users\\Nyx\\AppData\\Roaming\\Cubby\\java\\java-runtime-delta\\bin\\javaw.exe', major: 21, version: '21.0.5', arch: 'x86_64', vendor: 'Microsoft', managed: true },
        { executable: 'C:\\Program Files\\Eclipse Adoptium\\jdk-17.0.12.7-hotspot\\bin\\javaw.exe', major: 17, version: '17.0.12', arch: 'x86_64', vendor: 'Eclipse Adoptium', managed: false },
        { executable: 'C:\\Program Files\\Eclipse Adoptium\\jre-8.0.422.5-hotspot\\bin\\javaw.exe', major: 8, version: '1.8.0_422', arch: 'x86_64', vendor: 'Eclipse Adoptium', managed: false },
      ]
    case 'list_prism_instances':
      return [
        { path: 'C:\\Users\\Nyx\\AppData\\Roaming\\PrismLauncher\\instances\\Skyfactory', name: 'Skyfactory', minecraft: '1.16.5', loader: { kind: 'forge', version: '36.2.39' }, group: 'Modpacks' },
        { path: 'C:\\Users\\Nyx\\AppData\\Roaming\\PrismLauncher\\instances\\Vanilla 1.21', name: 'Vanilla 1.21', minecraft: '1.21', loader: { kind: 'vanilla' }, group: null },
        { path: 'C:\\Users\\Nyx\\AppData\\Roaming\\PrismLauncher\\instances\\Fabulously Optimized', name: 'Fabulously Optimized', minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' }, group: null },
      ]
    case 'import_prism': {
      const imported = (args.paths as string[]).map((p) => {
        const name = p.split('\\').pop()!
        return inst({ id: slug(name), name, icon: null, minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' } })
      })
      instances = [...instances, ...imported.filter((n) => !instances.some((i) => i.id === n.id))]
      return imported
    }

    // Discover
    case 'curseforge_categories': {
      const lists: Record<string, string[]> = {
        mod: ['Adventure and RPG', 'Armor, Tools, and Weapons', 'Magic', 'Technology', 'World Gen'],
        shader: ['Realistic', 'Fantasy', 'Vanilla-like', 'Lightweight'],
        resourcepack: ['16x', '32x', '64x', 'Modern', 'Medieval'],
      }
      return (lists[args.kind as string] ?? []).map((name, i) => ({ id: String(100 + i), name }))
    }
    case 'curseforge_search': {
      const q = args.query as ProjectQuery
      await sleep(300)
      const names: Record<string, string[]> = {
        mod: ['Just Enough Items', 'Jade', 'Create', 'Applied Energistics 2', 'Waystones', 'Farmer’s Delight'],
        shader: ['Complementary Shaders', 'BSL Shaders', 'Rethinking Voxels'],
        resourcepack: ['Faithful 32x', 'Bare Bones', 'Sphax PureBDCraft'],
      }
      const all = (names[q.kind] ?? []).map((title, i): ProjectHit => ({
        id: String(238222 + i),
        slug: title.toLowerCase().replace(/[^a-z0-9]+/g, '-'),
        kind: q.kind,
        title,
        author: ['mezz', 'Snownee', 'simibubi', 'AE2', 'BlayTheNinth', 'vectorwing'][i % 6],
        summary: 'A sample result from CurseForge.',
        iconUrl: null,
        categories: [],
        downloads: 90_000_000 / (i + 1),
        follows: 0,
        updated: daysAgo(i * 3 + 1),
        published: daysAgo(i * 3 + 1),
        gameVersions: ['1.21.1', '1.20.1'],
        loaders: q.kind === 'mod' ? ['fabric', 'neoforge'] : [],
        clientSide: 'unknown',
        serverSide: 'unknown',
        license: null,
        featuredImage: null,
      }))
      const hits = all.filter((h) => h.title.toLowerCase().includes(q.query.trim().toLowerCase()))
      return { hits, total: hits.length, offset: q.offset, limit: q.limit }
    }
    case 'search_projects': {
      const q: ProjectQuery = args.query
      const words = q.query.trim().toLowerCase()
      let hits = catalog.filter((p) => p.kind === q.kind && !['fabric-api', 'cloth-config', 'old-waypoints'].includes(p.slug))
      if (words) hits = hits.filter((p) => `${p.title} ${p.summary} ${p.author}`.toLowerCase().includes(words))
      if (q.categories.length) hits = hits.filter((p) => q.categories.every((c) => p.categories.includes(c)))
      if (q.environment === 'server') hits = hits.filter((p) => p.serverSide !== 'unsupported')
      if (q.gameVersion) hits = hits.filter((p) => p.gameVersions.some((g) => g === q.gameVersion))
      if (q.loader && q.kind === 'mod') hits = hits.filter((p) => p.loaders.includes(q.loader!) || (q.loader === 'quilt' && p.loaders.includes('fabric')))
      const key: Record<string, (p: CatalogEntry) => number> = { downloads: (p) => -p.downloads, follows: (p) => -p.follows, newest: (p) => -Date.parse(p.published), updated: (p) => -Date.parse(p.updated), relevance: () => 0 }
      hits = [...hits].sort((a, b) => key[q.sort](a) - key[q.sort](b))
      return { hits: hits.slice(q.offset, q.offset + q.limit).map(hit), total: hits.length * 37, offset: q.offset, limit: q.limit }
    }
    case 'get_project': {
      const p = bySlug(args.idOrSlug)
      if (!p) throw `Project not found: ${args.idOrSlug}`
      const { versions: vs, ...rest } = structuredClone(p)
      return { ...rest, versionCount: vs.length * 6 }
    }
    case 'list_project_versions': {
      const p = bySlug(args.idOrSlug)
      if (!p) throw `Project not found: ${args.idOrSlug}`
      const vs = structuredClone(p.versions).filter((v) => (!args.gameVersion || v.gameVersions.includes(args.gameVersion)) && (!args.loader || p.kind !== 'mod' || v.loaders.includes(args.loader) || (args.loader === 'quilt' && v.loaders.includes('fabric'))))
      for (const v of vs) for (const d of v.dependencies) {
        const dp = d.slug ? bySlug(d.slug) : undefined
        d.title = dp?.title ?? d.slug
      }
      return vs
    }
    case 'plan_install': {
      const i = instances.find((x) => x.id === args.instanceId)
      const p = catalog.find((c) => c.versions.some((v) => v.id === args.versionId))
      if (!i || !p) throw 'Unknown instance or version.'
      const v = p.versions.find((x) => x.id === args.versionId)!
      const installed = new Set(sourcesOf(i.id).map((s) => s.projectId))
      const folder = (k: ProjectKind): InstallItem['folder'] => (k === 'shader' ? 'shaderpacks' : k === 'resourcepack' ? 'resourcepacks' : 'mods')
      const items: InstallItem[] = [{ projectId: p.id, versionId: v.id, title: p.title, versionNumber: v.versionNumber, fileName: v.files[0].fileName, size: v.files[0].size, reason: 'requested', installed: installed.has(p.id), folder: folder(p.kind) }]
      const missing: { projectId: string; title: string | null }[] = []
      const conflicts: { projectId: string; title: string | null; fileName: string }[] = []
      for (const d of v.dependencies) {
        const dp = d.projectId ? bySlug(d.projectId.slice(2)) : undefined
        if (d.kind === 'required') {
          const dv = dp?.versions.find((x) => compatible(x, i, dp.kind))
          if (!dp || !dv) missing.push({ projectId: d.projectId ?? '', title: dp?.title ?? null })
          else items.push({ projectId: dp.id, versionId: dv.id, title: dp.title, versionNumber: dv.versionNumber, fileName: dv.files[0].fileName, size: dv.files[0].size, reason: 'dependency', installed: installed.has(dp.id), folder: 'mods' })
        } else if (d.kind === 'incompatible' && dp && installed.has(dp.id)) {
          conflicts.push({ projectId: dp.id, title: dp.title, fileName: sourcesOf(i.id).find((s) => s.projectId === dp.id)!.fileName })
        }
      }
      return { instanceId: i.id, items, missing, conflicts, downloadBytes: items.filter((x) => !x.installed).reduce((s, x) => s + x.size, 0) }
    }
    case 'install_version': {
      const p = catalog.find((c) => c.versions.some((v) => v.id === args.versionId))!
      const v = p.versions.find((x) => x.id === args.versionId)!
      void (async () => {
        await runTask(`install:${args.instanceId}`, [['Resolving dependencies', 3], [`Downloading ${p.title}`, Math.round(v.files[0].size / 100_000)]], 60)
      })()
      const f = v.files[0].fileName
      if (p.kind === 'mod') {
        const list = modsOf(args.instanceId)
        if (!list.some((m) => m.fileName === f)) list.push({ fileName: f, enabled: true, size: v.files[0].size, name: p.title, version: v.versionNumber, description: p.summary, authors: [p.author] })
      } else {
        const kind = p.kind === 'shader' ? 'shader' : 'resource'
        const list = packsOf(args.instanceId)[kind]
        if (!list.some((x) => x.fileName === f)) list.push({ fileName: f, name: p.title, isDir: false, size: v.files[0].size, modified: new Date().toISOString(), enabled: false })
        FILE_PROJECT.push([new RegExp(`^${f.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`), p.slug])
      }
      if (!FILE_PROJECT.some(([re]) => re.test(f))) FILE_PROJECT.push([new RegExp(`^${p.slug}`), p.slug])
      return
    }
    case 'install_versions': {
      const ids: string[] = args.versionIds
      const found = ids.map((id) => {
        const p = catalog.find((c) => c.versions.some((v) => v.id === id))
        return p ? { p, v: p.versions.find((x) => x.id === id)! } : null
      }).filter((x): x is NonNullable<typeof x> => !!x)
      void runTask(`install:${args.instanceId}`, [['Resolving dependencies', 3], ['Downloading content', Math.max(3, found.reduce((n, { v }) => n + Math.round(v.files[0].size / 100_000), 0))]], 60)
      for (const { p, v } of found) {
        const f = v.files[0].fileName
        if (p.kind === 'mod') {
          const list = modsOf(args.instanceId)
          if (!list.some((m) => m.fileName === f)) list.push({ fileName: f, enabled: true, size: v.files[0].size, name: p.title, version: v.versionNumber, description: p.summary, authors: [p.author] })
        }
        if (!FILE_PROJECT.some(([re]) => re.test(f))) FILE_PROJECT.push([new RegExp(`^${p.slug}`), p.slug])
      }
      return
    }
    case 'describe_versions': {
      return (args.versionIds as string[]).flatMap((id) => {
        const p = catalog.find((c) => c.versions.some((v) => v.id === id))
        const v = p?.versions.find((x) => x.id === id)
        return p && v ? [{ versionId: v.id, projectId: p.id, slug: p.slug, title: p.title, iconUrl: p.iconUrl, versionNumber: v.versionNumber, projectType: p.kind, size: v.files[0].size, gameVersions: v.gameVersions, loaders: v.loaders }] : []
      })
    }
    case 'install_modpack': {
      const p = catalog.find((c) => c.versions.some((v) => v.id === args.versionId))!
      const v = p.versions.find((x) => x.id === args.versionId)!
      let id = slug(args.name)
      while (instances.some((i) => i.id === id)) id += '-2'
      const created = inst({ id, name: args.name, group: args.group, icon: args.icon, minecraft: v.gameVersions[v.gameVersions.length - 1], loader: { kind: 'fabric', version: '0.16.5' }, installed: false, created: new Date().toISOString(), modpack: { projectId: p.id, versionId: v.id, title: p.title, versionNumber: v.versionNumber } })
      instances = [...instances, created]
      void runTask(`modpack:${id}`, [['Reading the modpack', 2], ['Downloading 142 mods', 142], ['Copying settings and configs', 12]], 50)
      return created
    }
    case 'content_sources':
      return sourcesOf(args.instanceId)
    case 'check_updates':
      await sleep(300)
      return structuredClone(pendingUpdates[args.instanceId] ?? [])
    case 'apply_updates': {
      const list = pendingUpdates[args.instanceId] ?? []
      const chosen = list.filter((u) => (args.fileNames as string[]).includes(u.fileName))
      pendingUpdates = { ...pendingUpdates, [args.instanceId]: list.filter((u) => !chosen.includes(u)) }
      for (const u of chosen) {
        const m = modsOf(args.instanceId).find((x) => x.fileName === u.fileName)
        if (m) {
          m.version = u.latestVersion
          m.fileName = m.fileName.replace(u.currentVersion, u.latestVersion)
        }
      }
      void runTask(`update:${args.instanceId}`, [['Backing up your mods', 2], [`Updating ${chosen.length} mods`, chosen.length * 4]], 80)
      return
    }

    // Modpack providers
    case 'pack_providers':
      return PROVIDERS.map((p): PackProvider => (p.id === 'curseforge' && settings.curseforgeApiKey ? { ...p, available: true, reason: null } : structuredClone(p)))
    case 'pack_search': {
      await sleep(250)
      const id = args.provider as ProviderId
      if (id === 'curseforge' && !settings.curseforgeApiKey) throw 'CurseForge needs an API key. Add one in Settings, under Accounts.'
      const q = String(args.query).trim().toLowerCase()
      const paged = PROVIDERS.find((p) => p.id === id)!.paged
      let all = paged ? morePacks(id, PACKS[id], 45) : PACKS[id]
      if (q) all = all.filter((d) => `${d.name} ${d.summary} ${d.authors.join(' ')}`.toLowerCase().includes(q))
      all = [...all].sort(SORTERS[args.sort as PackSort] ?? SORTERS.relevance)
      return { hits: (paged ? all.slice(args.offset, args.offset + 20) : all).map((d) => packSummary(id, d)), total: paged ? all.length : null }
    }
    case 'pack_details': {
      const { id, def } = findPack(args.provider, args.id)
      await sleep(200)
      const details: PackDetails = {
        summary: packSummary(id, def),
        description: def.markdown ? PACK_MARKDOWN(def.name, def.summary) : PACK_DESCRIPTION(def.name, def.summary),
        descriptionFormat: def.markdown ? 'markdown' : 'text',
        links: [
          { label: `${def.name} on ${PROVIDERS.find((p) => p.id === id)!.name}`, url: `https://example.org/${id}/${def.id}` },
          ...(def.markdown ? [{ label: 'Discord', url: 'https://example.org/discord' }] : []),
        ],
        gallery: (def.gallery ?? []).map((n) => SCENES[n as SceneName]),
        warning: def.warning ?? null,
      }
      return details
    }
    case 'pack_versions': {
      const { def } = findPack(args.provider, args.id)
      await sleep(200)
      const n = def.versions ?? 5
      const versions: PackVersion[] = Array.from({ length: n }, (_, i) => ({
        id: `${def.id}-v${n - i}`,
        name: `${n - i}.${i === 0 ? 2 : 0}.${i}`,
        channel: i === 0 && def.id.length % 2 === 0 ? 'beta' : i === 3 ? 'alpha' : 'release',
        minecraft: def.minecraft[0] ?? null,
        loader: def.loaders[0] ?? null,
        released: daysAgo(def.daysOld + i * 21),
      }))
      return versions
    }
    case 'pack_optional_mods': {
      await sleep(150)
      const { id, def } = findPack(args.provider, args.id)
      return id === 'atlauncher' && def.optional ? structuredClone(OPTIONAL_MODS) : []
    }
    case 'install_pack': {
      const { id: provider, def } = findPack(args.provider, args.packId)
      if (def.warning) throw "This pack uses an old format that Cubby can't install."
      let id = slug(args.name || def.name)
      while (instances.some((i) => i.id === id)) id += '-2'
      const blockedCount = provider === 'curseforge' ? 2 : 0
      const created = inst({
        id,
        name: args.name || def.name,
        group: args.group,
        icon: args.icon,
        minecraft: def.minecraft[0] ?? '1.20.1',
        loader: { kind: def.loaders[0] ?? 'fabric', version: '1.0.0' },
        installed: false,
        created: new Date().toISOString(),
        blockedFiles: blockedCount,
      })
      instances = [...instances, created]
      if (blockedCount) blockedByInstance.set(id, structuredClone(BLOCKED_SAMPLE.slice(0, blockedCount)))
      void runTask(`modpack:${id}`, [['Reading the modpack', 2], ['Downloading 86 mods', 86], ['Copying settings and configs', 12]], 50)
      return created
    }
    case 'pick_pack_file':
      await sleep(300)
      return 'C:\\Users\\Nyx\\Downloads\\Hollow-Valleys-1.2.0.mrpack'
    case 'inspect_pack_file': {
      await sleep(300)
      const path = String(args.path)
      if (/broken/i.test(path)) throw "This file isn't a modpack Cubby knows: there is no modrinth.index.json or manifest.json inside."
      const cf = /curseforge|\.zip$/i.test(path)
      const info: PackFileInfo = cf
        ? {
            format: 'curseforge',
            name: 'Skyhaven',
            version: '3.4.1',
            minecraft: '1.20.1',
            loader: { kind: 'forge', version: '47.2.0' },
            fileCount: 212,
            blockedReason: settings.curseforgeApiKey ? null : 'This pack comes from CurseForge, and Cubby needs a CurseForge API key to download its mods. Add one in Settings, under Accounts.',
          }
        : { format: 'mrpack', name: 'Hollow Valleys', version: '1.2.0', minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' }, fileCount: 142, blockedReason: null }
      return info
    }
    case 'import_pack_file':
    case 'import_pack_url': {
      if (args.url && !/^https?:\/\//i.test(String(args.url))) throw 'Enter a web address that starts with http:// or https://.'
      let id = slug(args.name)
      while (instances.some((i) => i.id === id)) id += '-2'
      const created = inst({ id, name: args.name, group: args.group, icon: null, minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' }, installed: false, created: new Date().toISOString() })
      instances = [...instances, created]
      void runTask(`modpack:${id}`, [['Reading the modpack', 2], ['Downloading 142 mods', 142], ['Copying settings and configs', 12]], 50)
      return created
    }
    case 'blocked_files':
      await sleep(200)
      return structuredClone(blockedByInstance.get(args.instanceId) ?? [])
    case 'find_blocked_files': {
      await sleep(700)
      const list = blockedByInstance.get(args.instanceId) ?? []
      // pretend the first file is waiting in Downloads
      if (list[0]) list[0].found = true
      const i = instances.find((x) => x.id === args.instanceId)
      if (i) i.blockedFiles = list.filter((f) => !f.found).length
      return structuredClone(list)
    }

    // Worlds
    case 'backup_world': {
      const t = new Date()
      const pad = (v: number) => String(v).padStart(2, '0')
      const b: WorldBackup = { fileName: `${args.folder}-${t.getFullYear()}${pad(t.getMonth() + 1)}${pad(t.getDate())}-${pad(t.getHours())}${pad(t.getMinutes())}${pad(t.getSeconds())}.zip`, world: args.folder, created: t.toISOString(), sizeBytes: worlds.find((w) => w.folder === args.folder)?.sizeBytes ?? 1e8 }
      await sleep(900)
      backups = [b, ...backups]
      return b
    }
    case 'list_backups':
      return structuredClone(backups)
    case 'restore_backup':
      await sleep(900)
      return
    case 'delete_world':
      return
    default:
      throw `Unknown command: ${command}`
  }
}

export function subscribe(handler: (e: CubbyEvent) => void): () => void {
  listeners.add(handler)
  return () => listeners.delete(handler)
}

/** Sample images for screens that need a picture without a real file (dev preview only). */
export const MOCK_SCENES = SCENE_LIST
