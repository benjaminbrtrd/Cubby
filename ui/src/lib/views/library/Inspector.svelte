<script lang="ts">
  import { Banner, Button, Icon, InstanceIcon, MetaChip, PlayButton, ProgressBar } from '../../cubby'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { iconOf } from '../../icon'
  import { loaderFull, num, playTime, plural, relativeTime, session } from '../../format'
  import type { Server, World } from '../../types'
  import { isSnapshot, liveState, playedLine, taskProgress, taskVerb } from './Library.svelte'

  let { id }: { id: string } = $props()

  const instance = $derived(app.instance(id))
  const live = $derived(instance ? liveState(instance) : null)
  const icon = $derived(iconOf(instance))

  let now = $state(Date.now())
  let content = $state<{ mods: number | null; worlds: World[] | null; shots: number | null } | null>(null)
  let servers = $state<Server[] | null>(null)
  let moreOpen = $state(false)
  let moreAnchor: HTMLElement | undefined = $state()
  let loadedFor = ''

  // Content counts and servers: again when another instance is picked, after a session and after a task of this instance.
  const refreshKey = $derived(
    [id, instance?.installed, instance?.lastPlayed ?? '', app.finished.find((f) => f.id.split(':')[1] === id)?.at ?? 0].join('|'),
  )
  $effect(() => {
    void refreshKey
    const target = id
    if (loadedFor !== target) {
      content = null
      servers = null
      loadedFor = target
    }
    let stale = false
    void Promise.allSettled([api.listMods(target), api.listWorlds(target), api.listScreenshots(target), api.listServers(target)]).then(
      ([m, w, s, sv]) => {
        if (stale) return
        content = {
          mods: m.status === 'fulfilled' ? m.value.length : null,
          worlds: w.status === 'fulfilled' ? w.value : null,
          shots: s.status === 'fulfilled' ? s.value.length : null,
        }
        servers = sv.status === 'fulfilled' ? sv.value : []
      },
    )
    return () => {
      stale = true
    }
  })

  $effect(() => {
    if (!live?.running) return
    now = Date.now()
    const timer = setInterval(() => (now = Date.now()), 15_000)
    return () => clearInterval(timer)
  })

  const busy = $derived(!!live?.running || !!live?.task)
  const server = $derived(servers?.[0])

  const playState = $derived<'ready' | 'installing' | 'running'>(live?.running ? 'running' : live?.task ? 'installing' : 'ready')
  const playDetail = $derived.by(() => {
    if (!instance || !live) return undefined
    if (live.running) return `Playing for ${session(now - app.running[id])}`
    if (live.task) return live.task.total > 1 ? `${live.task.label} · ${num(live.task.done)} / ${num(live.task.total)}` : live.task.label
    if (!instance.installed) return 'Downloads the game first'
    return undefined
  })

  const contentText = $derived.by(() => {
    if (!instance || !content) return null
    const parts: string[] = []
    if (content.mods != null && (content.mods > 0 || instance.loader.kind !== 'vanilla')) parts.push(content.mods ? plural(content.mods, 'mod', 'mods') : 'No mods')
    if (content.worlds) parts.push(content.worlds.length ? plural(content.worlds.length, 'world', 'worlds') : 'No worlds')
    if (content.shots) parts.push(plural(content.shots, 'screenshot', 'screenshots'))
    return parts.join(' · ') || 'Nothing to count yet'
  })

  /** The first task in the queue, for the download summary at the bottom. */
  const firstTask = $derived(Object.entries(app.tasks)[0])
  const taskCount = $derived(Object.keys(app.tasks).length)

  function onPlay() {
    if (!live) return
    if (live.running) void app.kill(id)
    else if (!live.task) void app.launch(id)
  }

  function joinServer() {
    if (server) void app.launch(id, false, server.address)
  }

  function openFolder() {
    api.openFolder(id).catch((e) => app.error(e))
  }

  async function duplicate() {
    closeMore()
    if (!instance) return
    try {
      const copy = await api.duplicateInstance(id, `${instance.name} (copy)`)
      await app.refreshInstances()
      app.selected = copy.id
      app.toast(`Made a copy: ${copy.name}.`, 'success')
    } catch (e) {
      app.error(e)
    }
  }

  function repair() {
    closeMore()
    api.repairInstance(id).catch((e) => app.error(e))
  }

  function trash() {
    closeMore()
    app.deletingId = id
  }

  function settings() {
    closeMore()
    app.openInstance(id, 'settings')
  }

  // More menu

  function closeMore(focusBack = false) {
    moreOpen = false
    if (focusBack) moreAnchor?.querySelector<HTMLElement>('button')?.focus()
  }

  function menuKeys(e: KeyboardEvent) {
    const items = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)')]
    const i = items.indexOf(document.activeElement as HTMLElement)
    let to: number | null = null
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      closeMore(true)
    } else if (e.key === 'Tab') moreOpen = false
    else if (e.key === 'ArrowDown') to = (i + 1) % items.length
    else if (e.key === 'ArrowUp') to = (i - 1 + items.length) % items.length
    else if (e.key === 'Home') to = 0
    else if (e.key === 'End') to = items.length - 1
    if (to != null) {
      e.preventDefault()
      items[to]?.focus()
    }
  }

  function leaveMore(e: FocusEvent) {
    const next = e.relatedTarget as Node | null
    if (!next || !(e.currentTarget as HTMLElement).contains(next)) moreOpen = false
  }

  function focusFirst(node: HTMLElement) {
    node.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus()
  }

  /** Where the player finds the updates: Mods when they are all mods, else Overview, which lists every kind. */
  function updatesTab(instanceId: string): 'mods' | 'overview' {
    const ups = app.updates[instanceId] ?? []
    return ups.length && ups.every((u) => /\.jar(\.disabled)?$/i.test(u.fileName)) ? 'mods' : 'overview'
  }
</script>

{#snippet crashActions()}
  <Button size="sm" icon="info" onclick={() => app.navigate({ name: 'crash', id })}>See what happened</Button>
{/snippet}
{#snippet updateActions()}
  <Button size="sm" icon="update" onclick={() => app.openInstance(id, updatesTab(id))}>Review updates</Button>
{/snippet}

<aside class="inspector" aria-label="Instance details">
  {#if instance && live}
    <div class="who">
      <InstanceIcon motif={icon.motif} ground={icon.ground} src={icon.src} seed={instance.name} size={64} raised />
      <div class="who__text">
        <h2 class="text-heading" title={instance.name}>{instance.name}</h2>
        <p class="text-small muted">{playedLine(instance)}</p>
      </div>
    </div>

    <PlayButton
      size="lg"
      state={playState}
      label={playState === 'installing' ? taskVerb(live.kind) : undefined}
      progress={live.progress}
      detail={playDetail}
      class="play"
      onclick={onPlay}
    />

    {#if live.crashed && app.crash}
      {@const crash = app.crash}
      <Banner tone="danger" title="Crashed last time" actions={crashActions}>
        {relativeTime(crash.at)}, {crash.durationSecs >= 60 ? `after ${playTime(crash.durationSecs)} of play.` : 'right after it started.'}
      </Banner>
    {:else if live.updates}
      <Banner tone="info" title={plural(live.updates, 'update', 'updates') + ' available'} actions={updateActions} />
    {/if}

    <div class="actions">
      <Button size="sm" icon="offline" disabled={busy} onclick={() => app.launch(id, true)}>Offline</Button>
      <Button
        size="sm"
        icon="servers"
        disabled={busy || !server}
        aria-label={server ? `Server: join ${server.name}` : undefined}
        title={server ? `Join ${server.name}` : undefined}
        onclick={joinServer}>Server</Button
      >
      <Button size="sm" icon="folder" onclick={openFolder}>Folder</Button>
      <div class="anchor" bind:this={moreAnchor} onfocusout={leaveMore}>
        <Button size="sm" icon="more" aria-haspopup="menu" aria-expanded={moreOpen} onclick={() => (moreOpen = !moreOpen)}>More</Button>
        {#if moreOpen}
          <div class="menu" role="menu" tabindex="-1" aria-label="More actions" onkeydown={menuKeys} {@attach focusFirst}>
            <button type="button" role="menuitem" class="menu__item text-body" onclick={settings}><Icon name="settings" size={18} />Instance settings</button>
            <button type="button" role="menuitem" class="menu__item text-body" onclick={duplicate}><Icon name="copy" size={18} />Duplicate</button>
            <button type="button" role="menuitem" class="menu__item text-body" disabled={busy || !instance.installed} onclick={repair}>
              <Icon name="wrench" size={18} />Repair game files
            </button>
            <button type="button" role="menuitem" class="menu__item menu__item--danger text-body" disabled={live.running} onclick={trash}>
              <Icon name="trash" size={18} />Move to trash
            </button>
          </div>
        {/if}
      </div>
    </div>
    {#if servers && !servers.length}
      <p class="hint text-small">
        No saved servers yet.
        <button type="button" class="link text-small" onclick={() => app.openInstance(id, 'servers')}>Add one</button>
      </p>
    {/if}

    <dl class="facts">
      <dt class="text-small">Version</dt>
      <dd><MetaChip tone={isSnapshot(instance.minecraft) ? 'snapshot' : undefined}>{instance.minecraft}</MetaChip></dd>
      <dt class="text-small">Loader</dt>
      <dd class="text-mono">{instance.loader.kind === 'vanilla' ? 'None' : loaderFull(instance.loader)}</dd>
      {#if instance.modpack}
        <dt class="text-small">Modpack</dt>
        <dd class="text-body">{instance.modpack.title} <span class="text-mono">{instance.modpack.versionNumber}</span></dd>
      {/if}
      <dt class="text-small">Content</dt>
      <dd class={['text-body', !contentText && 'muted']}>{contentText ?? 'Counting…'}</dd>
      <dt class="text-small">Played</dt>
      <dd class="text-body">{instance.totalPlayedSecs >= 60 ? playTime(instance.totalPlayedSecs) : 'Not yet'}</dd>
      <dt class="text-small">Last played</dt>
      <dd class="text-body">{relativeTime(instance.lastPlayed, 'Never')}</dd>
    </dl>

    <button type="button" class="link open text-body-strong" onclick={() => app.openInstance(id)}>Open instance</button>
  {/if}

  {#if firstTask}
    {@const [key, task] = firstTask}
    <div class="queue">
      <ProgressBar
        value={taskProgress(task)}
        tone="diamond"
        label={app.instance(key.split(':')[1])?.name ?? task.label}
        detail={taskCount > 1 ? `${task.label} · ${plural(taskCount, 'task', 'tasks')} running` : task.label}
      />
      <button type="button" class="link text-body-strong" onclick={() => app.navigate({ name: 'downloads' })}>Open downloads</button>
    </div>
  {/if}
</aside>

<style>
  .inspector {
    box-sizing: border-box;
    flex: 0 0 var(--size-inspector);
    width: var(--size-inspector);
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .who__text {
    min-width: 0;
  }
  .who h2 {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who p {
    margin: 2px 0 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .inspector :global(.play) {
    width: 100%;
  }
  .actions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-2);
  }
  .actions :global(.cb-btn) {
    width: 100%;
  }
  .hint {
    margin: calc(-1 * var(--space-2)) 0 0;
    color: var(--ink-muted);
  }
  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    align-items: center;
    margin: 0;
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .facts dt {
    color: var(--ink-muted);
  }
  .facts dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .link {
    align-self: flex-start;
    padding: 0;
    color: var(--grass-text);
    text-align: start;
    text-decoration: underline;
    text-underline-offset: 3px;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .link:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: 2px;
  }
  .queue {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: auto;
  }

  .anchor {
    position: relative;
  }
  .menu {
    position: absolute;
    z-index: var(--layer-menu);
    top: calc(100% + var(--space-1));
    inset-inline-end: 0;
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
    padding: 0 var(--space-3);
    color: var(--ink);
    text-align: start;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .menu__item:hover:not(:disabled) {
    background: var(--surface-hover);
  }
  .menu__item:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: -3px;
  }
  .menu__item:disabled {
    color: var(--ink-disabled);
    cursor: default;
  }
  .menu__item--danger:not(:disabled) {
    color: var(--redstone-text);
  }
</style>
