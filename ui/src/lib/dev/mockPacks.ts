// Fictional modpack catalogs for the browser preview: one per provider, plus a few optional mods for ATLauncher.
// Used by mock.ts only (never part of a real build).

import type { OptionalMod, PackLoaderKind, PackProvider, PackSort, ProviderId } from '../types'

export interface MockPackDef {
  id: string
  name: string
  summary: string
  authors: string[]
  downloads: number | null
  daysOld: number
  minecraft: string[]
  loaders: PackLoaderKind[]
  /** Names of scenes in mock.ts, used for the gallery. */
  gallery?: string[]
  warning?: string
  markdown?: boolean
  /** Number of versions to make up. */
  versions?: number
  /** Optional mods (ATLauncher). */
  optional?: boolean
}

const TEXT = (name: string, summary: string) =>
  `${summary}\n\n${name} is a made-up pack for the preview. It has a few dozen mods, a short quest book and a world that gets a little harder every day.\n\nYou don't need to know anything about modding to start: the first quest tells you what to craft.`

export const PACK_DESCRIPTION = TEXT

const MD = (name: string, summary: string) =>
  `${summary}\n\n## What you will do\n\n- Explore the world of **${name}**, one biome at a time.\n- Follow the quest book, or ignore it and build your own thing.\n- Play alone or with friends: it runs on a server too.\n\n## Good to know\n\nThe first start takes a few minutes, because every mod has to be set up. After that it opens in seconds.`

export const PACK_MARKDOWN = MD

export const PROVIDERS: PackProvider[] = [
  { id: 'atlauncher', name: 'ATLauncher', available: true, reason: null, sorts: ['relevance', 'name'], paged: false },
  {
    id: 'curseforge',
    name: 'CurseForge',
    available: false,
    reason: "CurseForge needs an API key, and this copy of Cubby doesn't have one. Get a free key from CurseForge and add it in Settings, under Accounts.",
    sorts: ['relevance', 'downloads', 'updated', 'newest', 'name'],
    paged: true,
  },
  { id: 'ftb', name: 'FTB', available: true, reason: null, sorts: ['relevance', 'downloads', 'updated'], paged: true },
  { id: 'modrinth', name: 'Modrinth', available: true, reason: null, sorts: ['relevance', 'downloads', 'updated', 'newest'], paged: true },
  { id: 'technic', name: 'Technic', available: true, reason: null, sorts: ['relevance'], paged: false },
]

const p = (
  id: string,
  name: string,
  summary: string,
  authors: string,
  downloads: number | null,
  daysOld: number,
  minecraft: string,
  loaders: PackLoaderKind[],
  extra: Partial<MockPackDef> = {},
): MockPackDef => ({ id, name, summary, authors: authors.split(', '), downloads, daysOld, minecraft: minecraft.split(', '), loaders, ...extra })

export const PACKS: Record<ProviderId, MockPackDef[]> = {
  atlauncher: [
    p('ember-isles', 'Ember Isles', 'Island hopping in a volcanic sea, with light magic and a lot of fire.', 'Atlas Crew', null, 14, '1.20.1', ['forge'], { optional: true, gallery: ['sunset', 'ocean', 'nether'] }),
    p('quiet-meadows', 'Quiet Meadows', 'Farming, fishing and cozy villages. Nothing here wants to hurt you.', 'meadowfolk', null, 40, '1.19.2', ['forge'], { optional: true, gallery: ['plains', 'sunset'] }),
    p('deep-rock', 'Deep Rock', 'Dig down, find ore, build machines. A long tech pack for patient players.', 'drillbit', null, 120, '1.12.2', ['forge'], { optional: true }),
    p('lantern-run', 'Lantern Run', 'Short adventure pack for 1.21 with dungeons and a few bosses.', 'Atlas Crew', null, 6, '1.21.1', ['neoforge'], { optional: true }),
    p('chrome-and-moss', 'Chrome & Moss', 'Overgrown factories and a quiet robot friend.', 'ferrite', null, 70, '1.20.1', ['fabric'], { optional: true, gallery: ['factory', 'plains'] }),
  ],
  curseforge: [
    p('120001', 'Hollow Kingdoms', 'Rebuild a fallen kingdom, one village at a time.', 'crownworks', 8_400_000, 9, '1.20.1', ['forge'], { gallery: ['sunset', 'night'] }),
    p('120002', 'Skyhaven', 'A sky pack with floating towns, airships and a long tech tree.', 'aerie', 5_100_000, 22, '1.20.1', ['forge']),
    p('120003', 'Mossy Depths', 'Cave exploration with dangerous new biomes.', 'mosswright', 2_300_000, 51, '1.19.2', ['forge', 'fabric']),
    p('120004', 'Copper Age', 'Tinker with copper, brass and steam, in the style of an old factory.', 'oxide', 1_950_000, 80, '1.18.2', ['forge']),
  ],
  ftb: [
    p('ftb-1', 'FTB Skyline', 'The classic sky start: one tree, one cobble generator, and a long way up.', 'FTB Team', 12_000_000, 4, '1.20.1', ['neoforge'], { markdown: true, gallery: ['factory', 'snow'] }),
    p('ftb-2', 'FTB Evolution Lite', 'A lighter take on the big kitchen-sink pack, with far fewer mods.', 'FTB Team', 7_300_000, 30, '1.19.2', ['forge'], { markdown: true }),
    p('ftb-3', 'FTB Oceanblock', 'Start on a tiny raft and build a whole harbour town.', 'FTB Team', 3_900_000, 90, '1.18.2', ['forge'], { markdown: true, gallery: ['ocean'] }),
    p('ftb-4', 'FTB Stoneblock Nova', 'Underground tech. The sky is made of stone.', 'FTB Team', 2_700_000, 160, '1.16.5', ['forge'], { markdown: true }),
  ],
  modrinth: [
    p('hollow-valleys', 'Hollow Valleys', 'A calm exploration and farming pack with deep valleys, cozy villages and light machines.', 'the Hollow team', 480_000, 12, '1.21.1', ['fabric'], { markdown: true, gallery: ['plains', 'sunset', 'snow', 'night'] }),
    p('sky-foundry', 'Sky Foundry', 'Start on a single block in the sky and automate your way to a floating factory.', 'aerie', 310_000, 30, '1.20.1', ['fabric'], { markdown: true }),
    p('pocket-kingdoms', 'Pocket Kingdoms', 'Light kingdom building with villagers who work, trade and grow your town.', 'crownworks', 190_000, 55, '1.21.1', ['fabric', 'quilt'], { markdown: true }),
    p('rails-and-rivers', 'Rails & Rivers', 'Trains, boats and long journeys between hand-built towns.', 'signalbox', 120_000, 100, '1.20.1', ['fabric'], { markdown: true }),
  ],
  technic: [
    p('tekkit-lite', 'Tekkit Lite', 'The old tech pack: a few dozen industrial mods, nothing more.', 'Slowpoke', 6_200_000, 600, '1.12.2', ['forge'], { warning: 'This pack uses an old format (jar mods) that Cubby can\'t install.' }),
    p('vault-and-voltage', 'Vault & Voltage', 'Power lines, vaults and a creeping sense of dread.', 'voltlab', 1_450_000, 33, '1.12.2', ['forge']),
    p('bright-orchards', 'Bright Orchards', 'Gentle orchard life with bees, jam and tiny machines.', 'orchardhand', 640_000, 61, '1.16.5', ['forge']),
    p('wild-frontier', 'Wild Frontier', 'Survival with hard winters and a lot of wolves.', 'northline', 280_000, 140, '1.7.10', ['forge']),
  ],
}

/** Extra made-up packs so a paged provider has a second page. */
export function morePacks(provider: ProviderId, base: MockPackDef[], total: number): MockPackDef[] {
  const out = [...base]
  for (let i = base.length; i < total; i++) {
    const b = base[i % base.length]
    out.push({ ...b, id: `${b.id}-${i}`, name: `${b.name} ${['Reborn', 'Lite', 'Classic', 'Remix', 'Plus', 'Zero'][i % 6]}`, downloads: b.downloads == null ? null : Math.round(b.downloads / (i + 1)), daysOld: b.daysOld + i * 9 })
  }
  void provider
  return out
}

export const SORTERS: Record<PackSort, (a: MockPackDef, b: MockPackDef) => number> = {
  relevance: () => 0,
  downloads: (a, b) => (b.downloads ?? 0) - (a.downloads ?? 0),
  updated: (a, b) => a.daysOld - b.daysOld,
  newest: (a, b) => a.daysOld - b.daysOld,
  name: (a, b) => a.name.localeCompare(b.name),
}

export const OPTIONAL_MODS: OptionalMod[] = [
  { name: 'Better Minimap', description: 'A minimap in the corner of the screen, with waypoints.', selected: true, recommended: true, group: null, depends: [], warning: null },
  { name: 'Shader Support', description: 'Lets you run shader packs. Needs a decent graphics card.', selected: false, recommended: false, group: null, depends: [], warning: 'Can slow the game down on older computers.' },
  { name: 'Shader Pack: Soft Light', description: 'A gentle shader pack with soft shadows.', selected: false, recommended: false, group: null, depends: ['Shader Support'], warning: null },
  { name: 'Music: Calm', description: 'Quiet background music for building.', selected: true, recommended: false, group: 'Music', depends: [], warning: null },
  { name: 'Music: Epic', description: 'Big orchestral tracks for fights and travel.', selected: false, recommended: false, group: 'Music', depends: [], warning: null },
  { name: 'Voice Chat', description: 'Talk to nearby players with your microphone.', selected: false, recommended: true, group: null, depends: [], warning: 'The server needs the same mod, or it does nothing.' },
]
