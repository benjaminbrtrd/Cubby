<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { untrack } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon, IconButton, Inspector, InspectorNotice, InspectorSection, InstanceIcon, LogViewer, MetaChip, Motd, PlayButton, SegmentedControl, type SegmentedOption, StatusTag, TextField } from '../../cubby'
  import { fold, loaderFull, megabytes, num, plural, session, timeOfDay } from '../../format'
  import { iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Screenshot, Server, ServerStatus } from '../../types'
  import { skinHead } from '../accounts/Accounts.svelte'
  import { LogParser } from './log'

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
    { value: 'all', label: t('All') },
    { value: 'warn', label: t('Warnings {n}', { n: num(counts.warn) }), icon: 'warning' },
    { value: 'error', label: t('Errors {n}', { n: num(counts.error) }), icon: 'error' },
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
      app.toast(t('Log copied to the clipboard.'), 'success')
    } catch {
      app.toast(t('Couldn’t copy the log. Open the logs folder and copy the file from there.'), 'error')
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
    return ms < 60_000 ? t('< 1 min') : session(ms)
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
      app.toast(t('Address copied. Paste it in Multiplayer, Direct Connection.'), 'success')
    } catch {
      app.toast(t('Couldn’t copy the address.'), 'error')
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
      <h1 class="text-title">{t('Game log')}</h1>
      <Banner tone="info" title={t('This instance isn’t here anymore')}>{t('It may have been moved to the trash.')}</Banner>
      <div><Button icon="library" onclick={() => app.navigate({ name: 'library' })}>{t('Back to the library')}</Button></div>
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
                <StatusTag state="installing" detail={launchPct != null ? `${Math.round(launchPct)}%` : null}>{t('Getting ready')}</StatusTag>
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
            <PlayButton state="running" detail={facts.world ?? facts.server ?? undefined} aria-label={t('Stop {name}', { name: instance.name })} onclick={() => app.kill(id)} />
          {:else if launch}
            <PlayButton state="installing" detail={launch.label} progress={launchPct} />
          {:else}
            <PlayButton label={t('Play again')} aria-label={t('Play {name} again', { name: instance.name })} onclick={playAgain} />
          {/if}
          <IconButton icon="settings" label={t('Instance settings')} onclick={() => app.openInstance(id, 'settings')} />
        </header>

        {#if !running && !launch}
          <Banner tone="info" title={t('The game isn’t running')}>
            {#if parsed.length}{t('This is the log of its last session.')}{:else}{t('Play to see the game log here.')}{/if}
          </Banner>
        {/if}

        {#if facts.world || facts.server || account || facts.java || facts.maxMemoryMb || startedAt}
          <dl class="facts" aria-label={t('This session')}>
            {#if facts.world}
              <div class="fact"><dt class="text-small">{t('World')}</dt><dd class="text-body-strong">{facts.world}</dd></div>
            {:else if facts.server}
              <div class="fact"><dt class="text-small">{t('Server')}</dt><dd class="text-mono">{facts.server}</dd></div>
            {/if}
            {#if account}
              <div class="fact">
                <dt class="text-small">{t('Account')}</dt>
                <dd class="account">{@render skinHead(account.url, account.name, 24, '')}<span class="text-body-strong">{account.name}</span></dd>
              </div>
            {/if}
            {#if facts.java}
              <div class="fact"><dt class="text-small">{t('Runtime')}</dt><dd class="text-body-strong">Java <span class="text-mono">{facts.java}</span></dd></div>
            {/if}
            {#if facts.maxMemoryMb}
              <div class="fact"><dt class="text-small">{t('Max memory')}</dt><dd class="text-body-strong">{megabytes(facts.maxMemoryMb)}</dd></div>
            {/if}
            {#if startedAt}
              <div class="fact"><dt class="text-small">{t('Started')}</dt><dd class="text-body-strong">{timeOfDay(new Date(startedAt).toISOString())}</dd></div>
            {/if}
          </dl>
        {/if}

        <section class="log" aria-label={t('Game log')}>
          <div class="cb-log__toolbar toolbar">
            <SegmentedControl label={t('Show lines')} options={levelOptions} value={filter} onChange={(v) => (filter = v as Filter)} />
            <div class="search">
              <TextField label={t('Search the log')} hideLabel icon="search" placeholder={t('Search the log')} size="sm" type="search" bind:value={query} />
            </div>
            {#if !follow && hasLines}
              <Button size="sm" variant="ghost" icon="chevron-down" onclick={toLatest}>{t('Latest lines')}</Button>
            {/if}
            <Button size="sm" icon="copy" disabled={!parsed.length} onclick={copyLog}>{t('Copy log')}</Button>
            <Button size="sm" icon="folder" onclick={openLogs}>{t('Open logs folder')}</Button>
          </div>
          <div class="lines" bind:this={logBox}>
            {#if hasLines}
              <LogViewer lines={visible} live={running} label={running ? t('Game log, live') : t('Game log of the last session')} />
            {:else}
              <div class="empty">
                <Icon name={parsed.length ? 'search' : 'terminal'} size={27} />
                <p class="text-body">
                  {#if parsed.length}
                    {t('No line matches this filter.')}
                  {:else if running || launch}
                    {t('Waiting for the first lines…')}
                  {:else}
                    {t('No log yet.')}
                  {/if}
                </p>
                {#if parsed.length}
                  <Button
                    size="sm"
                    variant="ghost"
                    onclick={() => {
                      filter = 'all'
                      query = ''
                    }}>{t('Show all lines')}</Button
                  >
                {/if}
              </div>
            {/if}
          </div>
        </section>
      </div>

      <aside class="aside" aria-label={running ? t('While you play') : t('Last session')}>
        <Inspector>
          <div class="cb-ins-head">
            <p class="cb-ins-eyebrow">{running ? t('While you play') : t('Last session')}</p>
          </div>
          {#if running}
            <InspectorNotice tone="info">
              {t('Closing Cubby won’t stop the game.')}{#if trackPlayTime}{' '}{t('Keep it open to count this session in your play time.')}{/if}
            </InspectorNotice>
          {/if}
          <InspectorSection title={t('Screenshots')} count={sessionShots?.length ? num(sessionShots.length) : null}>
            {#snippet action()}
              {#if sessionShots?.length}
                <Button size="sm" variant="ghost" iconEnd="chevron-right" onclick={() => app.navigate({ name: 'captures' })}>{t('See all')}</Button>
              {/if}
            {/snippet}
            {#if sessionShots == null}
              <p class="cb-ins-hint">{t('Looking for screenshots…')}</p>
            {:else if !sessionShots.length}
              <p class="cb-ins-hint">{t('None yet. Press F2 in game to take one.')}</p>
            {:else}
              <div class="shots">
                {#each sessionShots.slice(0, 6) as s (s.fileName)}
                  <button type="button" class="shot" aria-label={t('Screenshot taken at {taken}, open Screenshots', { taken: timeOfDay(s.taken) })} onclick={() => app.navigate({ name: 'captures' })}>
                    <img src={s.url} alt="" loading="lazy" />
                  </button>
                {/each}
              </div>
            {/if}
          </InspectorSection>
          {#if server}
            <InspectorSection title={t('Your server')}>
              <ul class="cb-ins-rows">
                <li class="cb-ins-row">
                  <span class="cb-ins-row__ic">
                    {#if status?.iconUrl ?? server.iconUrl}<img src={status?.iconUrl ?? server.iconUrl} alt="" />{:else}<Icon name="servers" size={18} />{/if}
                  </span>
                  <div class="cb-ins-row__txt">
                    <span class="cb-ins-row__t">{server.name}</span>
                    <span class="cb-ins-row__s">
                      {#if !status}{t('Checking the server…')}{:else if status.online}{num(status.playersOnline)}/{num(status.playersMax)} {t('players')}{#if status.pingMs != null} · {status.pingMs} ms{/if}{:else}{t('Can’t reach server')}{/if}
                    </span>
                  </div>
                  {#if running}
                    <IconButton size="sm" icon="copy" label={t('Copy address')} onclick={copyAddress} />
                  {:else}
                    <Button size="sm" icon="play" disabled={!status?.online || !!launch} aria-label={t('Join {name} with {name2}', { name: server.name, name2: instance.name })} onclick={join}>{t('Join')}</Button>
                  {/if}
                </li>
              </ul>
              {#if status?.online && motd}<Motd text={status.motdFormatted || motd} />{/if}
            </InspectorSection>
          {/if}
        </Inspector>
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
  p,
  dl,
  dd {
    margin: 0;
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
    overflow: auto;
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
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
</style>
