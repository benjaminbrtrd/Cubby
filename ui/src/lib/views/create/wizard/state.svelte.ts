// The New instance wizard's choices, kept at module level: leaving the screen and coming back keeps them, until
// an instance is created.
import { t } from '../../../i18n.svelte'
import { api } from '../../../api'
import { loaderName } from '../../../format'
import type { Ground, Motif } from '../../../cubby'
import type { LoaderKind, PackProvider, PackSummary, PackVersion, ProviderId, WizardExtra } from '../../../types'

export type WizardPath = 'custom' | 'modpack' | 'import'
export type StepId = 'start' | 'game' | 'mods' | 'looks' | 'datapacks' | 'review'
export type PickKind = WizardExtra['kind']

/** A project picked for the new instance. */
export interface Pick extends WizardExtra {
  iconUrl: string | null
  author: string
}

export interface StepInfo {
  id: StepId
  label: string
  sub: string
  optional: boolean
}

export const STEP_ORDER: StepId[] = ['start', 'game', 'mods', 'looks', 'datapacks', 'review']

/** Mods several players add first, by Modrinth slug: sets of quick picks. */
export const QUICK_PICKS: { id: string; label: string; icon: 'update' | 'cube' | 'globe'; mods: { slug: string; title: string; author: string }[] }[] = [
  {
    id: 'performance',
    label: 'Performance set',
    icon: 'update',
    mods: [
      { slug: 'sodium', title: 'Sodium', author: 'CaffeineMC' },
      { slug: 'lithium', title: 'Lithium', author: 'CaffeineMC' },
      { slug: 'ferrite-core', title: 'FerriteCore', author: 'malte0811' },
      { slug: 'entityculling', title: 'EntityCulling', author: 'tr7zw' },
    ],
  },
  {
    id: 'shaders',
    label: 'Ready for shaders',
    icon: 'cube',
    mods: [
      { slug: 'iris', title: 'Iris Shaders', author: 'coderbot, IMS' },
      { slug: 'sodium', title: 'Sodium', author: 'CaffeineMC' },
    ],
  },
  {
    id: 'map',
    label: 'Map and info',
    icon: 'globe',
    mods: [
      { slug: 'xaeros-minimap', title: 'Xaero’s Minimap', author: 'xaero96' },
      { slug: 'jade', title: 'Jade', author: 'Snownee' },
    ],
  },
]

export const pickKey = (p: Pick | WizardExtra) => `${p.source}:${p.kind}:${p.projectId}`

class WizardState {
  step = $state<StepId>('start')
  path = $state<WizardPath>('custom')
  /** Import: from a file, or from a link. */
  importFrom = $state<'file' | 'link'>('file')

  // custom
  minecraft = $state<string | null>(null)
  loader = $state<LoaderKind>('fabric')
  loaderVersion = $state<string | null>(null)

  // modpack
  provider = $state<ProviderId>('modrinth')
  pack = $state<PackSummary | null>(null)
  packVersions = $state<PackVersion[] | null>(null)
  packVersionId = $state<string | null>(null)
  packVersionsError = $state<string | null>(null)

  picks = $state<Pick[]>([])
  turnOn = $state(true)

  // identity
  name = $state('')
  nameTouched = $state(false)
  group = $state('')
  motif = $state<Motif | undefined>()
  ground = $state<Ground | undefined>()
  src = $state<string | null>(null)
  iconTouched = $state(false)

  get packVersion(): PackVersion | null {
    return this.packVersions?.find((v) => v.id === this.packVersionId) ?? null
  }

  /** The Minecraft version content is looked up for. */
  get gameVersion(): string | null {
    return this.path === 'modpack' ? (this.packVersion?.minecraft ?? this.pack?.minecraft[0] ?? null) : this.minecraft
  }

  /** The loader content is looked up for; null when a pack doesn't say. */
  get gameLoader(): LoaderKind | null {
    if (this.path !== 'modpack') return this.loader
    return this.packVersion?.loader ?? this.pack?.loaders[0] ?? null
  }

  get vanilla(): boolean {
    return this.path === 'custom' && this.loader === 'vanilla'
  }

  get defaultName(): string {
    if (this.path === 'modpack') return this.pack?.name ?? t('New modpack')
    if (!this.minecraft) return t('New instance')
    return `${this.loader === 'vanilla' ? 'Minecraft' : loaderName(this.loader)} ${this.minecraft}`
  }

  get shownName(): string {
    return this.name.trim() || this.defaultName
  }

  /** Every step but Start is shown for building or a pack; an import only has Start. */
  get steps(): StepInfo[] {
    if (this.path === 'import') return [{ id: 'start', label: t('Start'), sub: t('Import'), optional: false }]
    const pack = this.path === 'modpack'
    const game = this.gameVersion ? `${this.gameVersion}${this.gameLoader ? ` · ${loaderName(this.gameLoader)}` : ''}` : t('Not picked yet')
    return [
      { id: 'start', label: t('Start'), sub: pack ? t('From a modpack') : t('Build your own'), optional: false },
      { id: 'game', label: pack ? t('Modpack') : t('Game'), sub: pack ? (this.pack?.name ?? t('Not picked yet')) : game, optional: false },
      { id: 'mods', label: pack ? t('Extra mods') : t('Mods'), sub: t('Optional'), optional: true },
      { id: 'looks', label: t('Looks'), sub: t('Optional'), optional: true },
      { id: 'datapacks', label: t('Datapacks'), sub: t('Optional'), optional: true },
      { id: 'review', label: t('Review'), sub: t('Name and create'), optional: false },
    ]
  }

  picked(kind: PickKind): Pick[] {
    return this.picks.filter((p) => p.kind === kind)
  }

  /** Picks a modpack, with one of its versions: the one given (from its page), else its newest release once loaded. */
  choosePack(provider: ProviderId, pack: PackSummary, versions?: PackVersion[] | null, versionId?: string | null) {
    const same = this.pack?.id === pack.id && this.provider === provider
    this.provider = provider
    this.pack = pack
    if (versions?.length) {
      this.packVersions = versions
      this.packVersionId = versionId ?? (versions.find((v) => v.channel === 'release') ?? versions[0]).id
      this.packVersionsError = null
      return
    }
    if (same && this.packVersions) return
    this.packVersions = null
    this.packVersionId = null
    this.packVersionsError = null
    api
      .packVersions(provider, pack.id)
      .then((list) => {
        if (this.pack?.id !== pack.id) return
        this.packVersions = list
        this.packVersionId = (list.find((v) => v.channel === 'release') ?? list[0])?.id ?? null
      })
      .catch((e) => {
        if (this.pack?.id === pack.id) this.packVersionsError = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
      })
  }

  has(p: Pick | WizardExtra): boolean {
    return this.picks.some((x) => pickKey(x) === pickKey(p))
  }

  toggle(p: Pick) {
    if (this.has(p)) this.remove(p)
    else this.add(p)
  }

  add(p: Pick) {
    if (this.has(p)) return
    // one shader pack at a time
    const rest = p.kind === 'shader' ? this.picks.filter((x) => x.kind !== 'shader') : this.picks
    this.picks = [...rest, p]
  }

  remove(p: Pick | WizardExtra) {
    this.picks = this.picks.filter((x) => pickKey(x) !== pickKey(p))
  }

  /** The shader pack needs Iris and the player hasn't added it (a pack may have it: not checked there). */
  get shaderNeedsIris(): boolean {
    return this.path === 'custom' && this.picked('shader').length > 0 && !this.picks.some((p) => p.kind === 'mod' && p.source === 'modrinth' && p.projectId === 'iris')
  }

  addIris() {
    for (const m of QUICK_PICKS.find((q) => q.id === 'shaders')!.mods) this.add({ source: 'modrinth', projectId: m.slug, kind: 'mod', title: m.title, author: m.author, iconUrl: null })
  }

  /** Changing the version, loader or pack keeps the picks: Cubby finds each one's version for the instance. */
  reset() {
    this.step = 'start'
    this.path = 'custom'
    this.minecraft = null
    this.loader = 'fabric'
    this.loaderVersion = null
    this.pack = null
    this.packVersions = null
    this.packVersionId = null
    this.packVersionsError = null
    this.picks = []
    this.turnOn = true
    this.name = ''
    this.nameTouched = false
    this.group = ''
    this.src = null
    this.iconTouched = false
  }

  /** Opens on what the rest of the app asked for (`app.newSource`). */
  openOn(source: 'quick' | 'custom' | 'import' | 'link' | ProviderId, providers: PackProvider[] | null) {
    if (source === 'custom') {
      this.path = 'custom'
      this.step = 'game'
    } else if (source === 'import' || source === 'link') {
      this.path = 'import'
      this.importFrom = source === 'link' ? 'link' : 'file'
      this.step = 'start'
    } else if (source !== 'quick' && providers?.some((p) => p.id === source)) {
      this.path = 'modpack'
      this.provider = source
      this.step = 'game'
    }
  }
}

export const wizard = new WizardState()
