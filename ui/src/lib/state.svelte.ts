import { untrack } from 'svelte'
import { api, inTauri, onEvent } from './api'
import type { Account, AppInfo, AppUpdate, CfInstalled, ContentSource, ContentUpdate, Instance, LogLine, ProjectKind, PackProvider, PackSummary, ProjectSort, ProviderId, Settings, CubbyEvent } from './types'
import { t } from './i18n.svelte'
import { plural } from './format'

export type InstanceTab = 'overview' | 'mods' | 'resourcepacks' | 'shaders' | 'worlds' | 'screenshots' | 'servers' | 'settings'
export type SettingsSection = 'general' | 'appearance' | 'language' | 'java' | 'game' | 'downloads' | 'storage' | 'accounts' | 'keyboard' | 'privacy' | 'updates' | 'about'

/** A game that stopped with an error, shown by the crash screen. */
export interface Crash {
  instanceId: string
  exitCode: number
  /** Length of the session, in seconds. */
  durationSecs: number
  at: string
}

export type Route =
  | { name: 'library' }
  | { name: 'instance'; id: string; tab: InstanceTab }
  /** The game of this instance is running: live log and session facts. */
  | { name: 'running'; id: string }
  | { name: 'crash'; id: string }
  | { name: 'create' }
  | { name: 'discover' }
  /** A Modrinth project page (mod, modpack, shader pack or resource pack). */
  | { name: 'project'; idOrSlug: string; kind: ProjectKind; source?: 'curseforge' }
  /** A modpack page of a provider other than Modrinth (CurseForge, FTB, ATLauncher, Technic). */
  | { name: 'pack'; provider: ProviderId; id: string }
  | { name: 'downloads' }
  | { name: 'accounts' }
  /** Skin and cape of an account. */
  | { name: 'skins'; accountId: string }
  | { name: 'servers' }
  | { name: 'captures' }
  | { name: 'settings'; section: SettingsSection }

/** Section of the hotbar a route belongs to. */
export function sectionOf(route: Route): 'library' | 'discover' | 'servers' | 'captures' | 'downloads' | 'accounts' | 'settings' {
  switch (route.name) {
    case 'discover':
    case 'project':
    case 'pack':
      return 'discover'
    case 'skins':
      return 'accounts'
    case 'servers':
    case 'captures':
    case 'downloads':
    case 'accounts':
    case 'settings':
      return route.name
    default:
      return 'library'
  }
}

const TAB_LABELS: Record<InstanceTab, () => string> = {
  overview: () => t('Overview'),
  mods: () => t('Mods'),
  resourcepacks: () => t('Resource packs'),
  shaders: () => t('Shader packs'),
  worlds: () => t('Worlds'),
  screenshots: () => t('Screenshots'),
  servers: () => t('Servers'),
  settings: () => t('Settings'),
}

/** The same page, whatever the order of its fields. */
export function routeKey(route: Route): string {
  return JSON.stringify(route, Object.keys(route).sort())
}

/** Another tab of the same instance, another settings section: the same place, not a new page. */
function samePlace(a: Route, b: Route): boolean {
  if (a.name === 'instance' && b.name === 'instance') return a.id === b.id
  return a.name === 'settings' && b.name === 'settings'
}

/** Elements whose scroll position a page keeps: the main area, and the panes marked `data-keep-scroll`. */
function scrollers(): HTMLElement[] {
  if (typeof document === 'undefined') return []
  const main = document.querySelector<HTMLElement>('main')
  return [...(main ? [main] : []), ...document.querySelectorAll<HTMLElement>('[data-keep-scroll]')]
}

function scrollPositions(): number[] {
  return scrollers().map((e) => e.scrollTop)
}

/** True once every position could be applied (the content is tall enough). */
function applyScroll(saved: number[]): boolean {
  const els = scrollers()
  let done = els.length >= saved.length
  saved.forEach((y, i) => {
    const el = els[i]
    if (!el) return
    el.scrollTop = y
    if (Math.abs(el.scrollTop - y) > 2) done = false
  })
  return done
}

function resetScroll() {
  if (typeof requestAnimationFrame === 'undefined') return
  requestAnimationFrame(() => scrollers().forEach((e) => (e.scrollTop = 0)))
}

export interface Task {
  label: string
  done: number
  total: number
  started: number
}

export interface FinishedTask {
  id: string
  label: string
  error: string | null
  at: number
}

export interface Toast {
  id: number
  text: string
  kind: 'info' | 'success' | 'error'
  action?: { label: string; run: () => void }
}

/** Filters of the Discover screen, kept while the player browses project pages. */
export interface DiscoverState {
  kind: ProjectKind
  query: string
  categories: string[]
  environment: 'client' | 'server' | null
  sort: ProjectSort
  /** Only show what works with the target instance. */
  compatibleOnly: boolean
  hideInstalled: boolean
  page: number
  /** Modpacks tab: where the modpacks come from (Modrinth is Discover itself). */
  provider: ProviderId
}

const MAX_LOG_LINES = 5000

function stored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`cubby.${key}`)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}

function store(key: string, value: unknown) {
  try {
    localStorage.setItem(`cubby.${key}`, JSON.stringify(value))
  } catch {
    /* private mode: keep it in memory only */
  }
}

/** A short label for a task id ("install:friends-smp" → "Installing"). */
export function taskKind(id: string): string {
  return id.split(':')[0]
}

/** Where an update of Cubby stands. */
export interface UpdateState {
  phase: 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'failed'
  info: AppUpdate | null
  /** What went wrong: a failed download, or a manual check that couldn't reach GitHub. */
  error: string | null
  /** When the last check finished (ms). */
  checkedAt: number | null
  /** The banner was closed for this session; the title bar chip stays. */
  dismissed: boolean
}

/** Which update dialog is open. "updated" shows once after Cubby came back on a new version. */
export type UpdateDialog = 'notes' | 'restart' | 'playing' | 'updated'

const CHECK_EVERY_MS = 6 * 60 * 60 * 1000
const FIRST_CHECK_MS = 8000
/** Content updates are looked for a little after start, once the first screen is up. */
const CONTENT_CHECK_MS = 4000
const SEEN_VERSION_KEY = 'cubby.seenVersion'

class AppState {
  route = $state<Route>({ name: 'library' })
  /** Pages behind and ahead of this one, for Back and Forward. */
  past = $state<Route[]>([])
  future = $state<Route[]>([])
  /** A little view state per page (scroll position, tab, version picked), so a page comes back as it was left. */
  private memory = new Map<string, Record<string, unknown>>()
  /** Modpack summaries seen in lists, so a pack page opens at once. */
  packSummaries = new Map<string, PackSummary>()
  info = $state<AppInfo | null>(null)
  instances = $state<Instance[]>([])
  accounts = $state<Account[]>([])
  settings = $state<Settings | null>(null)
  /** instance id → start time (ms) */
  running = $state<Record<string, number>>({})
  /** task id → progress */
  tasks = $state<Record<string, Task>>({})
  /** Tasks finished during this session, newest first. */
  finished = $state<FinishedTask[]>([])
  logs = $state<Record<string, LogLine[]>>({})
  selected = $state<string | null>(null)
  /** Library group tab ("all", "" = no group, else the group name). */
  groupFilter = $state<string>('all')
  libraryView = $state<'grid' | 'list'>(stored('libraryView', 'grid'))
  paletteOpen = $state(false)
  update = $state<UpdateState>({ phase: 'idle', info: null, error: null, checkedAt: null, dismissed: false })
  updateDialog = $state<UpdateDialog | null>(null)
  /** Version Cubby was on before an update, for the "up to date" dialog. */
  updatedFrom = $state<string | null>(null)
  prismImportOpen = $state(false)
  /** New instance: the source shown (Custom, Import or a modpack provider). */
  /** A cubby://pack/… link opened from outside (a web page): New instance › From a link reads it. */
  pendingPackLink = $state<string | null>(null)
  newSource = $state<'quick' | 'custom' | 'import' | 'link' | ProviderId>('quick')
  providers = $state<PackProvider[] | null>(null)
  providersError = $state<string | null>(null)
  /** Instance whose delete confirmation is open. */
  deletingId = $state<string | null>(null)
  /** The last game that stopped with an error. */
  crash = $state<Crash | null>(null)
  /** Games stopped with the Stop button: their exit code is not a crash. */
  private killed = new Set<string>()
  toasts = $state<Toast[]>([])
  loaded = $state(false)

  /** Discover: filters and the instance things get installed to. */
  discover = $state<DiscoverState>({
    kind: 'mod',
    query: '',
    categories: [],
    environment: null,
    sort: 'relevance',
    compatibleOnly: true,
    hideInstalled: false,
    page: 0,
    provider: 'modrinth',
  })
  targetId = $state<string | null>(stored('targetId', null))
  /** instance id → files identified on Modrinth */
  sources = $state<Record<string, ContentSource[]>>({})
  /** instance id → available updates */
  updates = $state<Record<string, ContentUpdate[]>>({})
  /** instance id → files identified on CurseForge (loaded by Discover when CurseForge is the source) */
  cfInstalled = $state<Record<string, CfInstalled[]>>({})

  private toastId = 0

  get defaultAccount(): Account | undefined {
    return this.accounts.find((a) => a.isDefault)
  }

  get groups(): { name: string; count: number }[] {
    const counts = new Map<string, number>()
    for (const i of this.instances) if (i.group) counts.set(i.group, (counts.get(i.group) ?? 0) + 1)
    return [...counts.entries()].map(([name, count]) => ({ name, count }))
  }

  /** The maximum memory the game gets when this instance starts (MB): its own value, else the automatic or global one. */
  memoryOf(instance: Instance): number {
    const java = this.settings?.java
    if (instance.javaOverrides.maxMemoryMb != null) return instance.javaOverrides.maxMemoryMb
    if ((java?.autoMemory ?? true) && instance.recommendedMemoryMb) return instance.recommendedMemoryMb
    return java?.maxMemoryMb ?? 4096
  }

  get recent(): Instance[] {
    return [...this.instances].sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? '') || a.name.localeCompare(b.name))
  }

  instance(id: string | null | undefined): Instance | undefined {
    return id ? this.instances.find((i) => i.id === id) : undefined
  }

  /** The instance Discover installs to: the chosen one, else the selected one, else the last played. */
  get target(): Instance | undefined {
    return this.instance(this.targetId) ?? this.instance(this.selected) ?? this.recent[0]
  }

  setTarget(id: string) {
    if (id !== this.targetId) this.targetWorld = null
    this.targetId = id
    store('targetId', id)
  }

  /** The world of the target instance datapacks go into (its folder in saves/); null = the most recent one. */
  targetWorld = $state<string | null>(null)

  /** Discover › Datapacks, installing into this world of this instance. */
  openDatapacks(instanceId: string, world: string) {
    this.setTarget(instanceId)
    this.targetWorld = world
    this.discover.kind = 'datapack'
    this.discover.categories = []
    this.discover.page = 0
    this.navigate({ name: 'discover' })
  }

  setLibraryView(view: 'grid' | 'list') {
    this.libraryView = view
    store('libraryView', view)
  }

  /** Night theme only; old appearance settings are ignored. The OS reduce-motion setting is handled by cubby.css. */
  applyAppearance() {
    const root = document.documentElement
    root.dataset.theme = 'dark'
    delete root.dataset.font
    delete root.dataset.motion
  }

  async init() {
    const unlisten = await onEvent((e) => this.handleEvent(e))
    try {
      const [info, instances, accounts, settings, running] = await Promise.all([
        api.appInfo(),
        api.listInstances(),
        api.listAccounts(),
        api.getSettings(),
        api.runningGames(),
      ])
      this.info = info
      this.instances = instances
      this.accounts = accounts
      this.settings = settings
      for (const g of running) this.running[g.instanceId] = new Date(g.startedAt).getTime()
      this.selected = this.recent[0]?.id ?? null
    } catch (e) {
      this.error(e)
    }
    this.loaded = true
    this.listenForLinks()
    this.noticeNewVersion()
    this.startUpdateChecks()
    setTimeout(() => void this.checkContentUpdates(), CONTENT_CHECK_MS)
    return unlisten
  }

  /** At start (Settings › Check for updates): looks for newer mods, resource packs and shader packs in every
   *  instance, most recently played first, then says what it found. Nothing is installed: the player updates
   *  from the instance. */
  async checkContentUpdates() {
    if (!this.settings?.checkUpdates) return
    const found: { instance: Instance; count: number; mods: boolean }[] = []
    for (const instance of this.recent) {
      // An instance being installed or played is checked when it is opened.
      if (this.instanceTask(instance.id) || this.running[instance.id]) continue
      const list = await this.loadUpdates(instance.id, true)
      if (list.length) found.push({ instance, count: list.length, mods: list.every((u) => /\.jar(\.disabled)?$/i.test(u.fileName)) })
    }
    if (!found.length) return
    const total = found.reduce((n, f) => n + f.count, 0)
    const first = found[0]
    // Mods when they are all mods, else Overview, which lists every kind.
    const tab: InstanceTab = first.mods ? 'mods' : 'overview'
    this.toast(
      found.length === 1
        ? t('{n} can be updated in {name}.', { n: plural(total, 'file', 'files'), name: first.instance.name })
        : t('{n} can be updated in {count}.', { n: plural(total, 'file', 'files'), count: plural(found.length, 'instance', 'instances') }),
      'info',
      { label: found.length === 1 ? t('See them') : t('See {name}', { name: first.instance.name }), run: () => this.openInstance(first.instance.id, tab) },
    )
  }

  /** cubby://pack/<code> links (the "Open in Cubby" button of a shared pack's page) open New instance › From a link.
   *  cubby://play/<id> links (desktop shortcuts) start that instance. */
  private listenForLinks() {
    if (!inTauri) return
    const open = (urls: string[] | null | undefined, atStart = false) => {
      const play = urls?.find((u) => u.toLowerCase().startsWith('cubby://play/'))
      if (play) {
        // The link Cubby started with comes back on every page load (a language change reloads): play it once.
        if (atStart) {
          try {
            if (sessionStorage.getItem('cubby.startLink') === play) return
            sessionStorage.setItem('cubby.startLink', play)
          } catch {
            /* no storage: play it */
          }
        }
        const id = decodeURIComponent(play.slice('cubby://play/'.length).replace(/\/+$/, ''))
        if (this.instance(id)) {
          this.selected = id
          void this.launch(id)
        } else {
          this.toast(t('That shortcut’s instance isn’t in Cubby anymore.'), 'error')
        }
        return
      }
      const link = urls?.find((u) => u.toLowerCase().startsWith('cubby://pack/'))
      if (!link) return
      this.pendingPackLink = link
      this.newSource = 'link'
      this.navigate({ name: 'create' })
    }
    void import('@tauri-apps/plugin-deep-link').then(async (m) => {
      open(await m.getCurrent().catch(() => null), true)
      await m.onOpenUrl(open)
    }).catch((e) => console.warn('deep links unavailable', e))
  }

  /** Goes to a page. Another tab of the same instance, or another section of the settings, replaces the page in the
   *  history instead of adding one: Back leaves the instance, it doesn't walk through its tabs. */
  navigate(route: Route, opts: { replace?: boolean } = {}) {
    if (routeKey(route) === routeKey(this.route)) return
    const replace = opts.replace ?? samePlace(this.route, route)
    this.saveScroll()
    if (!replace) {
      this.past = [...this.past, this.route].slice(-50)
      this.future = []
    }
    this.show(route, false)
  }

  get canBack(): boolean {
    return this.past.length > 0 || this.route.name !== 'library'
  }

  get canForward(): boolean {
    return this.future.length > 0
  }

  /** The page Back goes to, for "Back to …" links. */
  get previous(): Route | null {
    return this.past[this.past.length - 1] ?? null
  }

  back() {
    const prev = this.past[this.past.length - 1]
    this.saveScroll()
    if (!prev) {
      if (this.route.name !== 'library') this.show({ name: 'library' }, false)
      return
    }
    this.past = this.past.slice(0, -1)
    this.future = [this.route, ...this.future].slice(0, 50)
    this.show(prev, true)
  }

  forward() {
    const next = this.future[0]
    if (!next) return
    this.saveScroll()
    this.future = this.future.slice(1)
    this.past = [...this.past, this.route].slice(-50)
    this.show(next, true)
  }

  private show(route: Route, restore: boolean) {
    this.route = route
    if ('id' in route && route.name !== 'crash' && route.name !== 'pack') this.selected = route.id
    if (restore) this.restoreScroll()
    else resetScroll()
  }

  /** A value a page keeps for when it comes back (Back, Forward): its tab, the version picked… */
  recall<T>(key: string): T | undefined {
    return this.memory.get(untrack(() => routeKey(this.route)))?.[key] as T | undefined
  }

  /** Not reactive on the route: an effect calling it doesn't run again when the page changes. */
  keep(key: string, value: unknown) {
    const k = untrack(() => routeKey(this.route))
    const m = this.memory.get(k) ?? {}
    m[key] = value
    this.memory.set(k, m)
    if (this.memory.size > 100) this.memory.delete(this.memory.keys().next().value!)
  }

  private saveScroll() {
    this.keep('$scroll', scrollPositions())
  }

  /** Puts the scroll positions back once the page has drawn what it loads (lists come a moment later). */
  private restoreScroll() {
    const saved = this.recall<number[]>('$scroll')
    if (!saved?.some((y) => y > 0)) return resetScroll()
    const started = Date.now()
    const tick = () => {
      const done = applyScroll(saved)
      if (!done && Date.now() - started < 1500) requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
  }

  /** What a page is called in "Back to …" links: the instance and its tab, the project, the pack… */
  routeLabel(route: Route): string {
    const titled = this.memory.get(routeKey(route))?.$title
    switch (route.name) {
      case 'library':
        return t('Library')
      case 'instance': {
        const name = this.instance(route.id)?.name ?? route.id
        return route.tab === 'overview' ? name : `${name} › ${TAB_LABELS[route.tab]()}`
      }
      case 'running':
      case 'crash':
        return this.instance(route.id)?.name ?? route.id
      case 'discover':
        return t('Discover')
      case 'project':
        return typeof titled === 'string' ? titled : t('Project')
      case 'pack':
        return this.packSummaries.get(`${route.provider}:${route.id}`)?.name ?? t('Modpack')
      case 'create':
        return t('New instance')
      case 'downloads':
        return t('Downloads')
      case 'servers':
        return t('Servers')
      case 'captures':
        return t('Screenshots')
      case 'accounts':
      case 'skins':
        return t('Accounts')
      case 'settings':
        return t('Settings')
    }
  }

  /** A modpack page of a provider (Modrinth packs have project pages). */
  openPack(provider: ProviderId, summary: PackSummary) {
    this.packSummaries.set(`${provider}:${summary.id}`, summary)
    this.navigate({ name: 'pack', provider, id: summary.id })
  }

  openInstance(id: string, tab: InstanceTab = 'overview') {
    this.navigate({ name: 'instance', id, tab })
  }

  /** Names of projects opened by id or slug, so their page can show the name before the project loads. */
  #projectTitles = new Map<string, string>()

  /** A project page; `source: 'curseforge'` opens a CurseForge project by its id. `title`: its name, when the
   * caller knows it (the breadcrumb shows it at once instead of the id). */
  openProject(idOrSlug: string, kind: ProjectKind, source?: 'curseforge', title?: string) {
    if (title?.trim()) this.#projectTitles.set(idOrSlug, title.trim())
    this.navigate(source ? { name: 'project', idOrSlug, kind, source } : { name: 'project', idOrSlug, kind })
  }

  /** The name a project was opened with, if any. */
  projectTitle(idOrSlug: string): string | undefined {
    return this.#projectTitles.get(idOrSlug)
  }

  async refreshInstances() {
    try {
      this.instances = await api.listInstances()
    } catch (e) {
      this.error(e)
    }
  }

  async refreshInfo() {
    try {
      this.info = await api.appInfo()
    } catch (e) {
      this.error(e)
    }
  }

  async loadProviders() {
    this.providersError = null
    try {
      this.providers = await api.packProviders()
    } catch (e) {
      this.providersError = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
    }
  }

  async refreshAccounts() {
    try {
      this.accounts = await api.listAccounts()
    } catch (e) {
      this.error(e)
    }
  }

  /** Resolves true once written to disk. */
  async saveSettings(patch: Partial<Settings>): Promise<boolean> {
    if (!this.settings) return false
    const next = { ...$state.snapshot(this.settings), ...patch } as Settings
    this.settings = next
    this.applyAppearance()
    try {
      this.settings = await api.saveSettings(next)
      return true
    } catch (e) {
      this.error(e)
      return false
    }
  }

  async updateInstance(next: Instance) {
    try {
      const saved = await api.updateInstance(next)
      this.instances = this.instances.map((i) => (i.id === saved.id ? saved : i))
      return saved
    } catch (e) {
      this.error(e)
    }
  }

  /** True when a launch without an account starts the demo (release builds); development builds play offline. */
  get demoOnly(): boolean {
    return !this.defaultAccount && !this.info?.devBuild
  }

  /** `server`: join this address right after the game starts; `world`: open this world folder. */
  async launch(id: string, offline = false, server: string | null = null, world: string | null = null) {
    if (this.running[id] || this.tasks[`launch:${id}`]) return
    // Without a signed-in account that owns the game, only the demo starts (development builds play offline).
    if (this.demoOnly) this.toast(t('No Microsoft account, so this is the Minecraft demo. Sign in to play the full game.'), 'info')
    if (this.crash?.instanceId === id) this.crash = null
    this.tasks[`launch:${id}`] = { label: t('Getting ready'), done: 0, total: 0, started: Date.now() }
    try {
      await api.launchInstance(id, offline, server, world)
    } catch (e) {
      delete this.tasks[`launch:${id}`]
      this.error(e)
    }
  }

  async kill(id: string) {
    this.killed.add(id)
    try {
      await api.killInstance(id)
    } catch (e) {
      this.error(e)
    }
  }

  /** Files of the instance identified on Modrinth; cached until `force`. */
  async loadSources(id: string, force = false) {
    if (this.sources[id] && !force) return this.sources[id]
    try {
      this.sources[id] = await api.contentSources(id)
    } catch {
      this.sources[id] = []
    }
    return this.sources[id]
  }

  async loadUpdates(id: string, force = false) {
    if (this.updates[id] && !force) return this.updates[id]
    try {
      this.updates[id] = await api.checkUpdates(id)
    } catch {
      this.updates[id] = []
    }
    return this.updates[id]
  }

  /** A content file left the instance: drop it from the cached sources and updates, then read them again. */
  forgetFile(id: string, fileName: string) {
    const base = fileName.replace(/\.disabled$/, '')
    const other = (f: string) => f.replace(/\.disabled$/, '') !== base
    if (this.sources[id]) this.sources[id] = this.sources[id].filter((s) => other(s.fileName))
    if (this.updates[id]) this.updates[id] = this.updates[id].filter((u) => other(u.fileName))
    if (this.cfInstalled[id]) this.cfInstalled[id] = this.cfInstalled[id].filter((f) => other(f.fileName))
    void this.loadSources(id, true)
  }

  /** Files of the instance identified on CurseForge; cached until `force`. */
  async loadCfInstalled(id: string, force = false) {
    if (this.cfInstalled[id] && !force) return this.cfInstalled[id]
    try {
      this.cfInstalled[id] = await api.curseforgeInstalled(id)
    } catch {
      this.cfInstalled[id] = []
    }
    return this.cfInstalled[id]
  }

  /** The instance's file of a CurseForge project, when CurseForge identified it. */
  cfInstalledIn(id: string | undefined, projectId: string): CfInstalled | undefined {
    return id ? this.cfInstalled[id]?.find((f) => f.projectId === projectId) : undefined
  }

  installedIn(id: string | undefined, projectId: string): ContentSource | undefined {
    return id ? this.sources[id]?.find((s) => s.projectId === projectId) : undefined
  }

  /** Progress of the first running task whose id starts with `prefix` (e.g. "install:friends-smp"). */
  taskFor(prefix: string): Task | undefined {
    const key = Object.keys(this.tasks).find((k) => k === prefix || k.startsWith(`${prefix}:`))
    return key ? this.tasks[key] : undefined
  }

  /** Progress of whatever prepares an instance (launch, install, modpack, update, repair, backup, restore). */
  instanceTask(id: string): (Task & { kind: string }) | undefined {
    const key = Object.keys(this.tasks).find((k) => k.split(':')[1] === id)
    return key ? { ...this.tasks[key], kind: taskKind(key) } : undefined
  }

  /* Updates of Cubby */

  /** The first check a few seconds after start (so it never slows the launch), then every few hours. */
  private startUpdateChecks() {
    const run = () => {
      if (this.settings && this.settings.updates.mode !== 'off') void this.checkForUpdate()
    }
    setTimeout(run, FIRST_CHECK_MS)
    setInterval(run, CHECK_EVERY_MS)
  }

  /** After an update Cubby opens on a new version: tell the player once. */
  private noticeNewVersion() {
    const now = this.info?.version
    if (!now) return
    try {
      const seen = localStorage.getItem(SEEN_VERSION_KEY)
      if (seen && seen !== now) {
        this.updatedFrom = seen
        this.updateDialog = 'updated'
      }
      localStorage.setItem(SEEN_VERSION_KEY, now)
    } catch {
      /* private mode: no notice */
    }
  }

  /** Looks for a newer Cubby. A manual check (the Check button) also reports a failure. */
  async checkForUpdate(manual = false) {
    const phase = this.update.phase
    if (phase === 'checking' || phase === 'downloading') return
    this.update.phase = 'checking'
    this.update.error = null
    try {
      const info = await api.checkUpdate()
      this.update.checkedAt = Date.now()
      if (info && info.version !== this.update.info?.version) this.update.dismissed = false
      this.update.info = info
      if (!info || (!manual && this.settings?.updates.skipped === info.version)) {
        this.update.phase = 'idle'
        return
      }
      this.update.phase = info.downloaded ? 'ready' : 'available'
      // Automatic mode downloads right away and installs when Cubby closes.
      if (this.settings?.updates.mode === 'auto' && !info.downloaded) void this.downloadUpdate()
    } catch (e) {
      console.warn(e)
      this.update.phase = 'idle'
      this.update.checkedAt = Date.now()
      if (manual) this.update.error = typeof e === 'string' ? e : t('Couldn’t reach GitHub.')
    }
  }

  async downloadUpdate() {
    if (!this.update.info || this.update.phase === 'downloading') return
    this.update.phase = 'downloading'
    this.update.error = null
    this.update.dismissed = false
    try {
      await api.downloadUpdate()
      this.update.info = { ...this.update.info, downloaded: true }
      this.update.phase = 'ready'
    } catch (e) {
      this.update.error = typeof e === 'string' ? e : t('The download didn’t finish.')
      this.update.phase = 'failed'
    }
  }

  /** Installs and restarts; with a game running, explains that the update waits for the game to close. */
  async installUpdate() {
    if (Object.keys(this.running).length) {
      this.updateDialog = 'playing'
      return
    }
    try {
      await api.installUpdate()
    } catch (e) {
      this.updateDialog = null
      this.error(e)
    }
  }

  async skipUpdate() {
    const info = this.update.info
    if (!info || !this.settings) return
    this.updateDialog = null
    this.update.phase = 'idle'
    await this.saveSettings({ updates: { ...this.settings.updates, skipped: info.version } })
  }

  toast(text: string, kind: Toast['kind'] = 'info', action?: Toast['action']) {
    const toast = { id: ++this.toastId, text, kind, action }
    this.toasts = [...this.toasts, toast]
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== toast.id)), kind === 'error' ? 8000 : 5000)
  }

  dismissToast(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id)
  }

  error(e: unknown) {
    const text = typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
    console.error(e)
    this.toast(text, 'error')
  }

  private handleEvent(e: CubbyEvent) {
    switch (e.type) {
      case 'progress': {
        const prev = this.tasks[e.task]
        this.tasks[e.task] = { label: e.label, done: e.done, total: e.total, started: prev?.started ?? Date.now() }
        break
      }
      case 'taskFinished': {
        const label = this.tasks[e.task]?.label ?? e.task
        delete this.tasks[e.task]
        this.finished = [{ id: e.task, label, error: e.error, at: Date.now() }, ...this.finished].slice(0, 50)
        if (e.error) this.error(e.error)
        const [kind, id] = e.task.split(':')
        if (id && ['install', 'update', 'modpack'].includes(kind)) {
          // Read again without emptying first: what is shown stays until the new answer replaces it.
          void this.loadSources(id, true)
          if (this.updates[id]) void this.loadUpdates(id, true)
          if (this.cfInstalled[id]) void this.loadCfInstalled(id, true)
        }
        void this.refreshInstances().then(() => {
          // a failed first install is deleted by the backend: leave its page instead of showing a hole
          if (e.error && kind === 'modpack' && id && !this.instance(id) && this.route.name !== 'library' && 'id' in this.route && this.route.id === id) this.navigate({ name: 'library' })
        })
        break
      }
      case 'gameLog': {
        const list = this.logs[e.instanceId] ?? []
        list.push({ line: e.line, stream: e.stream, time: new Date().toISOString() })
        if (list.length > MAX_LOG_LINES) list.splice(0, list.length - MAX_LOG_LINES)
        this.logs[e.instanceId] = list
        break
      }
      case 'gameState':
        if (e.running) {
          this.running[e.instanceId] = Date.now()
          this.logs[e.instanceId] = []
        } else {
          const startedAt = this.running[e.instanceId]
          delete this.running[e.instanceId]
          const killed = this.killed.delete(e.instanceId)
          if (e.exitCode && e.exitCode !== 0 && !killed) {
            this.crash = {
              instanceId: e.instanceId,
              exitCode: e.exitCode,
              durationSecs: startedAt ? Math.round((Date.now() - startedAt) / 1000) : 0,
              at: new Date().toISOString(),
            }
            this.navigate({ name: 'crash', id: e.instanceId })
          } else if (this.route.name === 'running' && this.route.id === e.instanceId) {
            this.navigate({ name: 'instance', id: e.instanceId, tab: 'overview' })
          }
          void this.refreshInstances()
        }
        break
    }
  }
}

// One state for the whole app. In development, a hot reload of this module (or of one it imports, like a catalog
// of texts) would otherwise make a second one: the screens it reloads would read and navigate a state the rest of
// the window doesn't show (links doing nothing, the account missing). The first one is kept across reloads.
export const app: AppState = (import.meta.hot?.data.app as AppState | undefined) ?? new AppState()
if (import.meta.hot) import.meta.hot.data.app = app
