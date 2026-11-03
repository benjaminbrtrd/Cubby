<script lang="ts" module>
  import type { Status } from '../../cubby'
  import { app, taskKind, type Task } from '../../state.svelte'
  import { playTime, relativeTime } from '../../format'
  import type { Instance } from '../../types'

  /** What an instance is doing right now, read from the live state. Shared by the library, the inspector and the palette. */
  export interface Live {
    status: Status
    task: Task | undefined
    /** Kind of the task that prepares the instance: "launch", "install", "modpack", "update", "repair". */
    kind: string | null
    /** 0 to 100 when the task knows its size. */
    progress: number | undefined
    running: boolean
    crashed: boolean
    updates: number
  }

  export function taskProgress(task: Task | undefined): number | undefined {
    return task && task.total > 0 ? Math.min(100, (task.done / task.total) * 100) : undefined
  }

  /** The last crash of this instance counts until a later session was recorded. */
  function playedSince(instance: Instance, at: string): boolean {
    return !!instance.lastPlayed && Date.parse(instance.lastPlayed) > Date.parse(at) + 60_000
  }

  export function liveState(instance: Instance): Live {
    const id = instance.id
    const running = !!app.running[id]
    const task = app.instanceTask(id)
    const key = task ? Object.keys(app.tasks).find((k) => k.split(':')[1] === id) : undefined
    const crash = app.crash
    const crashed = !!crash && crash.instanceId === id && !running && !task && !playedSince(instance, crash.at)
    const updates = app.updates[id]?.length ?? 0
    const status: Status = running
      ? 'running'
      : task
        ? 'installing'
        : crashed
          ? 'crashed'
          : !instance.installed
            ? 'download'
            : updates
              ? 'update'
              : 'ready'
    return { status, task, kind: key ? taskKind(key) : null, progress: taskProgress(task), running, crashed, updates }
  }

  /** "Played 2h ago · 41h", "Played yesterday", "Never played" */
  export function playedLine(instance: Instance): string {
    if (!instance.lastPlayed) return 'Never played'
    const rel = relativeTime(instance.lastPlayed)
    const when = `Played ${rel.charAt(0).toLowerCase()}${rel.slice(1)}`
    if (app.settings?.showPlayTime === false || instance.totalPlayedSecs < 60) return when
    return `${when} · ${playTime(instance.totalPlayedSecs)}`
  }

  /** Snapshots, pre-releases and release candidates get the snapshot chip. */
  export function isSnapshot(version: string): boolean {
    return /^\d{2}w\d{2}[a-z~]$/i.test(version) || /-(pre|rc)[-\d]*$/i.test(version) || /snapshot/i.test(version)
  }

  /** Label of the play button while a task prepares the instance. */
  export function taskVerb(kind: string | null): string {
    switch (kind) {
      case 'launch':
        return 'Starting'
      case 'update':
        return 'Updating'
      case 'repair':
        return 'Repairing'
      default:
        return 'Installing'
    }
  }
</script>

<script lang="ts">
  import { Button, Icon, IconButton, InstanceRow, InstanceTile, Logo, SegmentedControl, Tabs, TextField, type TabItem } from '../../cubby'
  import { api } from '../../api'
  import { iconOf } from '../../icon'
  import { fold, loaderFull, loaderName, num, plural, session } from '../../format'
  import Inspector from './Inspector.svelte'
  import DeleteInstanceDialog from './DeleteInstanceDialog.svelte'

  type Sort = 'played' | 'name' | 'time' | 'created'
  const SORTS: { id: Sort; label: string }[] = [
    { id: 'played', label: 'Last played' },
    { id: 'name', label: 'Name' },
    { id: 'time', label: 'Play time' },
    { id: 'created', label: 'Recently added' },
  ]
  const VIEWS = [
    { value: 'grid', label: 'Grid', icon: 'library' as const },
    { value: 'list', label: 'List', icon: 'servers' as const },
  ]
  const byName = new Intl.Collator('en-US', { sensitivity: 'base', numeric: true })

  let sort = $state<Sort>('played')
  let filter = $state('')
  /** List view: instances picked with Ctrl click, Shift click or Space. */
  let marked = $state<string[]>([])
  let menu = $state<'sort' | 'group' | null>(null)
  let bulkDelete = $state<string[] | null>(null)
  let now = $state(Date.now())
  /** List view: number of mod files per instance. */
  let mods = $state<Record<string, number>>({})
  let body: HTMLElement | undefined = $state()
  let sortAnchor: HTMLElement | undefined = $state()
  let groupAnchor: HTMLElement | undefined = $state()

  const isList = $derived(app.libraryView === 'list')

  // groups

  const ungrouped = $derived(app.instances.filter((i) => !i.group).length)
  const groupNames = $derived(app.groups.map((g) => g.name).sort(byName.compare))
  const tabs = $derived<TabItem[]>([
    { id: 'all', label: 'All', count: app.instances.length },
    ...groupNames.map((name) => ({ id: name, label: name, count: app.groups.find((g) => g.name === name)?.count ?? 0 })),
    ...(ungrouped && app.groups.length ? [{ id: '', label: 'No group', count: ungrouped }] : []),
  ])

  $effect(() => {
    if (!tabs.some((t) => t.id === app.groupFilter)) app.groupFilter = 'all'
  })

  // what is shown, in which order

  function compare(a: Instance, b: Instance): number {
    switch (sort) {
      case 'name':
        return byName.compare(a.name, b.name)
      case 'time':
        return b.totalPlayedSecs - a.totalPlayedSecs || byName.compare(a.name, b.name)
      case 'created':
        return b.created.localeCompare(a.created) || byName.compare(a.name, b.name)
      default:
        return (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? '') || byName.compare(a.name, b.name)
    }
  }

  const query = $derived(fold(filter.trim()))
  const visible = $derived.by(() => {
    const inGroup = app.instances.filter((i) => app.groupFilter === 'all' || (i.group ?? '') === app.groupFilter)
    const shown =
      isList && query
        ? inGroup.filter((i) => [i.name, i.minecraft, loaderFull(i.loader), i.group ?? ''].some((t) => fold(t).includes(query)))
        : inGroup
    return [...shown].sort(compare)
  })

  const headed = $derived(app.groupFilter === 'all' && app.groups.length > 0)
  const sections = $derived.by(() => {
    if (!headed) return [{ key: '*', name: '', items: visible }]
    const map = new Map<string, Instance[]>()
    for (const i of visible) map.set(i.group ?? '', [...(map.get(i.group ?? '') ?? []), i])
    return [...map.entries()]
      .sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : byName.compare(a, b)))
      .map(([name, items]) => ({ key: name || '*none', name: name || 'No group', items }))
  })

  /** Visual order, for the arrow keys. */
  const order = $derived(isList ? sections.flatMap((s) => s.items) : visible)
  const current = $derived(order.find((i) => i.id === app.selected) ?? order[0])
  const markedVisible = $derived(marked.filter((id) => order.some((i) => i.id === id)))
  const totalSecs = $derived(order.reduce((s, i) => s + i.totalPlayedSecs, 0))

  // live data

  const anyRunning = $derived(Object.keys(app.running).length > 0)
  $effect(() => {
    if (!anyRunning) return
    now = Date.now()
    const timer = setInterval(() => (now = Date.now()), 15_000)
    return () => clearInterval(timer)
  })

  // The list shows mod counts: read each modded instance's mods folder, one at a time, again after installs and updates.
  const moddedIds = $derived(
    app.instances
      .filter((i) => i.loader.kind !== 'vanilla')
      .map((i) => i.id)
      .join('\n'),
  )
  const lastFinished = $derived(app.finished[0]?.at ?? 0)
  $effect(() => {
    if (!isList || !moddedIds) return
    void lastFinished
    const ids = moddedIds.split('\n')
    let stale = false
    void (async () => {
      for (const id of ids) {
        if (stale) return
        try {
          const list = await api.listMods(id)
          if (!stale) mods[id] = list.length
        } catch {
          /* the column stays blank */
        }
      }
    })()
    return () => {
      stale = true
    }
  })

  function detailOf(instance: Instance, live: Live): string | undefined {
    if (live.running) return session(now - app.running[instance.id])
    if (live.status === 'installing') return live.progress != null ? `${Math.round(live.progress)}%` : undefined
    if (live.status === 'update') return num(live.updates)
    return undefined
  }

  function tileMeta(instance: Instance, live: Live): string {
    if (live.status === 'installing' && live.task) return live.task.label
    if (live.status === 'download') return 'Play downloads it first'
    return playedLine(instance)
  }

  function rowSub(instance: Instance, live: Live): string | undefined {
    if (instance.modpack) return `Modpack ${instance.modpack.versionNumber}`
    if (live.status === 'download') return 'Play downloads it first'
    if (instance.totalPlayedSecs >= 60) return `${playTime(instance.totalPlayedSecs)} played`
    return undefined
  }

  // actions

  function playOrStop(id: string) {
    if (app.running[id]) void app.kill(id)
    else void app.launch(id)
  }

  /** Enter on a tile or a row. */
  function play(id: string, offline: boolean) {
    if (app.running[id]) app.navigate({ name: 'running', id })
    else if (!app.instanceTask(id)) void app.launch(id, offline)
  }

  function askDelete(id: string) {
    if (isList && markedVisible.length > 1) bulkDelete = [...markedVisible]
    else app.deletingId = markedVisible.length === 1 ? markedVisible[0] : id
  }

  function select(id: string) {
    app.selected = id
    marked = []
  }

  function toggleMark(id: string) {
    const start = marked.length ? marked : app.selected && app.selected !== id && order.some((i) => i.id === app.selected) ? [app.selected] : []
    marked = start.includes(id) ? start.filter((x) => x !== id) : [...start, id]
    app.selected = id
  }

  function markRange(id: string) {
    const from = order.findIndex((i) => i.id === (app.selected ?? id))
    const to = order.findIndex((i) => i.id === id)
    if (from < 0 || to < 0) return toggleMark(id)
    const [a, b] = from < to ? [from, to] : [to, from]
    marked = [...new Set([...marked, ...order.slice(a, b + 1).map((i) => i.id)])]
  }

  function setView(view: string) {
    app.setLibraryView(view === 'list' ? 'list' : 'grid')
    marked = []
    menu = null
  }

  async function moveTo(group: string | null) {
    closeMenu(groupAnchor)
    const ids = [...markedVisible]
    let moved = 0
    for (const id of ids) {
      const instance = app.instance(id)
      if (!instance) continue
      if ((instance.group ?? null) === group) {
        moved++
        continue
      }
      if (await app.updateInstance({ ...$state.snapshot(instance), group })) moved++
    }
    if (!moved) return
    const what = plural(moved, 'instance', 'instances')
    app.toast(group ? `Moved ${what} to ${group}.` : `Took ${what} out of their group.`, 'success')
  }

  function discoverModpacks() {
    app.discover.kind = 'modpack'
    app.navigate({ name: 'discover' })
  }

  // keyboard: one tab stop in the grid or list, then the arrows

  function mainOf(cell: Element | null | undefined) {
    return cell?.querySelector<HTMLElement>('.cb-tile__main, .cb-irow__main') ?? null
  }

  function cellFrom(e: Event): HTMLElement | null {
    const target = e.target as HTMLElement | null
    if (!target?.matches('.cb-tile__main, .cb-irow__main')) return null
    return target.closest<HTMLElement>('[data-cell]')
  }

  function columns(): number {
    const cells = body?.querySelectorAll<HTMLElement>('[data-cell]')
    if (!cells?.length) return 1
    const top = cells[0].offsetTop
    let n = 0
    for (const c of cells) {
      if (c.offsetTop !== top) break
      n++
    }
    return Math.max(1, n)
  }

  function focusCell(id: string) {
    mainOf(body?.querySelector(`[data-cell="${CSS.escape(id)}"]`))?.focus()
  }

  function onkeydown(e: KeyboardEvent) {
    const cell = cellFrom(e)
    if (!cell || e.altKey) return
    const id = cell.dataset.cell!
    const ctrl = e.ctrlKey || e.metaKey
    if (e.key === 'Enter') {
      e.preventDefault()
      play(id, ctrl)
      return
    }
    if (e.key === 'Delete') {
      e.preventDefault()
      askDelete(id)
      return
    }
    if (e.key === ' ' && isList) {
      e.preventDefault()
      toggleMark(id)
      return
    }
    if (ctrl) return
    const at = order.findIndex((i) => i.id === id)
    const rtl = getComputedStyle(cell).direction === 'rtl'
    const cols = isList ? 1 : columns()
    let to: number
    switch (e.key) {
      case 'ArrowRight':
        if (isList) return
        to = at + (rtl ? -1 : 1)
        break
      case 'ArrowLeft':
        if (isList) return
        to = at + (rtl ? 1 : -1)
        break
      case 'ArrowDown':
        to = at + cols
        break
      case 'ArrowUp':
        to = at - cols
        break
      case 'Home':
        to = 0
        break
      case 'End':
        to = order.length - 1
        break
      default:
        return
    }
    e.preventDefault()
    const next = order[to]
    if (!next) return
    if (e.shiftKey && isList) markRange(next.id)
    app.selected = next.id
    focusCell(next.id)
  }

  // Space on a row marks it: keep the button from also clicking on key up.
  function onkeyup(e: KeyboardEvent) {
    if (e.key === ' ' && isList && cellFrom(e)) e.preventDefault()
  }

  // Ctrl click and Shift click in the list mark rows instead of selecting one.
  function clickCapture(e: MouseEvent, id: string) {
    if (!isList || !(e.ctrlKey || e.metaKey || e.shiftKey)) return
    if (!(e.target as HTMLElement).closest('.cb-irow__main')) return
    e.preventDefault()
    e.stopPropagation()
    if (e.shiftKey) markRange(id)
    else toggleMark(id)
    app.selected = id
  }

  // A double click opens the instance (the component would play it).
  function dblclickCapture(e: MouseEvent, id: string) {
    if (!(e.target as HTMLElement).closest('.cb-tile__main, .cb-irow__main')) return
    e.stopPropagation()
    app.openInstance(id)
  }

  // Only the selected tile or row is in the tab order; its Play button is reached with Enter.
  $effect(() => {
    const root = body
    const focusId = current?.id
    void order
    void isList
    if (!root) return
    for (const cell of root.querySelectorAll<HTMLElement>('[data-cell]')) {
      const main = mainOf(cell)
      if (main) main.tabIndex = cell.dataset.cell === focusId ? 0 : -1
      for (const b of cell.querySelectorAll<HTMLElement>('.cb-tile__play button, .cb-irow__action button')) b.tabIndex = -1
    }
  })

  // small menus (sort, move to group)

  function toggleMenu(which: 'sort' | 'group') {
    menu = menu === which ? null : which
  }

  function closeMenu(anchor?: HTMLElement) {
    menu = null
    anchor?.querySelector<HTMLElement>('button')?.focus()
  }

  function menuKeys(e: KeyboardEvent, anchor: HTMLElement | undefined) {
    const items = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role^="menuitem"]')]
    const i = items.indexOf(document.activeElement as HTMLElement)
    let to: number | null = null
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      closeMenu(anchor)
    } else if (e.key === 'Tab') {
      menu = null
    } else if (e.key === 'ArrowDown') to = (i + 1) % items.length
    else if (e.key === 'ArrowUp') to = (i - 1 + items.length) % items.length
    else if (e.key === 'Home') to = 0
    else if (e.key === 'End') to = items.length - 1
    if (to != null) {
      e.preventDefault()
      items[to]?.focus()
    }
  }

  function leaveMenu(e: FocusEvent) {
    const next = e.relatedTarget as Node | null
    if (!next || !(e.currentTarget as HTMLElement).contains(next)) menu = null
  }

  function focusFirst(node: HTMLElement) {
    ;(node.querySelector<HTMLElement>('[aria-checked="true"]') ?? node.querySelector<HTMLElement>('[role^="menuitem"]'))?.focus()
  }
</script>

<div class="library">
  {#if app.instances.length === 0}
    <section class="empty" aria-labelledby="library-empty-title">
      <Logo size={64} wordmark={false} />
      <h1 id="library-empty-title" class="text-title">No instances yet</h1>
      <p class="text-body muted">An instance is a copy of the game with its own version, mods and worlds. Make one, bring yours from Prism Launcher, or start from a modpack.</p>
      <div class="empty__actions">
        <Button variant="primary" icon="plus" onclick={() => app.navigate({ name: 'create' })}>New instance</Button>
        <Button icon="import" onclick={() => (app.prismImportOpen = true)}>Import from Prism Launcher</Button>
        <Button variant="ghost" icon="discover" onclick={discoverModpacks}>Discover modpacks</Button>
      </div>
    </section>
  {:else}
    <div class="split">
      <div class="content">
        <div class="head">
          <h1 class="text-title">Library</h1>
          <span class="grow"></span>
          <SegmentedControl label="View" options={VIEWS} value={app.libraryView} onChange={setView} />
          <Button icon="import" onclick={() => (app.prismImportOpen = true)}>Import</Button>
          <Button variant="primary" icon="plus" onclick={() => app.navigate({ name: 'create' })}>New instance</Button>
        </div>

        {#if tabs.length > 1}
          <Tabs label="Groups" items={tabs} bind:value={app.groupFilter} />
        {/if}

        {#if isList}
          <div class="tools">
            <div class="filter">
              <TextField label="Filter instances" hideLabel icon="search" placeholder="Filter instances" bind:value={filter} />
            </div>
            <div class="anchor" bind:this={sortAnchor} onfocusout={leaveMenu}>
              <Button variant="ghost" iconEnd="chevron-down" aria-haspopup="menu" aria-expanded={menu === 'sort'} onclick={() => toggleMenu('sort')}>
                <span class="text-body">Sort: <span class="sort-value">{SORTS.find((s) => s.id === sort)?.label}</span></span>
              </Button>
              {#if menu === 'sort'}
                <div class="menu" role="menu" tabindex="-1" aria-label="Sort by" onkeydown={(e) => menuKeys(e, sortAnchor)} {@attach focusFirst}>
                  {#each SORTS as s (s.id)}
                    <button
                      type="button"
                      role="menuitemradio"
                      aria-checked={sort === s.id}
                      class="menu__item text-body"
                      onclick={() => {
                        sort = s.id
                        closeMenu(sortAnchor)
                      }}
                    >
                      <span class="menu__check">{#if sort === s.id}<Icon name="check" size={18} />{/if}</span>
                      {s.label}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
            <span class="grow"></span>
            {#if markedVisible.length}
              <div class="selection" role="region" aria-label="Selection">
                <Icon name="check" size={18} />
                <span class="text-body-strong">{num(markedVisible.length)} selected</span>
                <div class="anchor" bind:this={groupAnchor} onfocusout={leaveMenu}>
                  <Button size="sm" icon="folder" iconEnd="chevron-down" aria-haspopup="menu" aria-expanded={menu === 'group'} onclick={() => toggleMenu('group')}>
                    Move to group
                  </Button>
                  {#if menu === 'group'}
                    <div class="menu menu--end" role="menu" tabindex="-1" aria-label="Move to group" onkeydown={(e) => menuKeys(e, groupAnchor)} {@attach focusFirst}>
                      {#each groupNames as name (name)}
                        <button type="button" role="menuitem" class="menu__item text-body" onclick={() => moveTo(name)}>
                          <span class="menu__check"><Icon name="folder" size={18} /></span>{name}
                        </button>
                      {/each}
                      <button type="button" role="menuitem" class="menu__item text-body" onclick={() => moveTo(null)}>
                        <span class="menu__check"><Icon name="minimize" size={18} /></span>No group
                      </button>
                      {#if !groupNames.length}
                        <p class="menu__note text-small">Give an instance a group in its settings to see it here.</p>
                      {/if}
                    </div>
                  {/if}
                </div>
                <Button size="sm" variant="danger" icon="trash" onclick={() => askDelete(markedVisible[0])}>Delete</Button>
                <IconButton icon="close" size="sm" label="Clear selection" onclick={() => (marked = [])} />
              </div>
            {/if}
          </div>
        {/if}

        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class={isList ? 'list' : 'grid'} bind:this={body} {onkeydown} {onkeyup}>
          {#if isList}
            {#if order.length}
              <InstanceRow head />
            {/if}
            {#each sections as section (section.key)}
              {#if headed}
                <div class="group-head">
                  <span class="text-label">{section.name}</span>
                  <span class="text-small muted">{plural(section.items.length, 'instance', 'instances')}</span>
                </div>
              {/if}
              {#each section.items as instance (instance.id)}
                {@const live = liveState(instance)}
                {@const icon = iconOf(instance)}
                <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
                <div data-cell={instance.id} onclickcapture={(e) => clickCapture(e, instance.id)} ondblclickcapture={(e) => dblclickCapture(e, instance.id)}>
                  <InstanceRow
                    name={instance.name}
                    motif={icon.motif}
                    ground={icon.ground}
                    iconSrc={icon.src}
                    version={instance.minecraft}
                    snapshot={isSnapshot(instance.minecraft)}
                    loader={loaderFull(instance.loader)}
                    status={live.status}
                    statusDetail={detailOf(instance, live)}
                    group={rowSub(instance, live)}
                    mods={instance.loader.kind === 'vanilla' ? undefined : mods[instance.id]}
                    lastPlayed={instance.lastPlayed ? relativeTime(instance.lastPlayed) : 'Never'}
                    selected={markedVisible.length ? markedVisible.includes(instance.id) : current?.id === instance.id}
                    onSelect={() => select(instance.id)}
                    onPlay={() => playOrStop(instance.id)}
                  />
                </div>
              {/each}
            {/each}
            {#if !order.length}
              <div class="none">
                <p class="text-body">No instance matches “{filter.trim()}”.</p>
                <Button size="sm" variant="ghost" icon="close" onclick={() => (filter = '')}>Clear filter</Button>
              </div>
            {/if}
          {:else}
            {#each visible as instance (instance.id)}
              {@const live = liveState(instance)}
              {@const icon = iconOf(instance)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="cell" data-cell={instance.id} ondblclickcapture={(e) => dblclickCapture(e, instance.id)}>
                <InstanceTile
                  name={instance.name}
                  motif={icon.motif}
                  ground={icon.ground}
                  iconSrc={icon.src}
                  version={instance.minecraft}
                  snapshot={isSnapshot(instance.minecraft)}
                  loader={instance.loader.kind === 'vanilla' ? undefined : loaderName(instance.loader)}
                  status={live.status}
                  statusDetail={detailOf(instance, live)}
                  progress={live.progress}
                  meta={tileMeta(instance, live)}
                  selected={current?.id === instance.id}
                  onSelect={() => select(instance.id)}
                  onPlay={() => playOrStop(instance.id)}
                />
              </div>
            {/each}
          {/if}
        </div>

        {#if isList}
          <div class="foot">
            <span class="grow"></span>
            <span class="text-small muted">
              {plural(order.length, 'instance', 'instances')}{#if totalSecs >= 60}{' · '}{playTime(totalSecs)} played{/if}
            </span>
          </div>
        {/if}
      </div>

      {#if !isList && current}
        <Inspector id={current.id} />
      {/if}
    </div>
  {/if}
</div>

{#if bulkDelete}
  <DeleteInstanceDialog
    id={bulkDelete[0]}
    ids={bulkDelete}
    onClose={(deleted) => {
      bulkDelete = null
      if (deleted) marked = []
    }}
  />
{/if}

<style>
  .library {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    height: 100%;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  .head,
  .tools,
  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  h1 {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .sort-value {
    color: var(--ink);
    font-weight: 500;
  }
  .muted {
    color: var(--ink-muted);
  }
  .filter {
    flex: 0 1 280px;
    min-width: 200px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: var(--space-5);
    padding-bottom: var(--space-2);
  }
  .cell {
    display: grid;
  }
  .list {
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .group-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-1) var(--space-3);
    background: var(--bg);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .none {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4);
  }
  .none p {
    margin: 0;
  }

  .selection {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--ink);
    box-shadow: var(--shadow-block-sm);
  }
  .selection > :global(.cb-icon) {
    color: var(--grass-text);
  }

  .anchor {
    position: relative;
  }
  .menu {
    position: absolute;
    z-index: var(--layer-menu);
    top: calc(100% + var(--space-1));
    inset-inline-start: 0;
    display: flex;
    flex-direction: column;
    min-width: 220px;
    max-height: 320px;
    overflow: auto;
    padding: var(--space-1) 0;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-float);
  }
  .menu--end {
    inset-inline-start: auto;
    inset-inline-end: 0;
  }
  .menu__item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--size-control);
    padding: 0 var(--space-3) 0 var(--space-2);
    color: var(--ink);
    text-align: start;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .menu__item:hover {
    background: var(--surface-hover);
  }
  .menu__item:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: -3px;
  }
  .menu__check {
    display: inline-flex;
    width: 18px;
    color: var(--grass-text);
  }
  .menu__note {
    margin: 0;
    padding: var(--space-2) var(--space-3);
    color: var(--ink-muted);
    border-top: var(--border-hair) solid var(--line);
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    padding: var(--space-12) var(--space-6);
    text-align: center;
  }
  .empty h1 {
    margin: 0;
  }
  .empty p {
    max-width: 520px;
    margin: 0;
  }
  .empty__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-3);
  }
</style>
