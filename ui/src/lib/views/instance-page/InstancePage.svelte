<script lang="ts" module>
  import { api } from '../../api'
  import { date, timeOfDay } from '../../format'
  import { t } from '../../i18n.svelte'
  import { app } from '../../state.svelte'
  import type { ContentSource, ContentUpdate, Instance, ModFile, Pack, ProjectKind, Screenshot, Server, World, UpdateBackup, WorldBackup } from '../../types'

  // Helpers shared by the tabs of this folder (plain hoisted functions, safe to import from the tabs).

  /** A mod or pack file name without the ".disabled" suffix the backend adds to turned-off files. */
  export function baseFile(fileName: string): string {
    return fileName.replace(/\.disabled$/i, '')
  }

  /** The mod's own name, else its file name without ".jar". */
  export function modName(m: ModFile): string {
    return m.name?.trim() || baseFile(m.fileName).replace(/\.jar$/i, '')
  }

  /** "Today at 18:02", "Yesterday at 18:02", "Oct 30, 2026 at 21:15" (`inline`: "today at 18:02" inside a sentence). */
  export function when(iso: string, inline = false): string {
    const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
    const days = Math.round((start(new Date()) - start(new Date(iso))) / 86_400_000)
    const text = t('{day} at {time}', { day: days === 0 ? t('Today') : days === 1 ? t('Yesterday') : date(iso), time: timeOfDay(iso) })
    return inline && days <= 1 ? text[0].toLowerCase() + text.slice(1) : text
  }

  /** "6 GB", "4.5 GB" from megabytes. */
  export function gigabytes(mb: number): string {
    return `${(mb / 1024).toLocaleString('en-US', { maximumFractionDigits: 1 })} GB`
  }

  const SHADER_MODS = {
    iris: { names: ['iris', 'iris shaders', 'oculus'], file: /^(iris|oculus)[-_+]/i },
    sodium: { names: ['sodium', 'embeddium', 'rubidium'], file: /^(sodium|embeddium|rubidium)[-_+](?!extra)/i },
    optifine: { names: ['optifine'], file: /^optifine/i },
  }

  function isMod(m: ModFile, kind: keyof typeof SHADER_MODS): boolean {
    const name = m.name?.trim().toLowerCase()
    return name ? SHADER_MODS[kind].names.includes(name) : SHADER_MODS[kind].file.test(baseFile(m.fileName))
  }

  /** What the instance page lists, loaded once and shared by the header (tab counts) and every tab. */
  export class InstanceData {
    readonly id: string
    mods = $state<ModFile[] | null>(null)
    resourcePacks = $state<Pack[] | null>(null)
    shaderPacks = $state<Pack[] | null>(null)
    worlds = $state<World[] | null>(null)
    screenshots = $state<Screenshot[] | null>(null)
    servers = $state<Server[] | null>(null)
    backups = $state<WorldBackup[] | null>(null)
    /** Content updates that can be undone, newest first. */
    updateBackups = $state<UpdateBackup[] | null>(null)
    checkingUpdates = $state(false)
    duplicating = $state(false)

    /** Files identified on Modrinth, by file name without ".disabled". */
    sourceByFile = $derived.by(() => new Map((app.sources[this.id] ?? []).map((s) => [baseFile(s.fileName), s] as const)))
    /** Available updates, by file name without ".disabled". */
    updateByFile = $derived.by(() => new Map((app.updates[this.id] ?? []).map((u) => [baseFile(u.fileName), u] as const)))
    /** Mods turned on more than once (two files with the same mod name): file name → the other files. */
    twins = $derived.by(() => {
      const byName = new Map<string, ModFile[]>()
      for (const m of this.mods ?? []) {
        const key = m.enabled ? (m.name ?? '').trim().toLowerCase() : ''
        if (key) byName.set(key, [...(byName.get(key) ?? []), m])
      }
      const out = new Map<string, ModFile[]>()
      for (const list of byName.values()) if (list.length > 1) for (const m of list) out.set(m.fileName, list.filter((x) => x !== m))
      return out
    })
    /** What shader packs need: Iris (or Oculus) with Sodium (or a port), or OptiFine. Turned-on files first. */
    shaderMods = $derived.by(() => {
      const list = [...(this.mods ?? [])].sort((a, b) => Number(b.enabled) - Number(a.enabled))
      return {
        iris: list.find((m) => isMod(m, 'iris')),
        sodium: list.find((m) => isMod(m, 'sodium')),
        optifine: list.find((m) => isMod(m, 'optifine')),
      }
    })
    /** The most recently played world. */
    lastWorld = $derived.by(() => {
      let best: World | null = null
      for (const w of this.worlds ?? []) if (w.lastPlayed && (!best || w.lastPlayed > (best.lastPlayed ?? ''))) best = w
      return best
    })

    constructor(id: string) {
      this.id = id
    }

    get updates(): ContentUpdate[] | undefined {
      return app.updates[this.id]
    }

    source(fileName: string): ContentSource | undefined {
      return this.sourceByFile.get(baseFile(fileName))
    }

    update(fileName: string): ContentUpdate | undefined {
      return this.updateByFile.get(baseFile(fileName))
    }

    loadAll() {
      void this.loadMods()
      void this.loadPacks()
      void this.loadWorlds()
      void this.loadScreenshots()
      void this.loadServers()
      void app.loadSources(this.id)
    }

    async loadMods() {
      try {
        this.mods = await api.listMods(this.id)
      } catch (e) {
        this.mods ??= []
        app.error(e)
      }
    }

    async loadPacks() {
      try {
        const [resource, shader] = await Promise.all([api.listPacks(this.id, 'resource'), api.listPacks(this.id, 'shader')])
        this.resourcePacks = resource
        this.shaderPacks = shader
      } catch (e) {
        this.resourcePacks ??= []
        this.shaderPacks ??= []
        app.error(e)
      }
    }

    async loadWorlds() {
      try {
        this.worlds = await api.listWorlds(this.id)
      } catch (e) {
        this.worlds ??= []
        app.error(e)
      }
    }

    async loadScreenshots() {
      try {
        this.screenshots = (await api.listScreenshots(this.id)).sort((a, b) => b.taken.localeCompare(a.taken))
      } catch (e) {
        this.screenshots ??= []
        app.error(e)
      }
    }

    async loadServers() {
      try {
        this.servers = await api.listServers(this.id)
      } catch (e) {
        this.servers ??= []
        app.error(e)
      }
    }

    async loadBackups() {
      try {
        this.backups = (await api.listBackups(this.id)).sort((a, b) => b.created.localeCompare(a.created))
      } catch (e) {
        this.backups ??= []
        app.error(e)
      }
    }

    /** Asks Modrinth for updates (cached by the app until `force`). */
    async checkUpdates(force = false) {
      if (this.checkingUpdates) return
      this.checkingUpdates = true
      try {
        await app.loadUpdates(this.id, force)
      } finally {
        this.checkingUpdates = false
      }
    }

    /** Replaces these files by their latest version (task `update:<id>`); the app reloads sources when it ends. */
    async applyUpdates(fileNames: string[]) {
      if (!fileNames.length) return
      try {
        await api.applyUpdates(this.id, fileNames)
      } catch (e) {
        app.error(e)
      }
      void this.loadUpdateBackups()
    }

    async loadUpdateBackups() {
      try {
        this.updateBackups = await api.listUpdateBackups(this.id)
      } catch (e) {
        this.updateBackups ??= []
        app.error(e)
      }
    }

    openFolder(sub?: string) {
      api.openFolder(this.id, sub).catch((e) => app.error(e))
    }

    /** Opens Discover on a kind of content, installing into this instance. */
    discover(kind: ProjectKind) {
      app.setTarget(this.id)
      app.discover.kind = kind
      app.discover.query = ''
      app.discover.categories = []
      app.discover.page = 0
      app.navigate({ name: 'discover' })
    }

    /** Opens a Modrinth project page, installing into this instance. */
    project(slug: string, kind: ProjectKind) {
      app.setTarget(this.id)
      app.openProject(slug, kind)
    }

    async duplicate(instance: Instance) {
      if (this.duplicating) return
      this.duplicating = true
      try {
        const copy = await api.duplicateInstance(instance.id, `${instance.name} copy`)
        await app.refreshInstances()
        app.toast(t('Made a copy: {name}.', { name: copy.name }), 'success', { label: t('Open'), run: () => app.openInstance(copy.id) })
      } catch (e) {
        app.error(e)
      } finally {
        this.duplicating = false
      }
    }

    /** Re-downloads and checks the game files (task `repair:<id>`, shown in the Play button). */
    async repair() {
      if (app.tasks[`repair:${this.id}`]) return
      try {
        await api.repairInstance(this.id)
        app.toast(t('Game files checked and repaired.'), 'success')
      } catch (e) {
        app.error(e)
      }
    }
  }
</script>

<script lang="ts">
  import ExportDialog from './ExportDialog.svelte'
  import { tick, untrack } from 'svelte'
  import { Banner, Button, Icon, IconButton, InstanceIcon, MetaChip, PlayButton, Tabs, type IconName, type TabItem } from '../../cubby'
  import { floating } from '../../cubby/floating'
  import { loaderFull, num, plural, playTime, relativeTime, session } from '../../format'
  import { iconOf } from '../../icon'
  import type { InstanceTab } from '../../state.svelte'
  import OverviewTab from './OverviewTab.svelte'
  import ModsTab from './ModsTab.svelte'
  import ResourcePacksTab from './ResourcePacksTab.svelte'
  import ShadersTab from './ShadersTab.svelte'
  import WorldsTab from './WorldsTab.svelte'
  import ScreenshotsTab from './ScreenshotsTab.svelte'
  import BlockedFilesDialog from './BlockedFilesDialog.svelte'
  import ServersTab from './ServersTab.svelte'
  import SettingsTab from './SettingsTab.svelte'

  let { id, tab }: { id: string; tab: InstanceTab } = $props()

  // The page is recreated for each instance (App keys it by id).
  const data = new InstanceData(untrack(() => id))
  const instance = $derived(app.instance(id))
  const startedAt = $derived(app.running[id])
  const task = $derived(app.instanceTask(id))
  const busy = $derived(!!startedAt || !!task)

  data.loadAll()

  // Check Modrinth for updates when the player asked for it in Settings (again after an update ran).
  $effect(() => {
    if (app.settings?.checkUpdates && !app.updates[id] && !app.tasks[`update:${id}`]) untrack(() => void data.checkUpdates())
  })

  // The game rewrites worlds, screenshots, servers and options while it runs: reload when it stops.
  let wasRunning = untrack(() => !!app.running[id])
  $effect(() => {
    const isRunning = !!app.running[id]
    if (wasRunning && !isRunning)
      untrack(() => {
        void data.loadWorlds()
        void data.loadScreenshots()
        void data.loadServers()
        void data.loadPacks()
        if (data.backups) void data.loadBackups()
      })
    wasRunning = isRunning
  })

  // Installing or updating content changes the mods and packs folders.
  let contentBusy = false
  $effect(() => {
    const working = Object.keys(app.tasks).some((k) => /^(install|update|modpack):/.test(k) && k.split(':')[1] === id)
    if (contentBusy && !working)
      untrack(() => {
        void data.loadMods()
        void data.loadPacks()
      })
    contentBusy = working
  })

  let blockedOpen = $state(false)

  let now = $state(Date.now())
  $effect(() => {
    if (!startedAt) return
    now = Date.now()
    const timer = setInterval(() => (now = Date.now()), 15_000)
    return () => clearInterval(timer)
  })

  const playedLine = $derived.by(() => {
    if (!instance) return ''
    const parts: string[] = []
    if (instance.totalPlayedSecs >= 60) parts.push(`${playTime(instance.totalPlayedSecs)} played`)
    if (instance.lastPlayed) parts.push(`last played ${relativeTime(instance.lastPlayed).toLowerCase()}`)
    const line = parts.join(' · ')
    return line ? line[0].toUpperCase() + line.slice(1) : t('Never played')
  })

  const TASK_LABEL: Record<string, string> = { launch: t('Installing'), install: t('Installing'), modpack: t('Installing'), update: t('Updating'), repair: t('Repairing') }
  const taskId = $derived(Object.keys(app.tasks).find((k) => k.split(':')[1] === id))

  const play = $derived.by(() => {
    if (startedAt) return { state: 'running' as const, label: undefined, detail: t('Playing for {startedAt}', { startedAt: session(now - startedAt) }), progress: undefined }
    if (task) {
      const progress = task.total > 0 ? (task.done / task.total) * 100 : undefined
      const kind = taskId?.split(':')[0] ?? ''
      // an installed instance only starts: nothing to download
      const label = kind === 'launch' && instance?.installed ? 'Starting' : (TASK_LABEL[kind] ?? 'Working')
      return { state: 'installing' as const, label, detail: task.label, progress }
    }
    const detail = !instance?.installed ? t('Downloads the game first') : undefined
    return { state: 'ready' as const, label: undefined, detail, progress: undefined }
  })

  function onPlay() {
    if (startedAt) void app.kill(id)
    else if (!task) void app.launch(id)
  }

  const TABS: { id: InstanceTab; label: string }[] = [
    { id: 'overview', label: t('Overview') },
    { id: 'mods', label: t('Mods') },
    { id: 'resourcepacks', label: t('Resource packs') },
    { id: 'shaders', label: t('Shader packs') },
    { id: 'worlds', label: t('Worlds') },
    { id: 'screenshots', label: t('Screenshots') },
    { id: 'servers', label: t('Servers') },
    { id: 'settings', label: t('Settings') },
  ]
  const PANEL = 'instance-panel'

  const counts = $derived<Partial<Record<InstanceTab, number | undefined>>>({
    mods: data.mods?.length,
    resourcepacks: data.resourcePacks?.length,
    shaders: data.shaderPacks?.length,
    worlds: data.worlds?.length,
    screenshots: data.screenshots?.length,
    servers: data.servers?.length,
  })
  const current = $derived<InstanceTab>(TABS.some((t) => t.id === tab) ? tab : 'overview')
  const tabItems = $derived<TabItem[]>(
    TABS.map((t) => {
      const n = counts[t.id]
      return { id: t.id, label: t.label, count: n == null ? undefined : num(n), panel: t.id === current ? PANEL : undefined }
    }),
  )

  let exporting = $state(false)

  // More actions menu
  let menuOpen = $state(false)
  let moreEl: HTMLDivElement | undefined = $state()
  let menuEl: HTMLDivElement | undefined = $state()
  const menuId = $props.id()

  type MenuItem = { id: string; label: string; icon: IconName; danger?: boolean; disabled?: boolean; run: () => void }
  const menuItems = $derived<MenuItem[]>([
    { id: 'offline', label: t('Play offline'), icon: 'offline', disabled: busy, run: () => void app.launch(id, true) },
    { id: 'log', label: t('Game log'), icon: 'terminal', run: () => app.navigate({ name: 'running', id }) },
    { id: 'folder', label: t('Open folder'), icon: 'folder', run: () => data.openFolder() },
    { id: 'export', label: t('Export as a modpack file'), icon: 'download', run: () => (exporting = true) },
    { id: 'duplicate', label: t('Duplicate'), icon: 'copy', disabled: data.duplicating, run: () => instance && void data.duplicate(instance) },
    { id: 'repair', label: t('Repair'), icon: 'wrench', disabled: busy, run: () => void data.repair() },
    { id: 'delete', label: t('Delete'), icon: 'trash', danger: true, disabled: !!startedAt, run: () => (app.deletingId = id) },
  ])

  function trigger(): HTMLButtonElement | null {
    return moreEl?.querySelector<HTMLButtonElement>(':scope > button') ?? null
  }

  function items(): HTMLButtonElement[] {
    return Array.from(menuEl?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? [])
  }

  async function openMenu() {
    menuOpen = true
    await tick()
    items()[0]?.focus()
  }

  function closeMenu(refocus = true) {
    menuOpen = false
    if (refocus) trigger()?.focus()
  }

  function pick(item: MenuItem) {
    // focus goes back to the trigger first, so a dialog opened by the item returns it there
    closeMenu()
    item.run()
  }

  function menuKeys(e: KeyboardEvent) {
    const list = items()
    const at = list.indexOf(document.activeElement as HTMLButtonElement)
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      closeMenu()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const step = e.key === 'ArrowDown' ? 1 : -1
      list[(at + step + list.length) % list.length]?.focus()
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault()
      list[e.key === 'Home' ? 0 : list.length - 1]?.focus()
    } else if (e.key === 'Tab') {
      closeMenu(false)
    }
  }

  function outside(e: PointerEvent) {
    if (menuOpen && moreEl && !moreEl.contains(e.target as Node)) closeMenu(false)
  }
</script>

<svelte:window onpointerdown={outside} />

{#if instance}
  <div class="page">
    <div class="layout">
      <header class="head">
        <nav aria-label={t('Breadcrumb')} class="crumbs">
          <button type="button" class="link text-body-strong" onclick={() => app.navigate({ name: 'library' })}>{t('Library')}</button>
          <span class="crumbs__sep" aria-hidden="true"><Icon name="chevron-right" size={18} /></span>
          <span class="crumbs__here text-body" aria-current="page">{instance.name}</span>
        </nav>

        <div class="hero">
          <InstanceIcon {...iconOf(instance)} size={64} raised />
          <div class="hero__titles">
            <h1 class="hero__name text-title" title={instance.name}>{instance.name}</h1>
            <div class="hero__meta">
              <MetaChip>{instance.minecraft}</MetaChip>
              <MetaChip>{loaderFull(instance.loader)}</MetaChip>
              {#if instance.modpack}<MetaChip icon="download" title={t('Installed from this Modrinth modpack')}>{instance.modpack.title} {instance.modpack.versionNumber}</MetaChip>{/if}
              <span class="muted text-small">{playedLine}</span>
            </div>
          </div>
          <PlayButton size="md" state={play.state} label={play.label} detail={play.detail} progress={play.progress} onclick={onPlay} />
          <div class="more" bind:this={moreEl}>
            <IconButton
              icon="more"
              variant="secondary"
              label={t('More actions')}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              aria-controls={menuOpen ? menuId : undefined}
              onclick={() => (menuOpen ? closeMenu() : openMenu())}
            />
            {#if menuOpen}
              <div class="menu" role="menu" id={menuId} aria-label={t('More actions')} tabindex="-1" bind:this={menuEl} onkeydown={menuKeys} {@attach floating}>
                {#each menuItems as item (item.id)}
                  {#if item.danger}<div class="menu__sep" role="separator"></div>{/if}
                  <button type="button" role="menuitem" tabindex="-1" class={['menu__item text-body-strong', item.danger && 'is-danger']} disabled={item.disabled} onclick={() => pick(item)}>
                    <Icon name={item.icon} size={18} />
                    <span>{item.label}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        </div>

        {#if instance.blockedFiles > 0}
          {#snippet showFiles()}<Button size="sm" icon="download" onclick={() => (blockedOpen = true)}>{t('Show the files')}</Button>{/snippet}
          <Banner tone="warning" title={t('{n} to be downloaded by hand', { n: plural(instance.blockedFiles, 'file needs', 'files need') })} actions={showFiles}>
            {instance.blockedFiles === 1 ? t('The pack\'s authors don\'t let launchers download it. You can still play meanwhile.') : t('The pack\'s authors don\'t let launchers download them. You can still play meanwhile.')}
          </Banner>
        {/if}

        {#if startedAt}
          {#snippet showLog()}<Button size="sm" icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>{t('Show log')}</Button>{/snippet}
          <Banner tone="success" icon="play" title={t('Playing now')} actions={showLog}>
            {t('Changes to mods, packs and settings apply the next time you start the game.')}
          </Banner>
        {/if}
      </header>

      <div class="tabs-row">
        <Tabs label={t('Instance sections')} items={tabItems} value={current} onChange={(t) => app.openInstance(id, t as InstanceTab)} />
      </div>

      {#if current === 'mods'}
        <ModsTab {instance} {data} />
      {:else if current === 'resourcepacks'}
        <ResourcePacksTab {instance} {data} />
      {:else if current === 'shaders'}
        <ShadersTab {instance} {data} />
      {:else if current === 'worlds'}
        <WorldsTab {instance} {data} />
      {:else if current === 'screenshots'}
        <ScreenshotsTab {instance} {data} />
      {:else if current === 'servers'}
        <ServersTab {instance} {data} />
      {:else if current === 'settings'}
        <SettingsTab {instance} {data} />
      {:else}
        <OverviewTab {instance} {data} />
      {/if}
    </div>
  </div>
{:else}
  <div class="page missing">
    <Icon name="cube" size={36} />
    <h1 class="text-title">{t('This instance is gone')}</h1>
    <p class="muted text-body">{t('It may have been deleted or moved to the trash.')}</p>
    <Button variant="primary" icon="library" onclick={() => app.navigate({ name: 'library' })}>{t('Back to library')}</Button>
  </div>
{/if}

{#if exporting && instance}
  <ExportDialog {instance} onClose={() => (exporting = false)} />
{/if}

{#if blockedOpen && instance}
  <BlockedFilesDialog {instance} onClose={() => (blockedOpen = false)} />
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .layout {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: auto auto minmax(0, 1fr);
    grid-template-areas:
      'head aside'
      'tabs aside'
      'body aside';
  }
  .head {
    grid-area: head;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
    padding: var(--space-6) var(--space-6) 0;
  }
  .tabs-row {
    grid-area: tabs;
    min-width: 0;
    padding: var(--space-4) var(--space-6) 0;
  }

  /* Each tab renders a `.ip-body` (its content) and, when it has one, an `aside.ip-aside` (its inspector). */
  .layout > :global(.ip-body) {
    grid-area: body;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-6) var(--space-6);
  }
  .layout > :global(.ip-aside) {
    grid-area: aside;
    width: var(--size-inspector);
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .layout > :global(.ip-aside) > :global(.ip-aside__inner) {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    height: 100%;
    overflow-y: auto;
    padding: var(--space-6) var(--space-5);
  }
  @media (max-width: 1099px) {
    .page {
      height: auto;
      min-height: 100%;
    }
    .layout {
      overflow: visible;
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto auto auto auto;
      grid-template-areas: 'head' 'tabs' 'body' 'aside';
    }
    .layout > :global(.ip-aside) {
      width: auto;
      border-inline-start: 0;
      border-top: var(--border-chrome) solid var(--edge);
    }
    .layout > :global(.ip-body),
    .layout > :global(.ip-aside) {
      overflow: visible;
    }
    .layout > :global(.ip-aside) > :global(.ip-aside__inner) {
      height: auto;
      overflow: visible;
    }
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .crumbs__sep {
    display: inline-flex;
    color: var(--ink-muted);
  }
  .crumbs__here {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--ink-muted);
  }
  .link {
    padding: 0;
    color: var(--grass-text);
    background: none;
    border: 0;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }

  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
    padding-top: var(--space-1);
  }
  .hero__titles {
    flex: 1 1 280px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .hero__name {
    margin: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .hero__meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .muted {
    color: var(--ink-muted);
  }

  .more {
    position: relative;
  }
  .menu {
    position: absolute;
    top: calc(100% + 6px);
    inset-inline-end: 0;
    z-index: var(--layer-menu);
    display: flex;
    flex-direction: column;
    min-width: 220px;
    padding: var(--space-1) 0;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-float);
  }
  .menu__item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--size-control);
    padding: 6px var(--space-3);
    color: var(--ink);
    text-align: start;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .menu__item:hover:not(:disabled) {
    background: var(--surface-hover);
  }
  .menu__item:focus-visible {
    outline: none;
    background: var(--surface-hover);
    box-shadow: inset 0 0 0 3px var(--focus);
  }
  .menu__item:disabled {
    color: var(--ink-disabled);
    cursor: not-allowed;
  }
  .menu__item.is-danger:not(:disabled) {
    color: var(--redstone-text);
  }
  .menu__sep {
    height: var(--border-hair);
    margin: var(--space-1) 0;
    background: var(--line);
  }

  .missing {
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding: var(--space-12) var(--space-6);
    text-align: center;
  }
  .missing h1,
  .missing p {
    margin: 0;
  }
</style>
