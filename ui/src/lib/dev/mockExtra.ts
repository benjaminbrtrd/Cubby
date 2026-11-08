// Browser dev mode: sample answers for the commands added with datapacks, update undo, exports, launcher
// imports, shared options, shortcuts, world maps and Discord. See mock.ts.
import type { Datapack, ProjectHit } from '../types'

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
  kind: 'mod',
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
  clientSide: 'unsupported',
  serverSide: 'required',
})) as unknown as ProjectHit[]

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
    case 'search_datapacks': {
      await sleep(300)
      const q = String(args.query ?? '').trim().toLowerCase()
      const hits = DATAPACK_HITS.filter((h) => !q || `${h.title} ${h.summary}`.toLowerCase().includes(q))
      return { hits, total: hits.length, offset: 0, limit: 20 }
    }
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
    default:
      return NOT_HANDLED
  }
}
