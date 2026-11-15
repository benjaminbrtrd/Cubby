// Made-up contents of the preview's modpacks (`pack_contents`), so the Content sections have something to show.
import type { PackContentItem, PackContents, ProviderId } from '../types'

/** A catalog project of the mock, as far as a pack list needs it. */
export interface MockProject {
  id: string
  slug: string
  title: string
  kind: string
  iconUrl: string | null
  author: string
  summary: string
  categories: string[]
}

const MORE_MODS = [
  'Comforting Lamps', 'Jade', 'Farmer’s Delight', 'Create', 'Supplementaries', 'Storage Drawers', 'Xaero’s Minimap', 'Better Combat',
  'Comforts', 'Chipped', 'Macaw’s Bridges', 'Macaw’s Roofs', 'Polymorph', 'Clumps', 'FerriteCore', 'ModernFix',
  'Entity Culling', 'Lithium', 'Architectury API', 'Balm', 'Collective', 'Puzzles Lib', 'Searchables', 'Controlling',
  'Mouse Tweaks', 'Inventory Profiles Next', 'Botany Pots', 'Bountiful', 'Exposure', 'Spice of Life',
]

const slugOf = (name: string) => name.toLowerCase().replace(/[’']/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')

function sorted(items: PackContentItem[]): PackContentItem[] {
  const rank = { mod: 0, resourcepack: 1, shader: 2 }
  return items.sort((a, b) => rank[a.kind] - rank[b.kind] || a.name.toLowerCase().localeCompare(b.name.toLowerCase()))
}

function item(name: string, extra: Partial<PackContentItem> = {}): PackContentItem {
  const ext = extra.kind && extra.kind !== 'mod' ? 'zip' : 'jar'
  return {
    name,
    fileName: `${slugOf(name)}-1.4.2.${ext}`,
    version: null,
    kind: 'mod',
    iconUrl: null,
    site: null,
    projectId: null,
    pageUrl: null,
    optional: false,
    description: null,
    authors: [],
    categories: [],
    ...extra,
  }
}

/** A made-up author, description and category for a name of MORE_MODS. */
function blurb(name: string, i: number): Partial<PackContentItem> {
  const lib = /API|Lib|Balm|Collective|Architectury/.test(name)
  const perf = /Lithium|FerriteCore|ModernFix|Entity Culling|Clumps/.test(name)
  return {
    authors: [['tidybits', 'oven_mitt', 'glowworm', 'signalbox', 'cartograph'][i % 5]],
    description: lib ? 'Shared code that other mods in the pack need.' : perf ? 'Makes the game run faster with less memory.' : `Adds ${name.toLowerCase()} to the game, with its own settings screen.`,
    categories: lib ? ['library'] : perf ? ['optimization'] : ['utility'],
  }
}

/**
 * The contents of a version: Modrinth packs list the mock catalog's mods (they open in Cubby), CurseForge and FTB
 * packs CurseForge projects, ATLauncher packs named mods with optional ones, and the Technic pack "Bright Orchards"
 * is a single zip with no list. The oldest CurseForge version fails, to show the error.
 */
export function mockPackContents(provider: ProviderId, packId: string, versionId: string, catalog: MockProject[], cfMods: MockProject[]): PackContents {
  const n = Math.max(1, Number(/(\d+)$/.exec(versionId)?.[1] ?? 3))
  const extra = MORE_MODS.slice(0, 18 + n * 3)
  switch (provider) {
    case 'modrinth': {
      const own = catalog.filter((p) => ['mod', 'resourcepack', 'shader'].includes(p.kind)).slice(0, 14)
      const items = own.map((p, i) =>
        item(p.title, {
          kind: p.kind as PackContentItem['kind'],
          version: `${1 + (i % 4)}.${n}.${i % 7}`,
          iconUrl: p.iconUrl,
          site: 'modrinth',
          projectId: p.id,
          pageUrl: `https://modrinth.com/${p.kind}/${p.slug}`,
          optional: p.slug === 'iris' || p.kind === 'shader',
          authors: [p.author],
          description: p.summary,
          categories: p.categories,
        }),
      )
      items.push(item('custom-tweaks', { fileName: 'custom-tweaks-1.0.jar' }))
      return { items: sorted(items), unavailable: null }
    }
    case 'curseforge': {
      if (versionId.endsWith('-v1')) throw "The author of this modpack doesn't let launchers download it, so Cubby can't read which mods it has."
      const items = cfMods.map((p, i) =>
        item(p.title, {
          kind: p.kind as PackContentItem['kind'],
          version: i % 3 === 0 ? null : `${p.title} ${2 + (i % 3)}.${n}.${i}`,
          iconUrl: p.iconUrl,
          site: 'curseforge',
          projectId: p.id,
          pageUrl: `https://www.curseforge.com/minecraft/mc-mods/${p.slug}`,
          optional: i === 4,
          authors: [p.author],
          description: p.summary,
        }),
      )
      extra.forEach((name, i) => items.push(item(name, { ...blurb(name, i), site: 'curseforge', projectId: String(600000 + i), pageUrl: `https://www.curseforge.com/minecraft/mc-mods/${slugOf(name)}` })))
      return { items: sorted(items), unavailable: null }
    }
    case 'ftb':
      return {
        items: sorted(extra.map((name, i) => item(name, { ...blurb(name, i), site: 'curseforge', projectId: String(500000 + i), pageUrl: `https://www.curseforge.com/projects/${500000 + i}`, version: `1.20.1-${n}.${i}` }))),
        unavailable: null,
      }
    case 'atlauncher':
      return {
        items: sorted([
          ...extra.slice(0, 16).map((name, i) => item(name, { ...blurb(name, i), categories: [], version: `${n}.${i}.0`, pageUrl: i % 2 ? `https://example.org/mods/${slugOf(name)}` : null })),
          item('Shaders Mod', { version: '1', optional: true, description: 'Pretty light, if your graphics card keeps up.' }),
          item('Minimap', { version: '2.1', optional: true }),
          item('Classic Look', { kind: 'resourcepack', version: '1.0', optional: true }),
        ]),
        unavailable: null,
      }
    case 'technic':
      if (packId === 'bright-orchards') {
        return { items: [], unavailable: "This pack is a single download that doesn't list its mods. You'll see them in the instance once it's installed." }
      }
      return { items: sorted(extra.slice(0, 12).map((name, i) => item(name, { version: `1.12.2-${n}.${i}` }))), unavailable: null }
  }
}
