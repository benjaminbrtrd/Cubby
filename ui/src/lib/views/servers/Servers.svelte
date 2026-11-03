<script lang="ts">
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog, Icon, InstanceIcon, MetaChip, PlayButton, ServerRow, StatusTag, TextField } from '../../cubby'
  import { num } from '../../format'
  import { iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Instance, Server, ServerStatus } from '../../types'

  interface Entry {
    key: string
    instanceId: string
    index: number
    server: Server
    /** undefined while the ping is on its way. */
    status: ServerStatus | undefined
  }

  const OFFLINE: ServerStatus = { online: false, motd: '', playersOnline: 0, playersMax: 0, version: null, pingMs: null, iconUrl: null }

  let entries = $state<Entry[] | null>(null)
  let selectedKey = $state<string | null>(null)
  let pinging = $state(false)
  let generation = 0

  async function load() {
    const gen = ++generation
    pinging = true
    const lists = await Promise.all(
      app.recent.map((inst) =>
        api.listServers(inst.id).then(
          (servers) => ({ inst, servers }),
          () => ({ inst, servers: [] as Server[] }),
        ),
      ),
    )
    if (gen !== generation) return
    entries = lists.flatMap(({ inst, servers }) =>
      servers.map((server, index) => ({ key: `${inst.id}#${index}`, instanceId: inst.id, index, server, status: undefined })),
    )
    if (!entries.length) adding = true
    await Promise.all(
      entries.map((e) =>
        api.pingServer(e.server.address).then(
          (s) => {
            if (gen === generation) e.status = s
          },
          () => {
            if (gen === generation) e.status = OFFLINE
          },
        ),
      ),
    )
    if (gen === generation) pinging = false
  }

  const instanceIds = $derived(app.instances.map((i) => i.id).join(','))
  $effect(() => {
    void instanceIds
    untrack(() => void load())
  })

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'F5') {
      e.preventDefault()
      void load()
    }
  }

  const selected = $derived(entries?.find((e) => e.key === selectedKey) ?? entries?.[0] ?? null)
  const selInstance = $derived(app.instance(selected?.instanceId))

  function nameOf(id: string): string {
    return app.instance(id)?.name ?? id
  }

  function motdOf(s: ServerStatus | undefined): string {
    return (s?.motd ?? '').replace(/§./g, '').trim()
  }

  function join(e: Entry, instanceId = e.instanceId) {
    void app.launch(instanceId, false, e.server.address)
    // Follow the start on the game screen, unless it didn't start (no account, for one).
    if (app.tasks[`launch:${instanceId}`]) app.navigate({ name: 'running', id: instanceId })
  }

  // Selection: one tab stop on the selected row, then the arrow keys.
  let listEl: HTMLElement | undefined = $state()

  async function select(i: number, focus = true) {
    if (!entries?.length) return
    const at = Math.max(0, Math.min(entries.length - 1, i))
    selectedKey = entries[at].key
    if (!focus) return
    await tick()
    listEl?.querySelectorAll<HTMLButtonElement>('.pick')[at]?.focus()
  }

  function onPickKey(e: KeyboardEvent, i: number) {
    const moves: Record<string, number> = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: (entries?.length ?? 1) - 1 }
    if (e.key in moves) {
      e.preventDefault()
      void select(moves[e.key])
    }
  }

  // Version check: does the instance's Minecraft version match the server's?
  /** Minecraft versions named in a server's version text ("Paper 1.21.1", "1.8.x-1.21.x", "24w45a"). */
  function versionsIn(text: string | null): string[] {
    if (!text) return []
    return [...text.matchAll(/\b(1\.\d+(?:\.\d+)?|\d{2}w\d{2}[a-z])\b/g)].map((m) => m[1].replace(/\.0$/, ''))
  }

  function majorMinor(v: string): number[] {
    return v.split('.').slice(0, 2).map(Number)
  }

  function cmp(a: number[], b: number[]): number {
    return a[0] - b[0] || (a[1] ?? 0) - (b[1] ?? 0)
  }

  type Fit = 'match' | 'range' | 'mismatch' | 'unknown'

  function fit(serverVersion: string | null, mc: string): Fit {
    const list = versionsIn(serverVersion)
    if (!list.length) return 'unknown'
    const v = mc.replace(/\.0$/, '')
    if (list.includes(v)) return 'match'
    if (list.length >= 2 && /\d\s*(-|–|to)\s*\d/.test(serverVersion ?? '') && /^1\./.test(v)) {
      const lo = majorMinor(list[0])
      const hi = majorMinor(list[list.length - 1])
      const mine = majorMinor(v)
      if (cmp(mine, lo) >= 0 && cmp(mine, hi) <= 0) return 'range'
    }
    return 'mismatch'
  }

  /** Instances whose version the server runs, most recently played first. */
  function fitting(serverVersion: string | null): Instance[] {
    return app.recent.filter((i) => {
      const f = fit(serverVersion, i.minecraft)
      return f === 'match' || f === 'range'
    })
  }

  const check = $derived.by(() => {
    if (!selected || !selInstance || !selected.status) return null
    const version = selected.status.online ? selected.status.version : null
    const f = fit(version, selInstance.minecraft)
    const other = f === 'mismatch' ? fitting(version).find((i) => i.id !== selInstance.id) ?? null : null
    return { fit: f, version, other }
  })

  // Add a server
  let adding = $state(false)
  let addr = $state('')
  let addName = $state('')
  let playWith = $state('auto')
  let addError = $state<string | undefined>(undefined)
  let saving = $state(false)
  let addrField: HTMLDivElement | undefined = $state()

  async function openAdd() {
    adding = true
    await tick()
    addrField?.querySelector('input')?.focus()
  }

  /** Esc in a field closes the form (when there is a list to go back to). */
  function closeOnEscape(e: KeyboardEvent) {
    if (e.key === 'Escape' && entries?.length) {
      e.preventDefault()
      adding = false
    }
  }

  async function add(e: SubmitEvent) {
    e.preventDefault()
    const address = addr.trim()
    addError = undefined
    if (!address) {
      addError = 'Enter the server address.'
      return
    }
    if (/\s/.test(address)) {
      addError = 'An address has no spaces. Check the spelling or the port.'
      return
    }
    saving = true
    try {
      let target = playWith
      if (target === 'auto') {
        const status = await api.pingServer(address).catch(() => null)
        target = (status?.online ? fitting(status.version)[0] : undefined)?.id ?? app.recent[0]?.id ?? ''
      }
      if (!target) {
        addError = 'Create an instance first.'
        return
      }
      const existing = await api.listServers(target)
      if (existing.some((s) => s.address.toLowerCase() === address.toLowerCase())) {
        addError = `${nameOf(target)} already has this server.`
        return
      }
      const label = addName.trim() || address
      await api.saveServers(target, [...existing, { name: label, address, iconUrl: null, acceptTextures: null }])
      app.toast(`${label} is in the server list of ${nameOf(target)}.`, 'success')
      selectedKey = `${target}#${existing.length}`
      addr = ''
      addName = ''
      adding = false
      await load()
    } catch (err) {
      app.error(err)
    } finally {
      saving = false
    }
  }

  // Copy and remove
  async function copyAddress(e: Entry) {
    try {
      await navigator.clipboard.writeText(e.server.address)
      app.toast('Address copied.', 'success')
    } catch {
      app.toast('Couldn’t copy the address.', 'error')
    }
  }

  let confirmRemove = $state<Entry | null>(null)
  let removing = $state(false)

  async function remove(e: Entry) {
    removing = true
    try {
      // Read the list again: the game may have changed it since.
      const list = await api.listServers(e.instanceId)
      const at = list.findIndex((s) => s.address === e.server.address && s.name === e.server.name)
      if (at >= 0) {
        list.splice(at, 1)
        await api.saveServers(e.instanceId, list)
      }
      app.toast(`${e.server.name} is no longer in the list of ${nameOf(e.instanceId)}.`, 'success')
      confirmRemove = null
      selectedKey = null
      await load()
    } catch (err) {
      app.error(err)
    } finally {
      removing = false
    }
  }
</script>

<svelte:window {onkeydown} />

{#snippet removeActions()}
  <Button variant="ghost" onclick={() => (confirmRemove = null)}>Cancel</Button>
  <Button variant="danger" icon="trash" busy={removing} disabled={removing} onclick={() => confirmRemove && remove(confirmRemove)}>Remove</Button>
{/snippet}

<div class="screen">
  <div class="layout">
    <div class="content">
      <div class="head">
        <h1 class="text-title">Servers</h1>
        <span class="grow"></span>
        <Button icon="update" busy={pinging} onclick={() => load()}>Refresh</Button>
        {#if app.instances.length}
          <Button variant="primary" icon="plus" onclick={openAdd}>Add server</Button>
        {/if}
      </div>

      {#if !app.instances.length}
        <div class="card empty">
          <Icon name="servers" size={27} />
          <div class="grow">
            <p class="text-body-strong">Create an instance first</p>
            <p class="text-small muted">Servers belong to an instance: its version is the one you join with.</p>
          </div>
          <Button icon="plus" onclick={() => app.navigate({ name: 'create' })}>New instance</Button>
        </div>
      {:else}
        {#if adding}
          <section class="card add" aria-labelledby="add-title">
            <h2 id="add-title" class="text-label muted">Add a server</h2>
            <form class="add-row" onsubmit={add}>
              <div class="f-addr" bind:this={addrField}>
                <TextField label="Server address" icon="globe" mono placeholder="play.example.net" autocomplete="off" spellcheck={false} bind:value={addr} error={addError} onkeydown={closeOnEscape} />
              </div>
              <div class="f-name">
                <TextField label="Name" placeholder="Shown in your list" bind:value={addName} onkeydown={closeOnEscape} />
              </div>
              <div class="f-with">
                <label for="play-with" class="cb-label text-body-strong">Play with</label>
                <select id="play-with" class="select text-body" bind:value={playWith} onkeydown={closeOnEscape}>
                  <option value="auto">Automatic</option>
                  {#each app.recent as inst (inst.id)}
                    <option value={inst.id}>{inst.name} · {inst.minecraft}</option>
                  {/each}
                </select>
              </div>
              <div class="f-submit">
                <Button type="submit" icon="plus" busy={saving} disabled={saving}>Add</Button>
                {#if entries?.length}<Button variant="ghost" onclick={() => (adding = false)}>Cancel</Button>{/if}
              </div>
            </form>
            <p class="hint text-small muted">
              <Icon name="info" size={18} />
              <span>
                {playWith === 'auto'
                  ? 'On Automatic, Cubby picks the instance whose version the server runs.'
                  : `Adds it to the Multiplayer list of ${nameOf(playWith)}.`}
              </span>
            </p>
          </section>
        {/if}

        <section class="section" aria-labelledby="saved-title">
          <div class="sec-head">
            <h2 id="saved-title" class="text-label muted">Saved servers</h2>
            {#if entries}<span class="text-counter muted">{num(entries.length)}</span>{/if}
          </div>
          {#if entries == null}
            <p class="text-small muted">Reading the server lists…</p>
          {:else if !entries.length}
            <div class="card empty">
              <Icon name="servers" size={27} />
              <div class="grow">
                <p class="text-body-strong">No servers yet</p>
                <p class="text-small muted">Add one above. Servers you add in the game’s Multiplayer screen show up here too.</p>
              </div>
            </div>
          {:else}
            <ul class="servers" bind:this={listEl} aria-label="Saved servers">
              {#each entries as e, i (e.key)}
                {@const inst = nameOf(e.instanceId)}
                {@const on = selected?.key === e.key}
                <li class={['entry', on && 'is-selected', !e.status && 'is-pending']}>
                  <button
                    type="button"
                    class="pick"
                    tabindex={on ? 0 : -1}
                    aria-pressed={on}
                    aria-label={`${e.server.name}, ${inst}: show details`}
                    onclick={() => select(i, false)}
                    onkeydown={(ev) => onPickKey(ev, i)}
                  ></button>
                  <ServerRow
                    name={e.server.name}
                    address={e.server.address}
                    iconSrc={e.status?.iconUrl ?? e.server.iconUrl ?? undefined}
                    motd={motdOf(e.status)}
                    online={e.status ? e.status.online : false}
                    offlineText={e.status ? 'Can’t reach server' : 'Checking the server…'}
                    players={e.status?.online ? { online: e.status.playersOnline, max: e.status.playersMax } : undefined}
                    playersLabel={e.status ? `${num(e.status.playersOnline)} of ${num(e.status.playersMax)} players online` : undefined}
                    ping={e.status?.pingMs ?? undefined}
                    instance={inst}
                    joinLabel={`Join ${e.server.name} with ${inst}`}
                    onJoin={() => join(e)}
                  />
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
    </div>

    <aside class="aside" aria-label="Server details">
      <div class="aside-inner">
        {#if selected && selInstance}
          {@const s = selected.status}
          {@const icon = s?.iconUrl ?? selected.server.iconUrl}
          {@const isRunning = app.running[selInstance.id] != null}
          {@const launch = app.tasks[`launch:${selInstance.id}`]}
          <div class="who">
            {#if icon}
              <img class="big-icon" src={icon} alt={`${selected.server.name} server icon`} width="64" height="64" />
            {:else}
              <span class="big-icon placeholder" aria-hidden="true"><Icon name="servers" size={36} /></span>
            {/if}
            <div class="who-text">
              <h2 class="text-heading">{selected.server.name}</h2>
              <p class="text-mono muted address">{selected.server.address}</p>
            </div>
          </div>

          {#if isRunning}
            <PlayButton state="running" size="lg" detail={`${selInstance.name} is running`} aria-label={`Stop ${selInstance.name}`} onclick={() => app.kill(selInstance.id)} />
          {:else if launch}
            <PlayButton state="installing" size="lg" detail={launch.label} progress={launch.total ? (launch.done / launch.total) * 100 : undefined} />
          {:else}
            <PlayButton
              size="lg"
              label="Join"
              detail={s && !s.online ? 'Can’t reach it right now' : `With ${selInstance.name}`}
              aria-label={`Join ${selected.server.name} with ${selInstance.name}`}
              onclick={() => join(selected)}
            />
          {/if}

          <div class="actions">
            <Button size="sm" icon="copy" onclick={() => copyAddress(selected)}>Copy address</Button>
            <Button size="sm" variant="ghost" icon="trash" onclick={() => (confirmRemove = selected)}>Remove</Button>
          </div>

          {#if s?.online && motdOf(s)}
            <div>
              <p class="text-label muted label">Message of the day</p>
              <p class="text-mono motd">{motdOf(s)}</p>
            </div>
          {/if}

          <dl class="facts">
            <dt class="text-small muted">Status</dt>
            <dd class="dd-row">
              {#if !s}
                <span class="text-small muted">Checking…</span>
              {:else}
                <StatusTag state={s.online ? 'online' : 'offline'} />
                {#if s.online && s.pingMs != null}<span class="text-mono">{s.pingMs} ms</span>{/if}
              {/if}
            </dd>
            {#if s?.online}
              <dt class="text-small muted">Players</dt>
              <dd class="text-body">{num(s.playersOnline)} of {num(s.playersMax)}</dd>
              {#if s.version}
                <dt class="text-small muted">Version</dt>
                <dd class="dd-row"><MetaChip>{s.version}</MetaChip></dd>
              {/if}
            {/if}
            <dt class="text-small muted">Instance</dt>
            <dd class="dd-row">
              <InstanceIcon {...iconOf(selInstance)} seed={selInstance.name} size={24} />
              <span class="text-body">{selInstance.name}</span>
              <MetaChip>{selInstance.minecraft}</MetaChip>
            </dd>
          </dl>

          {#if check}
            <div class={['fit', `fit--${check.fit}`]}>
              <span class="fit-icon">
                <Icon name={check.fit === 'mismatch' ? 'warning' : check.fit === 'unknown' ? 'info' : 'check'} size={18} />
              </span>
              <div class="fit-text">
                {#if check.fit === 'match'}
                  <p class="text-body-strong">{selInstance.name} matches the server</p>
                  <p class="text-small muted">Both use Minecraft <span class="text-mono">{selInstance.minecraft}</span>.</p>
                {:else if check.fit === 'range'}
                  <p class="text-body-strong">{selInstance.name} should work</p>
                  <p class="text-small muted">The server accepts <span class="text-mono">{check.version}</span>.</p>
                {:else if check.fit === 'mismatch'}
                  <p class="text-body-strong">{selInstance.name} may not get in</p>
                  <p class="text-small muted">
                    The server runs <span class="text-mono">{check.version}</span>, and {selInstance.name} uses
                    <span class="text-mono">{selInstance.minecraft}</span>.
                  </p>
                  {#if check.other}
                    {@const other = check.other}
                    <div class="fit-action">
                      <Button size="sm" icon="play" onclick={() => join(selected, other.id)}>Join with {other.name}</Button>
                    </div>
                  {/if}
                {:else}
                  <p class="text-body-strong">Version not known</p>
                  <p class="text-small muted">
                    {s?.online ? 'The server doesn’t say which Minecraft version it runs.' : 'Cubby can check it when the server is online.'}
                  </p>
                {/if}
              </div>
            </div>
          {/if}
        {:else}
          <h2 class="text-heading">Server details</h2>
          <p class="text-small muted">{entries?.length ? 'Select a server to see its details.' : 'Add a server to see its status here.'}</p>
        {/if}
      </div>
    </aside>
  </div>
</div>

{#if confirmRemove}
  <Dialog title={`Remove ${confirmRemove.server.name}?`} icon="trash" tone="danger" onClose={() => (confirmRemove = null)} actions={removeActions}>
    <p>It leaves the Multiplayer list of {nameOf(confirmRemove.instanceId)}.</p>
    <p>Nothing changes on the server, and you can add it again.</p>
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .layout {
    flex: 1;
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
  p,
  ul,
  dl,
  dd {
    margin: 0;
  }
  ul {
    list-style: none;
    padding: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .card {
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .empty {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-4);
    color: var(--ink-muted);
  }
  .empty .text-body-strong {
    color: var(--ink);
  }

  .add {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--space-3) var(--space-4);
  }
  .add-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-3) var(--space-4);
  }
  .f-addr {
    flex: 2 1 240px;
    min-width: 0;
  }
  .f-name {
    flex: 1 1 160px;
    min-width: 0;
  }
  .f-with {
    flex: 1 1 200px;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .f-submit {
    display: flex;
    gap: var(--space-2);
    /* Lines the buttons up with the fields, under their labels. */
    padding-top: 28px;
  }
  .select {
    width: 100%;
    min-height: var(--size-control);
    padding: 0 var(--space-2) 0 var(--space-3);
    color: var(--ink);
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    border-radius: 0;
    box-shadow: var(--shadow-well);
    cursor: pointer;
  }
  .select:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .hint {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .sec-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }
  .servers {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  /* The whole row selects it; the Join button stays on top and keeps its own click. */
  .entry {
    position: relative;
  }
  .pick {
    position: absolute;
    inset: 0;
    z-index: 1;
    padding: 0;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .pick:focus-visible {
    outline: 2px solid transparent;
  }
  .entry:has(.pick:focus-visible) {
    box-shadow: var(--focus-ring);
  }
  .entry :global(.cb-server__join) {
    position: relative;
    z-index: 2;
  }
  .entry.is-selected::after {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 3;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
  }
  .entry.is-pending :global(.cb-server__stats) {
    visibility: hidden;
  }

  .aside {
    flex: none;
    width: var(--size-inspector);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .aside-inner {
    position: sticky;
    top: 0;
    max-height: calc(100vh - var(--size-titlebar));
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
  }
  .aside-inner :global(.cb-play) {
    width: 100%;
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .who-text {
    min-width: 0;
  }
  .address {
    margin-top: var(--space-1);
    overflow-wrap: anywhere;
  }
  .big-icon {
    display: block;
    flex: none;
    width: 64px;
    height: 64px;
    image-rendering: pixelated;
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-muted);
    background: var(--surface);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .label {
    margin-bottom: 6px;
  }
  .motd {
    padding: var(--space-2) var(--space-3);
    white-space: pre-line;
    overflow-wrap: anywhere;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .dd-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .fit {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .fit-icon {
    display: inline-flex;
    flex: none;
    padding-top: 2px;
  }
  .fit--match .fit-icon,
  .fit--range .fit-icon {
    color: var(--grass-text);
  }
  .fit--mismatch .fit-icon {
    color: var(--gold-text);
  }
  .fit--unknown .fit-icon {
    color: var(--diamond-text);
  }
  .fit-text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fit-action {
    margin-top: var(--space-2);
  }
</style>
