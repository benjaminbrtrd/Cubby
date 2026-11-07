<!--
  Top of the library: the selected instance over its latest screenshot, its Play button, and the
  worlds and servers it was last played in (Jump back in), one click from being in game.
-->
<script lang="ts" module>
  /** Quick Play opens a world directly from Minecraft 1.20 (snapshot 23w14a) on. */
  export function opensWorlds(version: string): boolean {
    const snap = /^(\d{2})w(\d{2})/.exec(version)
    if (snap) return +snap[1] > 23 || (+snap[1] === 23 && +snap[2] >= 14)
    const rel = /^(\d+)\.(\d+)/.exec(version)
    if (!rel) return false
    return +rel[1] > 1 || +rel[2] >= 20
  }
</script>

<script lang="ts">
  import { Banner, Button, GROUNDS, Icon, IconButton, InstanceIcon, PlayButton } from '../../cubby'
  import { floating } from '../../cubby/floating'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { iconOf } from '../../icon'
  import { loaderFull, num, playTime, plural, relativeTime, session } from '../../format'
  import type { Server, World } from '../../types'
  import { t } from '../../i18n.svelte'
  import SharePackDialog from './SharePackDialog.svelte'
  import { liveState, playedLine, taskVerb } from './Library.svelte'

  let { id, cover }: { id: string; cover: string | null } = $props()
  let sharing = $state(false)

  const instance = $derived(app.instance(id))
  const live = $derived(instance ? liveState(instance) : null)
  const icon = $derived(iconOf(instance))
  const ground = $derived(icon.ground ? GROUNDS[icon.ground] : null)

  let now = $state(Date.now())
  let worlds = $state<World[] | null>(null)
  let servers = $state<Server[] | null>(null)
  let mods = $state<number | null>(null)
  let moreOpen = $state(false)
  let moreAnchor: HTMLElement | undefined = $state()
  let loadedFor = ''

  // Worlds, servers and mods: again when another instance is picked, after a session and after a task of this instance.
  const refreshKey = $derived(
    [id, instance?.installed, instance?.lastPlayed ?? '', app.finished.find((f) => f.id.split(':')[1] === id)?.at ?? 0].join('|'),
  )
  $effect(() => {
    void refreshKey
    const target = id
    if (loadedFor !== target) {
      worlds = null
      servers = null
      mods = null
      loadedFor = target
    }
    let stale = false
    void Promise.allSettled([api.listWorlds(target), api.listServers(target), api.listMods(target)]).then(([w, s, m]) => {
      if (stale) return
      worlds = w.status === 'fulfilled' ? w.value : []
      servers = s.status === 'fulfilled' ? s.value : []
      mods = m.status === 'fulfilled' ? m.value.length : null
    })
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
  const lowerFirst = (s: string) => s.charAt(0).toLowerCase() + s.slice(1)

  const kicker = $derived(instance?.modpack ? t('Modpack {version}', { version: instance.modpack.versionNumber }) : (instance?.group ?? ''))
  const meta = $derived.by(() => {
    if (!instance) return ''
    const parts = [instance.minecraft, instance.loader.kind === 'vanilla' ? 'Vanilla' : loaderFull(instance.loader)]
    if (mods && instance.loader.kind !== 'vanilla') parts.push(plural(mods, 'mod', 'mods'))
    return parts.join(' · ')
  })

  const statusLine = $derived.by(() => {
    if (!instance || !live) return ''
    if (live.running) return t('Playing for {time}', { time: session(now - app.running[id]) })
    if (live.task) return live.task.total > 1 ? t('{label} · {done} of {total}', { label: live.task.label, done: num(live.task.done), total: num(live.task.total) }) : live.task.label
    if (!instance.installed) return t('Play downloads Minecraft {version} first', { version: instance.minecraft })
    return playedLine(instance)
  })

  const playState = $derived<'ready' | 'installing' | 'running'>(live?.running ? 'running' : live?.task ? 'installing' : 'ready')

  type Spot = { key: string; kind: 'world' | 'server'; name: string; sub: string; world?: string; server?: string }

  /** Up to three places to go back to: the last played worlds, and the first saved server. */
  const spots = $derived.by<Spot[] | null>(() => {
    if (!instance || worlds === null || servers === null) return null
    const canOpen = opensWorlds(instance.minecraft)
    const recent = canOpen
      ? [...worlds].filter((w) => w.lastPlayed).sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? ''))
      : []
    const shownServers = servers.slice(0, recent.length ? 1 : 3)
    const shownWorlds = recent.slice(0, 3 - shownServers.length)
    return [
      ...shownWorlds.map((w) => ({ key: `w:${w.folder}`, kind: 'world' as const, name: w.name, sub: t('World · {when}', { when: lowerFirst(relativeTime(w.lastPlayed, 'never')) }), world: w.folder })),
      ...shownServers.map((s) => ({
        key: `s:${s.address}`,
        kind: 'server' as const,
        name: s.name.trim() || s.address,
        sub: s.name.trim() ? t('Server · {address}', { address: s.address }) : t('Server'),
        server: s.address,
      })),
    ]
  })

  function onPlay() {
    if (!live) return
    if (live.running) void app.kill(id)
    else if (!live.task) void app.launch(id)
  }

  function go(spot: Spot) {
    void app.launch(id, false, spot.server ?? null, spot.world ?? null)
  }

  function openFolder() {
    api.openFolder(id).catch((e) => app.error(e))
  }

  /** Where the player finds the updates: Mods when they are all mods, else Overview, which lists every kind. */
  function updatesTab(): 'mods' | 'overview' {
    const ups = app.updates[id] ?? []
    return ups.length && ups.every((u) => /\.jar(\.disabled)?$/i.test(u.fileName)) ? 'mods' : 'overview'
  }

  // More menu

  async function duplicate() {
    closeMore()
    if (!instance) return
    try {
      const copy = await api.duplicateInstance(id, t('{name} (copy)', { name: instance.name }))
      await app.refreshInstances()
      app.selected = copy.id
      app.toast(t('Made a copy: {name}.', { name: copy.name }), 'success')
    } catch (e) {
      app.error(e)
    }
  }

  function playOffline() {
    closeMore()
    void app.launch(id, true)
  }

  function gameLog() {
    closeMore()
    app.navigate({ name: 'running', id })
  }

  function repair() {
    closeMore()
    api.repairInstance(id).catch((e) => app.error(e))
  }

  function trash() {
    closeMore()
    app.deletingId = id
  }

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
</script>

{#snippet crashActions()}
  <Button size="sm" icon="info" onclick={() => app.navigate({ name: 'crash', id })}>{t('See what happened')}</Button>
{/snippet}
{#snippet updateActions()}
  <Button size="sm" icon="update" onclick={() => app.openInstance(id, updatesTab())}>{t('Review')}</Button>
{/snippet}

{#if instance && live}
  <section class="stage" aria-labelledby="stage-name">
    <div class="stage__media" aria-hidden="true">
      {#if cover}
        <img src={cover} alt="" />
      {:else if ground}
        <div class="stage__ground" style:background="linear-gradient(135deg, {ground.base}, {ground.dark})"></div>
      {/if}
      <div class="stage__scrim"></div>
    </div>

    <div class="stage__main">
      <div class="who">
        <InstanceIcon motif={icon.motif} ground={icon.ground} src={icon.src} seed={instance.name} size={72} raised />
        <div class="who__text">
          {#if kicker}<span class="text-label kicker">{kicker}</span>{/if}
          <h1 id="stage-name" class="name" title={instance.name}>{instance.name}</h1>
          <span class="text-mono muted">{meta}</span>
        </div>
      </div>

      <span class="grow"></span>

      {#if live.crashed && app.crash}
        {@const crash = app.crash}
        <Banner tone="danger" title={t('Crashed {when}', { when: lowerFirst(relativeTime(crash.at)) })} actions={crashActions} class="stage__banner">
          {crash.durationSecs >= 60 ? t('After {time} of play.', { time: playTime(crash.durationSecs) }) : t('Right after it started.')}
        </Banner>
      {:else if live.updates && !live.running}
        <Banner tone="info" title={t('{updates} ready', { updates: plural(live.updates, 'update', 'updates') })} actions={updateActions} class="stage__banner" />
      {/if}

      <p class="status text-small">
        {#if live.running}<span class="live" aria-hidden="true"></span>{:else}<Icon name="clock" size={16} />{/if}
        <span class:muted={!live.running}>{statusLine}</span>
      </p>

      <div class="actions">
        <PlayButton
          state={playState}
          label={playState === 'installing' ? taskVerb(live.kind) : undefined}
          progress={live.progress}
          class="stage__play"
          onclick={onPlay}
        />
        <Button size="lg" icon="settings" onclick={() => app.openInstance(id)}>{t('Mods and settings')}</Button>
        <Button size="lg" icon="globe" onclick={() => (sharing = true)}>{t('Share')}</Button>
        <IconButton icon="folder" size="lg" variant="secondary" label={t('Open folder')} onclick={openFolder} />
        <div class="anchor" bind:this={moreAnchor} onfocusout={leaveMore}>
          <IconButton icon="more" size="lg" variant="secondary" label={t('More actions')} aria-haspopup="menu" aria-expanded={moreOpen} onclick={() => (moreOpen = !moreOpen)} />
          {#if moreOpen}
            <div class="menu" role="menu" tabindex="-1" aria-label={t('More actions')} onkeydown={menuKeys} {@attach floating} {@attach focusFirst}>
              <button type="button" role="menuitem" class="menu__item text-body" disabled={busy} onclick={playOffline}><Icon name="offline" size={18} />{t('Play offline')}</button>
              <button type="button" role="menuitem" class="menu__item text-body" onclick={gameLog}><Icon name="terminal" size={18} />{t('Game log')}</button>
              <button type="button" role="menuitem" class="menu__item text-body" onclick={duplicate}><Icon name="copy" size={18} />{t('Duplicate')}</button>
              <button type="button" role="menuitem" class="menu__item text-body" disabled={busy || !instance.installed} onclick={repair}>
                <Icon name="wrench" size={18} />{t('Repair game files')}
              </button>
              <span class="menu__sep" role="separator"></span>
              <button type="button" role="menuitem" class="menu__item menu__item--danger text-body" disabled={live.running} onclick={trash}>
                <Icon name="trash" size={18} />{t('Move to trash')}
              </button>
            </div>
          {/if}
        </div>
      </div>
    </div>

    <aside class="jump" aria-labelledby="jump-title">
      <h2 id="jump-title" class="text-label muted">{t('Jump back in')}</h2>
      {#if spots === null}
        <p class="text-small muted hint">{t('Looking for worlds…')}</p>
      {:else if spots.length}
        <ul>
          {#each spots as spot (spot.key)}
            <li>
              <button type="button" class="spot" disabled={busy} aria-label={t('Play {name}', { name: spot.name })} onclick={() => go(spot)}>
                <span class="spot__icon"><Icon name={spot.kind === 'world' ? 'globe' : 'servers'} size={20} /></span>
                <span class="spot__text">
                  <span class="spot__name text-body-strong">{spot.name}</span>
                  <span class="spot__sub text-small muted">{spot.sub}</span>
                </span>
                <span class="spot__go"><Icon name="play" size={18} /></span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if !opensWorlds(instance.minecraft) && worlds?.length}
        <p class="text-small muted hint">{t('Minecraft {version} can’t open a world straight away. Saved servers show up here.', { version: instance.minecraft })}</p>
      {:else}
        <p class="text-small muted hint">{t('Your worlds and servers show up here after you play. Then one click takes you straight back in.')}</p>
      {/if}
    </aside>
  </section>
{/if}

{#if sharing}
  <SharePackDialog {id} onClose={() => (sharing = false)} />
{/if}

<style>
  .stage {
    position: relative;
    z-index: 2;
    display: flex;
    gap: var(--space-6);
    min-height: 300px;
    padding: var(--space-6) var(--space-8);
    border-bottom: var(--border-chrome) solid var(--edge);
  }
  .stage__media {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background: var(--bg-chrome);
  }
  .stage__media img,
  .stage__ground {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Keeps the text readable over any screenshot. */
  .stage__scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgb(21 18 15 / 0.95) 0%, rgb(21 18 15 / 0.82) 40%, rgb(21 18 15 / 0.3) 72%, rgb(21 18 15 / 0.6) 100%);
  }
  .stage__main,
  .jump {
    position: relative;
  }
  .stage__main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .who__text {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .kicker {
    color: var(--grass-text);
  }
  .name {
    margin: 2px 0 0;
    font-size: 34px;
    line-height: 40px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
    min-height: var(--space-4);
  }
  .stage :global(.stage__banner) {
    max-width: 560px;
    margin-bottom: var(--space-3);
  }
  .status {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-3);
    color: var(--ink-muted);
  }
  .live {
    width: 10px;
    height: 10px;
    flex: none;
    background: var(--grass);
    animation: blink 1.4s steps(2) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .actions :global(.stage__play) {
    min-width: 200px;
  }

  .jump {
    flex: 0 0 330px;
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3);
    background: color-mix(in srgb, var(--bg-chrome) 94%, transparent);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .jump h2 {
    margin: 0;
    padding: var(--space-1) var(--space-2) var(--space-2);
  }
  .jump ul {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .hint {
    margin: var(--space-1) var(--space-2);
  }
  .spot {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) 24px;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: 6px;
    color: var(--ink);
    text-align: start;
    background: transparent;
    border: var(--border-block) solid transparent;
    cursor: pointer;
  }
  .spot:hover:not(:disabled) {
    background: var(--surface);
    border-color: var(--edge);
  }
  .spot:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: -3px;
  }
  .spot:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .spot__icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    color: var(--grass-text);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .spot__text {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .spot__name,
  .spot__sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spot__go {
    display: flex;
    justify-content: center;
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
    min-width: 240px;
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
    padding: 0 var(--space-4) 0 var(--space-3);
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
  .menu__item--danger {
    color: var(--redstone-text);
  }
  .menu__sep {
    height: 1px;
    margin: var(--space-1) 0;
    background: var(--line);
  }
</style>
