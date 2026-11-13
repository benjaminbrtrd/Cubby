// Browser dev mode: sample answers for the commands added with datapacks, update undo, exports, launcher
// imports, shared options, shortcuts, world maps and Discord. See mock.ts.
import type { Datapack, Project, ProjectHit, ProjectVersion, UpdateBackup } from '../types'

export const NOT_HANDLED = Symbol('not handled')

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

const datapacks = new Map<string, Datapack[]>([
  [
    'friends-smp/Lake Village',
    [
      { fileName: 'Terralith_1.21_v2.5.5.zip', name: 'Terralith_1.21_v2.5.5', isDir: false, enabled: true, size: 2_310_000, modified: '2026-10-02T18:00:00Z', description: 'Explore almost 100 new biomes' },
      { fileName: 'coordinates hud', name: 'coordinates hud', isDir: true, enabled: false, size: 12_400, modified: '2026-09-12T10:00:00Z', description: 'Shows your coordinates' },
    ],
  ],
])

const DATAPACK_HITS: ProjectHit[] = [
  ['terralith', 'Terralith', 'Stardust Labs', 'Explore almost 100 new biomes consisting of both realism and light fantasy.', 9_800_000],
  ['incendium', 'Incendium', 'Stardust Labs', 'A Nether overhaul with new biomes, structures and items.', 4_100_000],
  ['dungeons-and-taverns', 'Dungeons and Taverns', 'nova_wostra', 'New structures to find on your travels.', 6_300_000],
  ['veinminer', 'Veinminer', 'MiraculixxT', 'Mine a whole vein of ore at once.', 2_900_000],
].map(([slug, title, author, summary, downloads]) => ({
  id: `dp-${slug}`,
  slug: slug as string,
  title: title as string,
  author: author as string,
  summary: summary as string,
  iconUrl: null,
  categories: ['datapack'],
  downloads: downloads as number,
  follows: Math.round((downloads as number) / 300),
  updated: '2026-10-20T10:00:00Z',
  published: '2023-05-01T10:00:00Z',
  gameVersions: ['1.21.1'],
  loaders: ['datapack'],
  kind: 'datapack',
  clientSide: 'unsupported',
  serverSide: 'required',
  license: 'MIT',
  featuredImage: null,
})) as unknown as ProjectHit[]

const datapackHit = (idOrSlug: string) => DATAPACK_HITS.find((h) => h.id === idOrSlug || h.slug === idOrSlug)

function datapackVersions(h: ProjectHit): ProjectVersion[] {
  return ['1.21.1', '1.20.1'].map((mc, i) => ({
    id: `${h.id}-v${i}`,
    projectId: h.id,
    name: `${h.title} ${mc}`,
    versionNumber: `2.${5 - i}.0`,
    channel: 'release',
    gameVersions: [mc],
    loaders: ['datapack'],
    published: new Date(Date.now() - (i + 1) * 9 * 86400e3).toISOString(),
    downloads: 1000,
    changelog: '- New structures\n- Fixes',
    files: [{ url: 'https://cdn.example/x.zip', fileName: `${h.slug}-${mc}.zip`, size: 2_100_000, primary: true, sha1: 'a'.repeat(40) }],
    dependencies: [],
  }))
}

/** Discover › Datapacks in the browser: searches, project pages and versions of the sample datapacks. */
export function handleDatapackProject(command: string, args: Record<string, any>): unknown {
  if (command === 'search_projects' && args.query?.kind === 'datapack') {
    const q = String(args.query.query ?? '').trim().toLowerCase()
    const hits = DATAPACK_HITS.filter((h) => !q || `${h.title} ${h.summary}`.toLowerCase().includes(q))
    return Promise.resolve({ hits: structuredClone(hits), total: hits.length, offset: 0, limit: 20 })
  }
  const hit = (command === 'get_project' || command === 'list_project_versions') && datapackHit(args.idOrSlug)
  if (!hit) return NOT_HANDLED
  if (command === 'list_project_versions') return Promise.resolve(datapackVersions(hit))
  const project: Project = { ...structuredClone(hit), body: hit.summary, gallery: [], links: { source: null, issues: null, wiki: null, discord: null, donation: [] }, versionCount: 2 }
  return Promise.resolve(project)
}

let updateBackups: UpdateBackup[] = [
  {
    name: 'content-20261106-201500',
    created: '2026-11-06T19:15:00Z',
    files: [
      { folder: 'mods', old: 'sodium-fabric-0.5.11+mc1.21.jar', new: 'sodium-fabric-0.6.0+mc1.21.1.jar' },
      { folder: 'mods', old: 'lithium-fabric-0.13.0.jar', new: 'lithium-fabric-0.14.3.jar' },
    ],
  },
]

export async function handleExtra(command: string, args: Record<string, any>): Promise<unknown> {
  switch (command) {
    case 'list_datapacks':
      await sleep(150)
      return structuredClone(datapacks.get(`${args.id}/${args.world}`) ?? [])
    case 'set_datapack_enabled': {
      const p = datapacks.get(`${args.id}/${args.world}`)?.find((d) => d.fileName === args.fileName)
      if (p) p.enabled = args.enabled
      return
    }
    case 'remove_datapack': {
      const key = `${args.id}/${args.world}`
      datapacks.set(key, (datapacks.get(key) ?? []).filter((d) => d.fileName !== args.fileName))
      return
    }
    case 'add_datapack_files':
      return []
    case 'install_datapack': {
      await sleep(800)
      const hit = DATAPACK_HITS.find((h) => h.id === args.projectId)
      const fileName = `${hit?.title ?? 'Datapack'}.zip`
      const key = `${args.id}/${args.world}`
      const list = (datapacks.get(key) ?? []).filter((d) => d.fileName !== fileName)
      list.push({ fileName, name: fileName.slice(0, -4), isDir: false, enabled: true, size: 1_200_000, modified: new Date().toISOString(), description: hit?.summary ?? null })
      datapacks.set(key, list)
      return fileName
    }
    case 'list_update_backups':
      return args.instanceId === 'friends-smp' ? structuredClone(updateBackups) : []
    case 'undo_update': {
      await sleep(700)
      const b = updateBackups.find((x) => x.name === args.name)
      updateBackups = updateBackups.filter((x) => x.name !== args.name)
      return b?.files.length ?? 0
    }
    case 'export_candidates':
      return [
        { name: 'config', isDir: true, size: 1_840_000, suggested: true },
        { name: 'options.txt', isDir: false, size: 5_600, suggested: true },
        { name: 'saves', isDir: true, size: 412_000_000, suggested: false },
        { name: 'servers.dat', isDir: false, size: 420, suggested: false },
      ]
    case 'export_instance':
      await sleep(1200)
      return { path: `/Users/you/Downloads/${args.request.name} ${args.request.version}.${args.request.format === 'mrpack' ? 'mrpack' : 'zip'}`, listed: 34, stored: 6, withoutKey: false }
    case 'reveal_file':
      return
    case 'list_launcher_instances': {
      await sleep(400)
      const prism = 'C:\\Users\\Nyx\\AppData\\Roaming\\PrismLauncher\\instances\\'
      return [
        { launcher: 'prism', launcherName: 'Prism Launcher', path: `${prism}Skyfactory`, name: 'Skyfactory', minecraft: '1.16.5', loader: { kind: 'forge', version: '36.2.39' }, group: 'Modpacks', gameDir: `${prism}Skyfactory\\minecraft` },
        { launcher: 'prism', launcherName: 'Prism Launcher', path: `${prism}Vanilla 1.21`, name: 'Vanilla 1.21', minecraft: '1.21', loader: { kind: 'vanilla' }, group: null, gameDir: `${prism}Vanilla 1.21\\minecraft` },
        { launcher: 'curseForge', launcherName: 'CurseForge', path: 'C:\\Users\\Nyx\\curseforge\\minecraft\\Instances\\All the Mods 9', name: 'All the Mods 9', minecraft: '1.20.1', loader: { kind: 'forge', version: '47.3.0' }, group: null, gameDir: 'C:\\Users\\Nyx\\curseforge\\minecraft\\Instances\\All the Mods 9' },
        { launcher: 'modrinthApp', launcherName: 'Modrinth App', path: 'C:\\Users\\Nyx\\AppData\\Roaming\\ModrinthApp\\profiles\\Fabulously Optimized', name: 'Fabulously Optimized', minecraft: null, loader: { kind: 'vanilla' }, group: null, gameDir: 'C:\\Users\\Nyx\\AppData\\Roaming\\ModrinthApp\\profiles\\Fabulously Optimized' },
        { launcher: 'official', launcherName: 'Minecraft Launcher', path: 'C:\\Users\\Nyx\\AppData\\Roaming\\.minecraft\\launcher_profiles.json#a1', name: 'Fabric 1.21.1', minecraft: '1.21.1', loader: { kind: 'fabric', version: '0.16.5' }, group: null, gameDir: 'C:\\Users\\Nyx\\AppData\\Roaming\\.minecraft' },
      ]
    }
    case 'copy_options':
      await sleep(300)
      return { ...args.what }
    case 'create_desktop_shortcut':
      return `/Users/you/Desktop/${args.id}.app`
    case 'render_world_map': {
      await sleep(900)
      return { url: mockMap(), minX: -384, minZ: -384, size: 768, chunks: 1840 }
    }
    default:
      return NOT_HANDLED
  }
}

/** A made-up landscape: water, sand, grass and forest from a few waves. */
function mockMap(): string {
  const size = 96
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = size
  const ctx = canvas.getContext('2d')!
  for (let z = 0; z < size; z++) {
    for (let x = 0; x < size; x++) {
      const h = Math.sin(x / 9) + Math.cos(z / 11) + Math.sin((x + z) / 17) * 0.8
      ctx.fillStyle = h < -0.6 ? '#345cc4' : h < -0.35 ? '#dbcfa3' : h < 1 ? (Math.random() < 0.06 ? '#376e23' : '#76a846') : h < 1.6 ? '#376e23' : '#7d7d7d'
      ctx.fillRect(x, z, 1, 1)
    }
  }
  return canvas.toDataURL()
}
