import { api, onEvent } from './api'
import type { Account, AppInfo, ContentSource, ContentUpdate, Instance, LogLine, ProjectKind, PackProvider, ProjectSort, ProviderId, Settings, CubbyEvent } from './types'

export type InstanceTab = 'overview' | 'mods' | 'resourcepacks' | 'shaders' | 'worlds' | 'screenshots' | 'servers' | 'settings'
export type SettingsSection = 'general' | 'appearance' | 'language' | 'java' | 'game' | 'downloads' | 'storage' | 'accounts' | 'keyboard' | 'privacy' | 'about'

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
  | { name: 'project'; idOrSlug: string; kind: ProjectKind }
  | { name: 'downloads' }
  | { name: 'accounts' }
  | { name: 'servers' }
  | { name: 'captures' }
  | { name: 'settings'; section: SettingsSection }

/** Section of the hotbar a route belongs to. */
export function sectionOf(route: Route): 'library' | 'discover' | 'servers' | 'captures' | 'downloads' | 'accounts' | 'settings' {
  switch (route.name) {
    case 'discover':
    case 'project':
      return 'discover'
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

class AppState {
  route = $state<Route>({ name: 'library' })
  /** Previous routes, for Back (Alt Left). */
  private history: Route[] = []
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
  prismImportOpen = $state(false)
  /** New instance: the source shown (Custom, Import or a modpack provider). */
  newSource = $state<'custom' | 'import' | ProviderId>('custom')
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
  })
  targetId = $state<string | null>(stored('targetId', null))
  /** instance id → files identified on Modrinth */
  sources = $state<Record<string, ContentSource[]>>({})
  /** instance id → available updates */
  updates = $state<Record<string, ContentUpdate[]>>({})

  private toastId = 0

  get defaultAccount(): Account | undefined {
    return this.accounts.find((a) => a.isDefault)
  }

  get groups(): { name: string; count: number }[] {
    const counts = new Map<string, number>()
    for (const i of this.instances) if (i.group) counts.set(i.group, (counts.get(i.group) ?? 0) + 1)
    return [...counts.entries()].map(([name, count]) => ({ name, count }))
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
    this.targetId = id
    store('targetId', id)
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
    return unlisten
  }

  navigate(route: Route) {
    if (JSON.stringify(route) === JSON.stringify(this.route)) return
    this.history.push(this.route)
    if (this.history.length > 50) this.history.shift()
    this.route = route
    if ('id' in route && route.name !== 'crash') this.selected = route.id
    document.querySelector('main')?.scrollTo?.(0, 0)
  }

  back() {
    const prev = this.history.pop()
    if (prev) this.route = prev
    else if (this.route.name !== 'library') this.route = { name: 'library' }
  }

  openInstance(id: string, tab: InstanceTab = 'overview') {
    this.navigate({ name: 'instance', id, tab })
  }

  openProject(idOrSlug: string, kind: ProjectKind) {
    this.navigate({ name: 'project', idOrSlug, kind })
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

  async saveSettings(patch: Partial<Settings>) {
    if (!this.settings) return
    const next = { ...$state.snapshot(this.settings), ...patch } as Settings
    this.settings = next
    this.applyAppearance()
    try {
      this.settings = await api.saveSettings(next)
    } catch (e) {
      this.error(e)
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

  /** `server`: join this address right after the game starts. */
  async launch(id: string, offline = false, server: string | null = null) {
    if (this.running[id] || this.tasks[`launch:${id}`]) return
    // Without a signed-in account that owns the game, only the demo starts.
    if (!this.defaultAccount) this.toast('No Microsoft account, so this is the Minecraft demo. Sign in to play the full game.', 'info')
    if (this.crash?.instanceId === id) this.crash = null
    this.tasks[`launch:${id}`] = { label: 'Getting ready', done: 0, total: 0, started: Date.now() }
    try {
      await api.launchInstance(id, offline, server)
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
          delete this.sources[id]
          delete this.updates[id]
          void this.loadSources(id, true)
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

export const app = new AppState()
