<script lang="ts">
  import { untrack } from 'svelte'
  import { api } from '../../api'
  import {
    Banner,
    Button,
    Icon,
    IconButton,
    InstanceIcon,
    LogViewer,
    MetaChip,
    PlayButton,
    SegmentedControl,
    StatusTag,
    TextField,
    type SegmentedOption,
  } from '../../cubby'
  import { fold, loaderFull, megabytes, num, plural, session, timeOfDay } from '../../format'
  import { iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Screenshot, Server, ServerStatus } from '../../types'
  import { skinHead } from '../accounts/Accounts.svelte'
  import { LogParser } from './crash'

  let { id }: { id: string } = $props()

  type Filter = 'all' | 'warn' | 'error'

  const instance = $derived(app.instance(id))
  const startedAt = $derived<number | undefined>(app.running[id])
  const running = $derived(startedAt != null)
  const launch = $derived(app.tasks[`launch:${id}`])
  const launchPct = $derived(launch && launch.total ? (launch.done / launch.total) * 100 : undefined)

  let now = $state(Date.now())
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  // The backend keeps the log of the current or last session: load it when the UI has none yet.
  $effect(() => {
    const lid = id
    untrack(() => {
      if (app.logs[lid]?.length) return
      api
        .gameLog(lid)
        .then((lines) => {
          if (!app.logs[lid]?.length && lines.length) app.logs[lid] = lines
        })
        .catch(() => {})
    })
  })

  // Log
  const parser = new LogParser()
  const parsed = $derived(parser.parse(app.logs[id] ?? []))
  const facts = $derived.by(() => {
    void parsed
    return parser.facts
  })

  let filter = $state<Filter>('all')
  let query = $state('')

  const counts = $derived.by(() => {
    let warn = 0
    let error = 0
    for (const l of parsed) {
      if (l.level === 'warn') warn++
      else if (l.level === 'error') error++
    }
    return { warn, error }
  })

  const levelOptions = $derived<SegmentedOption[]>([
    { value: 'all', label: 'All' },
    { value: 'warn', label: `Warnings ${num(counts.warn)}`, icon: 'warning' },
    { value: 'error', label: `Errors ${num(counts.error)}`, icon: 'error' },
  ])

  const visible = $derived.by(() => {
    const q = fold(query.trim())
    if (filter === 'all' && !q) return parsed
    return parsed.filter((l) => (filter === 'all' || l.level === filter) && (!q || l.search.includes(q)))
  })

  // Autoscroll: follow new lines unless the player scrolled up to read.
  let logBox: HTMLDivElement | undefined = $state()
  let follow = $state(true)
  const hasLines = $derived(visible.length > 0)

  function linesEl(): HTMLElement | null {
    return logBox?.querySelector<HTMLElement>('.cb-log__lines') ?? null
  }

  $effect(() => {
    void hasLines
    const el = linesEl()
    if (!el) return
    const onScroll = () => (follow = el.scrollHeight - el.scrollTop - el.clientHeight < 24)
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  })

  $effect(() => {
    void visible
    if (!follow) return
    const el = untrack(linesEl)
    if (el) el.scrollTop = el.scrollHeight
  })

  function toLatest() {
    follow = true
    const el = linesEl()
    if (el) el.scrollTop = el.scrollHeight
  }

  async function copyLog() {
    const text = (app.logs[id] ?? []).map((l) => l.line).join('\n')
    try {
      await navigator.clipboard.writeText(text)
      app.toast('Log copied to the clipboard.', 'success')
    } catch {
      app.toast('Couldn’t copy the log. Open the logs folder and copy the file from there.', 'error')
    }
  }

  function openLogs() {
    api.openFolder(id, 'logs').catch((e) => app.error(e))
  }

  // Session
  /** Start of the session shown: the running game, else the first line of the last session's log. */
  const sessionStart = $derived(startedAt ?? (app.logs[id]?.[0] ? new Date(app.logs[id][0].time).getTime() : null))
  const sessionEnd = $derived.by(() => {
    if (running) return Infinity
    const list = app.logs[id]
    return list?.length ? new Date(list[list.length - 1].time).getTime() + 60_000 : Infinity
  })

  const account = $derived.by(() => {
    const name = facts.user ?? (running ? app.defaultAccount?.username : undefined)
    if (!name) return null
    const a = app.accounts.find((x) => x.username === name)
    return { name, url: a?.skinUrl ?? null }
  })

  function elapsed(ms: number): string {
    return ms < 60_000 ? '< 1 min' : session(ms)
  }

  function playAgain() {
    void app.launch(id)
  }

  // Aside: screenshots of this session and the instance's first server
  let shots = $state<Screenshot[] | null>(null)
  let server = $state<Server | null | undefined>(undefined)
  let status = $state<ServerStatus | null>(null)

  const sessionShots = $derived.by(() => {
    if (!shots || sessionStart == null) return shots ? [] : null
    return shots.filter((s) => {
      const t = new Date(s.taken).getTime()
      return t >= sessionStart - 2000 && t <= sessionEnd
    })
  })

  async function loadShots() {
    try {
      shots = await api.listScreenshots(id)
    } catch {
      shots ??= []
    }
  }

  async function ping() {
    const s = server
    if (!s) return
    try {
      status = await api.pingServer(s.address)
    } catch {
      status = { online: false, motd: '', playersOnline: 0, playersMax: 0, version: null, pingMs: null, iconUrl: null }
    }
  }

  async function loadServer() {
    try {
      server = (await api.listServers(id))[0] ?? null
    } catch {
      server = null
    }
    void ping()
  }

  $effect(() => {
    untrack(() => void loadServer())
  })

  // Every 30 s while the game runs: new screenshots and the server's player count.
  $effect(() => {
    const live = running
    untrack(() => void loadShots())
    if (!live) return
    const timer = setInterval(() => {
      void loadShots()
      void ping()
    }, 30_000)
    return () => clearInterval(timer)
  })

  async function copyAddress() {
    if (!server) return
    try {
      await navigator.clipboard.writeText(server.address)
      app.toast('Address copied. Paste it in Multiplayer, Direct Connection.', 'success')
    } catch {
      app.toast('Couldn’t copy the address.', 'error')
    }
  }

  function join() {
    if (server) void app.launch(id, false, server.address)
  }

  const motd = $derived((status?.motd ?? '').replace(/§./g, '').trim())
  const trackPlayTime = $derived(app.settings?.trackPlayTime ?? false)
</script>

<div class="screen">
  {#if !instance}
    <div class="content">
      <h1 class="text-title">Game log</h1>
      <Banner tone="info" title="This instance isn’t here anymore">It may have been moved to the trash.</Banner>
      <div><Button icon="library" onclick={() => app.navigate({ name: 'library' })}>Back to the library</Button></div>
    </div>
  {:else}
    <div class="layout">
      <div class="content">
        <header class="head">
          <InstanceIcon {...iconOf(instance)} seed={instance.name} size={64} raised />
          <div class="who">
            <h1 class="text-title name">{instance.name}</h1>
            <div class="tags">
              {#if running && startedAt != null}
                <StatusTag state="running" detail={elapsed(now - startedAt)} />
              {:else if launch}
                <StatusTag state="installing" detail={launchPct != null ? `${Math.round(launchPct)}%` : null}>Getting ready</StatusTag>
              {:else if app.crash?.instanceId === id}
                <StatusTag state="crashed" />
              {:else}
                <StatusTag state="ready" />
              {/if}
              <MetaChip>{instance.minecraft}</MetaChip>
              {#if instance.loader.kind !== 'vanilla'}<MetaChip>{loaderFull(instance.loader)}</MetaChip>{/if}
            </div>
          </div>
          {#if running}
            <PlayButton state="running" detail={facts.world ?? facts.server ?? undefined} aria-label={`Stop ${instance.name}`} onclick={() => app.kill(id)} />
          {:else if launch}
            <PlayButton state="installing" detail={launch.label} progress={launchPct} />
          {:else}
            <PlayButton label="Play again" aria-label={`Play ${instance.name} again`} onclick={playAgain} />
          {/if}
          <IconButton icon="settings" label="Instance settings" onclick={() => app.openInstance(id, 'settings')} />
        </header>

        {#if !running && !launch}
          <Banner tone="info" title="The game isn’t running">
            {#if parsed.length}This is the log of its last session.{:else}Play to see the game log here.{/if}
          </Banner>
        {/if}

        {#if facts.world || facts.server || account || facts.java || facts.maxMemoryMb || startedAt}
          <dl class="facts" aria-label="This session">
            {#if facts.world}
              <div class="fact"><dt class="text-small">World</dt><dd class="text-body-strong">{facts.world}</dd></div>
            {:else if facts.server}
              <div class="fact"><dt class="text-small">Server</dt><dd class="text-mono">{facts.server}</dd></div>
            {/if}
            {#if account}
              <div class="fact">
                <dt class="text-small">Account</dt>
                <dd class="account">{@render skinHead(account.url, account.name, 24, '')}<span class="text-body-strong">{account.name}</span></dd>
              </div>
            {/if}
            {#if facts.java}
              <div class="fact"><dt class="text-small">Runtime</dt><dd class="text-body-strong">Java <span class="text-mono">{facts.java}</span></dd></div>
            {/if}
            {#if facts.maxMemoryMb}
              <div class="fact"><dt class="text-small">Max memory</dt><dd class="text-body-strong">{megabytes(facts.maxMemoryMb)}</dd></div>
            {/if}
            {#if startedAt}
              <div class="fact"><dt class="text-small">Started</dt><dd class="text-body-strong">{timeOfDay(new Date(startedAt).toISOString())}</dd></div>
            {/if}
          </dl>
        {/if}

        <section class="log" aria-label="Game log">
          <div class="cb-log__toolbar toolbar">
            <SegmentedControl label="Show lines" options={levelOptions} value={filter} onChange={(v) => (filter = v as Filter)} />
            <div class="search">
              <TextField label="Search the log" hideLabel icon="search" placeholder="Search the log" size="sm" type="search" bind:value={query} />
            </div>
            {#if !follow && hasLines}
              <Button size="sm" variant="ghost" icon="chevron-down" onclick={toLatest}>Latest lines</Button>
            {/if}
            <Button size="sm" icon="copy" disabled={!parsed.length} onclick={copyLog}>Copy log</Button>
            <Button size="sm" icon="folder" onclick={openLogs}>Open logs folder</Button>
          </div>
          <div class="lines" bind:this={logBox}>
            {#if hasLines}
              <LogViewer lines={visible} live={running} label={running ? 'Game log, live' : 'Game log of the last session'} />
            {:else}
              <div class="empty">
                <Icon name={parsed.length ? 'search' : 'terminal'} size={27} />
                <p class="text-body">
                  {#if parsed.length}
                    No line matches this filter.
                  {:else if running || launch}
                    Waiting for the first lines…
                  {:else}
                    No log yet.
                  {/if}
                </p>
                {#if parsed.length}
                  <Button
                    size="sm"
                    variant="ghost"
                    onclick={() => {
                      filter = 'all'
                      query = ''
                    }}>Show all lines</Button
                  >
                {/if}
              </div>
            {/if}
          </div>
        </section>
      </div>

      <aside class="aside" aria-label={running ? 'While you play' : 'Last session'}>
        <h2 class="text-heading">{running ? 'While you play' : 'Last session'}</h2>

        <section class="block" aria-labelledby="shots-title">
          <div class="block-head">
            <h3 id="shots-title" class="text-body-strong">Screenshots{sessionShots ? `: ${num(sessionShots.length)}` : ''}</h3>
            <span class="text-small muted">this session</span>
          </div>
          {#if sessionShots == null}
            <p class="text-small muted">Looking for screenshots…</p>
          {:else if !sessionShots.length}
            <p class="text-small muted">None yet. Press F2 in game to take one.</p>
          {:else}
            <div class="shots">
              {#each sessionShots.slice(0, 6) as s (s.fileName)}
                <button
                  type="button"
                  class="shot"
                  aria-label={`Screenshot taken at ${timeOfDay(s.taken)}, open Screenshots`}
                  onclick={() => app.navigate({ name: 'captures' })}
                >
                  <img src={s.url} alt="" loading="lazy" />
                </button>
              {/each}
            </div>
            {#if sessionShots.length > 6}
              <Button size="sm" variant="ghost" iconEnd="chevron-right" onclick={() => app.navigate({ name: 'captures' })}>
                See all {plural(sessionShots.length, 'screenshot', 'screenshots')}
              </Button>
            {/if}
          {/if}
        </section>

        {#if server}
          <section class="block" aria-labelledby="server-title">
            <h3 id="server-title" class="text-label muted">Your server</h3>
            <div class="server">
              <div class="server-top">
                <span class="server-icon">
                  {#if status?.iconUrl ?? server.iconUrl}<img src={status?.iconUrl ?? server.iconUrl} alt="" />{:else}<Icon name="servers" size={27} />{/if}
                </span>
                <div class="server-name">
                  <p class="text-body-strong">{server.name}</p>
                  <p class="text-mono muted">{server.address}</p>
                </div>
              </div>
              {#if !status}
                <p class="text-small muted">Checking the server…</p>
              {:else}
                <div class="server-stats">
                  <StatusTag state={status.online ? 'online' : 'offline'} />
                  {#if status.online}
                    <span class="text-small">
                      <span class="text-counter">{num(status.playersOnline)}/{num(status.playersMax)}</span> players
                    </span>
                    {#if status.pingMs != null}<span class="text-mono muted">{status.pingMs} ms</span>{/if}
                  {/if}
                </div>
                {#if status.online && motd}<p class="text-small muted motd">{motd}</p>{/if}
                {#if !status.online}<p class="text-small muted">Can’t reach server</p>{/if}
              {/if}
              {#if running}
                <Button size="sm" icon="copy" onclick={copyAddress}>Copy address</Button>
              {:else}
                <Button size="sm" icon="play" disabled={!status?.online || !!launch} aria-label={`Join ${server.name} with ${instance.name}`} onclick={join}>Join</Button>
              {/if}
            </div>
          </section>
        {/if}

        {#if running}
          <div class="note">
            <Banner tone="info">
              Closing Cubby won’t stop the game.{#if trackPlayTime}{' '}Keep it open to count this session in your play time.{/if}
            </Banner>
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    height: 100%;
  }
  .layout {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  h1,
  h2,
  h3,
  p,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
  }
  .who {
    flex: 1 1 260px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .name {
    overflow-wrap: anywhere;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-4) var(--space-8);
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .fact {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }
  .fact dt {
    color: var(--ink-muted);
  }
  .fact dd {
    overflow-wrap: anywhere;
  }
  .account {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .log {
    flex: 1;
    min-height: 280px;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .toolbar {
    border: var(--border-block) solid var(--edge);
    border-bottom: 0;
  }
  .search {
    flex: 1 1 200px;
    min-width: 160px;
  }
  .lines {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .lines :global(.cb-log) {
    flex: 1;
    min-height: 0;
  }
  .lines :global(.cb-log__lines) {
    flex: 1;
    min-height: 0;
  }
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: var(--space-6);
    color: var(--ink-muted);
    text-align: center;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-well);
  }

  .aside {
    flex: none;
    width: var(--size-inspector);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-4);
    overflow: auto;
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .block-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }
  .shots {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-2);
  }
  .shot {
    display: block;
    padding: 0;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: pointer;
  }
  .shot:hover {
    border-color: var(--ink);
  }
  .shot:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .shot img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
  }

  .server {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .server-top {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    width: 100%;
  }
  .server-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 48px;
    height: 48px;
    overflow: hidden;
    color: var(--ink-muted);
    background: var(--bg-chrome);
    border: var(--border-block) solid var(--edge);
  }
  .server-icon img {
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
  }
  .server-name {
    min-width: 0;
  }
  .server-name p {
    overflow-wrap: anywhere;
  }
  .server-stats {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }
  .motd {
    overflow-wrap: anywhere;
    white-space: pre-line;
  }
  .note {
    margin-top: auto;
  }
</style>
