<script lang="ts" module>
  import type { Status } from '../../cubby'
  import { app, taskKind, type Task } from '../../state.svelte'
  import { playTime, relativeTime } from '../../format'
  import { t } from '../../i18n.svelte'
  import type { Instance } from '../../types'

  /** What an instance is doing right now, read from the live state. Shared by the library, the stage and the palette. */
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
    if (!instance.lastPlayed) return t('Never played')
    const rel = relativeTime(instance.lastPlayed)
    const when = t('Played {when}', { when: rel.charAt(0).toLowerCase() + rel.slice(1) })
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
        return t('Starting')
      case 'update':
        return t('Updating')
      case 'repair':
        return t('Repairing')
      default:
        return t('Installing')
    }
  }
</script>

<script lang="ts">
  import { Button, Icon, IconButton, InstanceRow, ProgressBar, SegmentedControl } from '../../cubby'
  import { floating } from '../../cubby/floating'
  import { api } from '../../api'
  import { iconOf } from '../../icon'
  import { loaderFull, num, plural, session } from '../../format'
  import DeleteInstanceDialog from './DeleteInstanceDialog.svelte'
  import FirstGame from './FirstGame.svelte'
  import InstanceCard from './InstanceCard.svelte'
  import Stage from './Stage.svelte'

  type Sort = 'played' | 'name' | 'time' | 'created'
  const SORTS: { id: Sort; label: string }[] = [
    { id: 'played', label: 'Last played' },
    { id: 'name', label: 'Name' },
    { id: 'time', label: 'Play time' },
    { id: 'created', label: 'Recently added' },
  ]
  const VIEWS = [
    { value: 'grid', label: 'Cards', icon: 'library' as const },
    { value: 'list', label: 'Rows', icon: 'servers' as const },
  ]
  const byName = new Intl.Collator('en-US', { sensitivity: 'base', numeric: true })

  let sort = $state<Sort>('played')
  /** Rows view: instances picked with Ctrl click, Shift click or Space. */
  let marked = $state<string[]>([])
  let menu = $state<'sort' | 'group' | null>(null)
  let bulkDelete = $state<string[] | null>(null)
  let now = $state(Date.now())
  /** Rows view: number of mod files per instance. */
  let mods = $state<Record<string, number>>({})
  /** instance id → URL of its newest screenshot */
  let covers = $state<Record<string, string>>({})
  let body: HTMLElement | undefined = $state()
  let sortAnchor: HTMLElement | undefined = $state()
  let groupAnchor: HTMLElement | undefined = $state()

  const isList = $derived(app.libraryView === 'list')

  // groups

  const ungrouped = $derived(app.instances.filter((i) => !i.group).length)
  const groupNames = $derived(app.groups.map((g) => g.name).sort(byName.compare))
  const chips = $derived([
    { id: 'all', label: t('All'), count: app.instances.length },
    ...groupNames.map((name) => ({ id: name, label: name, count: app.groups.find((g) => g.name === name)?.count ?? 0 })),
    ...(ungrouped && app.groups.length ? [{ id: '', label: t('No group'), count: ungrouped }] : []),
  ])

  $effect(() => {
    if (!chips.some((c) => c.id === app.groupFilter)) app.groupFilter = 'all'
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

  const visible = $derived.by(() => {
    const inGroup = app.instances.filter((i) => app.groupFilter === 'all' || (i.group ?? '') === app.groupFilter)
    return [...inGroup].sort(compare)
  })

  const headed = $derived(isList && app.groupFilter === 'all' && app.groups.length > 0)
  const sections = $derived.by(() => {
    if (!headed) return [{ key: '*', name: '', items: visible }]
    const map = new Map<string, Instance[]>()
    for (const i of visible) map.set(i.group ?? '', [...(map.get(i.group ?? '') ?? []), i])
    return [...map.entries()]
      .sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : byName.compare(a, b)))
      .map(([name, items]) => ({ key: name || '*none', name: name || t('No group'), items }))
  })

  /** Visual order, for the arrow keys. */
  const order = $derived(isList ? sections.flatMap((s) => s.items) : visible)
  const current = $derived(order.find((i) => i.id === app.selected) ?? order[0])
  /** The stage keeps the selected instance even when the filter hides it. */
  const staged = $derived(app.instance(app.selected) ?? current ?? app.recent[0])
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

  const lastFinished = $derived(app.finished[0]?.at ?? 0)

  // Covers: the newest screenshot of each played instance, read one at a time, again after each session.
  const playedIds = $derived(
    app.instances
      .filter((i) => i.lastPlayed)
      .map((i) => `${i.id}@${i.lastPlayed}`)
      .join('\n'),
  )
  $effect(() => {
    if (!playedIds) return
    const ids = playedIds.split('\n').map((k) => k.slice(0, k.lastIndexOf('@')))
    let stale = false
    void (async () => {
      for (const id of ids) {
        if (stale) return
        try {
          const shots = await api.listScreenshots(id)
          if (stale) return
          if (shots[0]) covers[id] = shots[0].url
          else delete covers[id]
        } catch {
          /* no cover: the card shows the emblem */
        }
      }
    })()
    return () => {
      stale = true
    }
  })

  // The rows show mod counts: read each modded instance's mods folder, one at a time, again after installs and updates.
  const moddedIds = $derived(
    app.instances
      .filter((i) => i.loader.kind !== 'vanilla')
      .map((i) => i.id)
      .join('\n'),
  )
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

  function rowSub(instance: Instance, live: Live): string | undefined {
    if (instance.modpack) return t('Modpack {version}', { version: instance.modpack.versionNumber })
    if (live.status === 'download') return t('Play downloads it first')
    if (instance.totalPlayedSecs >= 60) return t('{time} played', { time: playTime(instance.totalPlayedSecs) })
    return undefined
  }

  /** The first task in the queue, for the bar at the bottom. */
  const firstTask = $derived(Object.entries(app.tasks)[0])
  const taskCount = $derived(Object.keys(app.tasks).length)

  // actions

  function playOrStop(id: string) {
    app.selected = id
    if (app.running[id]) void app.kill(id)
    else void app.launch(id)
  }

  /** Enter on a card or a row. */
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
    app.toast(group ? t('Moved {what} to {group}.', { what, group }) : t('Took {what} out of their group.', { what }), 'success')
  }

  // keyboard: one tab stop in the cards or rows, then the arrows

  const MAIN = '.card__main, .cb-irow__main'

  function mainOf(cell: Element | null | undefined) {
    return cell?.querySelector<HTMLElement>(MAIN) ?? null
  }

  function cellFrom(e: Event): HTMLElement | null {
    const target = e.target as HTMLElement | null
    if (!target?.matches(MAIN)) return null
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

  // Ctrl click and Shift click in the rows mark them instead of selecting one.
  function clickCapture(e: MouseEvent, id: string) {
    if (!isList || !(e.ctrlKey || e.metaKey || e.shiftKey)) return
    if (!(e.target as HTMLElement).closest('.cb-irow__main')) return
    e.preventDefault()
    e.stopPropagation()
    if (e.shiftKey) markRange(id)
    else toggleMark(id)
    app.selected = id
  }

  // A double click opens the instance (the row component would play it).
  function dblclickCapture(e: MouseEvent, id: string) {
    if (!(e.target as HTMLElement).closest(MAIN)) return
    e.stopPropagation()
    app.openInstance(id)
  }

  // Only the selected card or row is in the tab order; its Play button is reached with Enter.
  $effect(() => {
    const root = body
    const focusId = current?.id
    void order
    void isList
    if (!root) return
    for (const cell of root.querySelectorAll<HTMLElement>('[data-cell]')) {
      const main = mainOf(cell)
      if (main) main.tabIndex = cell.dataset.cell === focusId ? 0 : -1
      for (const b of cell.querySelectorAll<HTMLElement>('.card__play button, .cb-irow__action button')) b.tabIndex = -1
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
    <FirstGame />
  {:else}
    <div class="scroll">
      {#if staged}
        <Stage id={staged.id} cover={covers[staged.id] ?? null} />
      {/if}

      <section class="shelf" aria-label={t('Instances')}>
        <div class="tools">
          {#if isList && markedVisible.length}
            <div class="selection" role="region" aria-label={t('Selection')}>
              <Icon name="check" size={18} />
              <span class="text-body-strong">{t('{n} selected', { n: num(markedVisible.length) })}</span>
              <div class="anchor" bind:this={groupAnchor} onfocusout={leaveMenu}>
                <Button size="sm" icon="folder" iconEnd="chevron-down" aria-haspopup="menu" aria-expanded={menu === 'group'} onclick={() => toggleMenu('group')}>
                  {t('Move to group')}
                </Button>
                {#if menu === 'group'}
                  <div class="menu" role="menu" tabindex="-1" aria-label={t('Move to group')} onkeydown={(e) => menuKeys(e, groupAnchor)} {@attach floating} {@attach focusFirst}>
                    {#each groupNames as name (name)}
                      <button type="button" role="menuitem" class="menu__item text-body" onclick={() => moveTo(name)}>
                        <span class="menu__check"><Icon name="folder" size={18} /></span>{name}
                      </button>
                    {/each}
                    <button type="button" role="menuitem" class="menu__item text-body" onclick={() => moveTo(null)}>
                      <span class="menu__check"><Icon name="minimize" size={18} /></span>{t('No group')}
                    </button>
                    {#if !groupNames.length}
                      <p class="menu__note text-small">{t('Give an instance a group in its settings to see it here.')}</p>
                    {/if}
                  </div>
                {/if}
              </div>
              <Button size="sm" variant="danger" icon="trash" onclick={() => askDelete(markedVisible[0])}>{t('Move to trash')}</Button>
              <IconButton icon="close" size="sm" label={t('Clear selection')} onclick={() => (marked = [])} />
            </div>
          {:else if chips.length > 1}
            <div class="chips" role="group" aria-label={t('Groups')}>
              {#each chips as chip (chip.id)}
                <button type="button" class={['chip', app.groupFilter === chip.id && 'is-on']} aria-pressed={app.groupFilter === chip.id} onclick={() => (app.groupFilter = chip.id)}>
                  {chip.label}<span class="chip__count">{num(chip.count)}</span>
                </button>
              {/each}
            </div>
          {/if}
          <span class="grow"></span>
          <div class="anchor" bind:this={sortAnchor} onfocusout={leaveMenu}>
            <Button variant="ghost" size="sm" iconEnd="chevron-down" aria-haspopup="menu" aria-expanded={menu === 'sort'} onclick={() => toggleMenu('sort')}>
              <span class="text-body"><span class="muted">{t('Sort:')}</span> {t(SORTS.find((s) => s.id === sort)?.label ?? '')}</span>
            </Button>
            {#if menu === 'sort'}
              <div class="menu menu--end" role="menu" tabindex="-1" aria-label={t('Sort by')} onkeydown={(e) => menuKeys(e, sortAnchor)} {@attach floating} {@attach focusFirst}>
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
                    {t(s.label)}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          <SegmentedControl label={t('View')} options={VIEWS.map((v) => ({ ...v, label: t(v.label) }))} value={app.libraryView} onChange={setView} />
          <Button variant="primary" icon="plus" onclick={() => app.navigate({ name: 'create' })}>{t('New instance')}</Button>
        </div>

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
                    lastPlayed={instance.lastPlayed ? relativeTime(instance.lastPlayed) : t('Never')}
                    selected={markedVisible.length ? markedVisible.includes(instance.id) : current?.id === instance.id}
                    onSelect={() => select(instance.id)}
                    onPlay={() => playOrStop(instance.id)}
                  />
                </div>
              {/each}
            {/each}
          {:else}
            {#each visible as instance (instance.id)}
              {@const live = liveState(instance)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="cell" data-cell={instance.id} ondblclickcapture={(e) => dblclickCapture(e, instance.id)}>
                <InstanceCard
                  {instance}
                  {live}
                  cover={covers[instance.id] ?? null}
                  detail={detailOf(instance, live)}
                  selected={current?.id === instance.id}
                  onSelect={() => select(instance.id)}
                  onPlay={() => playOrStop(instance.id)}
                />
              </div>
            {/each}
            {#if app.groupFilter === 'all'}
              <button type="button" class="new text-body-strong" onclick={() => app.navigate({ name: 'create' })}>
                <span class="new__icon"><Icon name="plus" size={22} /></span>
                {t('New instance')}
              </button>
            {/if}
          {/if}
        </div>

        {#if isList}
          <p class="foot text-small muted">
            {plural(order.length, 'instance', 'instances')}{#if totalSecs >= 60}{' · '}{t('{time} played', { time: playTime(totalSecs) })}{/if}
          </p>
        {/if}
      </section>
    </div>

    {#if firstTask}
      {@const [key, task] = firstTask}
      <div class="activity" role="status">
        <ProgressBar
          value={taskProgress(task)}
          tone="diamond"
          label={app.instance(key.split(':')[1])?.name ?? task.label}
          detail={taskCount > 1 ? t('{label} · {tasks} running', { label: task.label, tasks: plural(taskCount, 'task', 'tasks') }) : task.label}
          class="activity__bar"
        />
        <Button size="sm" icon="download" onclick={() => app.navigate({ name: 'downloads' })}>{t('Open downloads')}</Button>
      </div>
    {/if}
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
    height: 100%;
    min-height: 0;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .shelf {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-5) var(--space-6) var(--space-8);
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-height: 40px;
  }
  .grow {
    flex: 1;
  }
  .muted {
    color: var(--ink-muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 34px;
    padding: 0 var(--space-3);
    font: inherit;
    font-size: 15px;
    font-weight: 500;
    color: var(--ink-muted);
    background: transparent;
    border: var(--border-block) solid transparent;
    cursor: pointer;
  }
  .chip:hover {
    color: var(--ink);
    background: var(--surface);
  }
  .chip.is-on {
    color: var(--ink);
    background: var(--surface);
    border-color: var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .chip:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: 2px;
  }
  .chip__count {
    font-size: 13px;
    color: var(--ink-muted);
  }
  .chip.is-on .chip__count {
    color: var(--grass-text);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--space-5);
  }
  .cell {
    display: grid;
  }
  .new {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    min-height: 224px;
    color: var(--ink-muted);
    background: transparent;
    border: var(--border-block) dashed var(--outline);
    cursor: pointer;
  }
  .new:hover {
    color: var(--ink);
    background: color-mix(in srgb, var(--surface) 50%, transparent);
  }
  .new:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: 2px;
  }
  .new__icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border: var(--border-block) solid currentColor;
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
  .foot {
    margin: 0;
    text-align: end;
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

  .activity {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-6);
    background: var(--bg-chrome);
    border-top: var(--border-chrome) solid var(--edge);
  }
  .activity :global(.activity__bar) {
    flex: 1;
    min-width: 0;
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
</style>
