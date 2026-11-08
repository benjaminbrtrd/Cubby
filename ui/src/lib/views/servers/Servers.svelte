<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog, Icon, Inspector, InspectorHead, InspectorNotice, InspectorSection, InspectorStatus, InstanceIcon, MetaChip, Motd, PlayButton, ServerRow, StatusTag, TextField } from '../../cubby'
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

  const OFFLINE: ServerStatus = { online: false, motd: '', playersOnline: 0, playersMax: 0, version: null, pingMs: null, iconUrl: null, compatible: null }

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
        api.pingServer(e.server.address, e.instanceId).then(
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

  type Fit = 'match' | 'range' | 'accepted' | 'mismatch' | 'unknown'

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
    const st = selected.status
    const version = st.online ? st.version : null
    // The protocol answer is exact: the server says whether it takes this version. The text of the version
    // ("Requires MC 1.8 / 1.21") is only a hint, used when the protocol is not known.
    const exact = st.online && st.compatible != null
    let f: Fit = fit(version, selInstance.minecraft)
    if (exact) f = st.compatible ? (f === 'match' ? 'match' : 'accepted') : 'mismatch'
    const other = f === 'mismatch' ? fitting(version).find((i) => i.id !== selInstance.id) ?? null : null
    return { fit: f, version, other, exact }
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
      addError = t('Enter the server address.')
      return
    }
    if (/\s/.test(address)) {
      addError = t('An address has no spaces. Check the spelling or the port.')
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
        addError = t('Create an instance first.')
        return
      }
      const existing = await api.listServers(target)
      if (existing.some((s) => s.address.toLowerCase() === address.toLowerCase())) {
        addError = t('{target} already has this server.', { target: nameOf(target) })
        return
      }
      const label = addName.trim() || address
      await api.saveServers(target, [...existing, { name: label, address, iconUrl: null, acceptTextures: null }])
      app.toast(t('{label} is in the server list of {target}.', { label, target: nameOf(target) }), 'success')
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
      app.toast(t('Address copied.'), 'success')
    } catch {
      app.toast(t('Couldn’t copy the address.'), 'error')
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
      app.toast(t('{name} is no longer in the list of {instanceId}.', { name: e.server.name, instanceId: nameOf(e.instanceId) }), 'success')
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
  <Button variant="ghost" onclick={() => (confirmRemove = null)}>{t('Cancel')}</Button>
  <Button variant="danger" icon="trash" busy={removing} disabled={removing} onclick={() => confirmRemove && remove(confirmRemove)}>{t('Remove')}</Button>
{/snippet}

<div class="screen">
  <div class="layout">
    <div class="content">
      <div class="head">
        <h1 class="text-title">{t('Servers')}</h1>
        <span class="grow"></span>
        <Button icon="update" busy={pinging} onclick={() => load()}>{t('Refresh')}</Button>
        {#if app.instances.length}
          <Button variant="primary" icon="plus" onclick={openAdd}>{t('Add server')}</Button>
        {/if}
      </div>

      {#if !app.instances.length}
        <div class="card empty">
          <Icon name="servers" size={27} />
          <div class="grow">
            <p class="text-body-strong">{t('Create an instance first')}</p>
            <p class="text-small muted">{t('Servers belong to an instance: its version is the one you join with.')}</p>
          </div>
          <Button icon="plus" onclick={() => app.navigate({ name: 'create' })}>{t('New instance')}</Button>
        </div>
      {:else}
        {#if adding}
          <section class="card add" aria-labelledby="add-title">
            <h2 id="add-title" class="text-label muted">{t('Add a server')}</h2>
            <form class="add-row" onsubmit={add}>
              <div class="f-addr" bind:this={addrField}>
                <TextField label={t('Server address')} icon="globe" mono placeholder="play.example.net" autocomplete="off" spellcheck={false} bind:value={addr} error={addError} onkeydown={closeOnEscape} />
              </div>
              <div class="f-name">
                <TextField label={t('Name')} placeholder={t('Shown in your list')} bind:value={addName} onkeydown={closeOnEscape} />
              </div>
              <div class="f-with">
                <label for="play-with" class="cb-label text-body-strong">{t('Play with')}</label>
                <select id="play-with" class="select text-body" bind:value={playWith} onkeydown={closeOnEscape}>
                  <option value="auto">{t('Automatic')}</option>
                  {#each app.recent as inst (inst.id)}
                    <option value={inst.id}>{inst.name} · {inst.minecraft}</option>
                  {/each}
                </select>
              </div>
              <div class="f-submit">
                <Button type="submit" icon="plus" busy={saving} disabled={saving}>{t('Add')}</Button>
                {#if entries?.length}<Button variant="ghost" onclick={() => (adding = false)}>{t('Cancel')}</Button>{/if}
              </div>
            </form>
            <p class="hint text-small muted">
              <Icon name="info" size={18} />
              <span>
                {playWith === 'auto'
                  ? t('On Automatic, Cubby picks the instance whose version the server runs.')
                  : t('Adds it to the Multiplayer list of {playWith}.', { playWith: nameOf(playWith) })}
              </span>
            </p>
          </section>
        {/if}

        <section class="section" aria-labelledby="saved-title">
          <div class="sec-head">
            <h2 id="saved-title" class="text-label muted">{t('Saved servers')}</h2>
            {#if entries}<span class="text-counter muted">{num(entries.length)}</span>{/if}
          </div>
          {#if entries == null}
            <p class="text-small muted">{t('Reading the server lists…')}</p>
          {:else if !entries.length}
            <div class="card empty">
              <Icon name="servers" size={27} />
              <div class="grow">
                <p class="text-body-strong">{t('No servers yet')}</p>
                <p class="text-small muted">{t('Add one above. Servers you add in the game’s Multiplayer screen show up here too.')}</p>
              </div>
            </div>
          {:else}
            <ul class="servers" bind:this={listEl} aria-label={t('Saved servers')}>
              {#each entries as e, i (e.key)}
                {@const inst = nameOf(e.instanceId)}
                {@const on = selected?.key === e.key}
                <li class={['entry', on && 'is-selected', !e.status && 'is-pending']}>
                  <button
                    type="button"
                    class="pick"
                    tabindex={on ? 0 : -1}
                    aria-pressed={on}
                    aria-label={t('{name}, {inst}: show details', { name: e.server.name, inst })}
                    onclick={() => select(i, false)}
                    onkeydown={(ev) => onPickKey(ev, i)}
                  ></button>
                  <ServerRow
                    name={e.server.name}
                    address={e.server.address}
                    iconSrc={e.status?.iconUrl ?? e.server.iconUrl ?? undefined}
                    motd={motdOf(e.status)}
                    motdFormatted={e.status?.motdFormatted}
                    online={e.status ? e.status.online : false}
                    offlineText={e.status ? t('Can’t reach server') : t('Checking the server…')}
                    players={e.status?.online ? { online: e.status.playersOnline, max: e.status.playersMax } : undefined}
                    playersLabel={e.status ? t('{n} of {n2} players online', { n: num(e.status.playersOnline), n2: num(e.status.playersMax) }) : undefined}
                    ping={e.status?.pingMs ?? undefined}
                    instance={inst}
                    joinLabel={t('Join {name} with {inst}', { name: e.server.name, inst })}
                    onJoin={() => join(e)}
                  />
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
    </div>

    <aside class="aside" aria-label={t('Server details')}>
      <Inspector class="aside-inner" danger={selected && selInstance ? { label: t('Remove'), hint: t('Takes it off the server list.'), onclick: () => (confirmRemove = selected) } : null}>
        {#if selected && selInstance}
          {@const s = selected.status}
          {@const icon = s?.iconUrl ?? selected.server.iconUrl}
          {@const isRunning = app.running[selInstance.id] != null}
          {@const launch = app.tasks[`launch:${selInstance.id}`]}
          {#snippet svMedia()}
            {#if icon}<span class="cb-ins-media"><img src={icon} alt="" /></span>{:else}<span class="cb-ins-media"><Icon name="servers" size={27} /></span>{/if}
          {/snippet}
          {#snippet svMeta()}
            {#if !s}
              <InspectorStatus tone="off">{t('Checking…')}</InspectorStatus>
            {:else if s.online}
              <InspectorStatus tone="ok">{t('Online')}</InspectorStatus>
              <MetaChip>{num(s.playersOnline)} / {num(s.playersMax)}</MetaChip>
              {#if s.pingMs != null}<MetaChip>{num(s.pingMs)} ms</MetaChip>{/if}
            {:else}
              <InspectorStatus tone="bad">{t('Can’t reach it')}</InspectorStatus>
            {/if}
          {/snippet}
          <InspectorHead eyebrow={t('Server')} title={selected.server.name} sub={selected.server.address} mono media={svMedia} meta={svMeta} />
          <div class="cb-ins-acts">
            {#if isRunning}
              <PlayButton state="running" detail={t('{name} is running', { name: selInstance.name })} aria-label={t('Stop {name}', { name: selInstance.name })} onclick={() => app.kill(selInstance.id)} />
            {:else if launch}
              <PlayButton state="installing" detail={launch.label} progress={launch.total ? (launch.done / launch.total) * 100 : undefined} />
            {:else}
              <PlayButton
                label={t('Join')}
                detail={s && !s.online ? t('Can’t reach it right now') : t('With {name}', { name: selInstance.name })}
                aria-label={t('Join {name} with {name2}', { name: selected.server.name, name2: selInstance.name })}
                onclick={() => join(selected)}
              />
            {/if}
            <Button class="wide" icon="copy" aria-label={t('Copy address')} onclick={() => copyAddress(selected)}>{t('Copy address')}</Button>
          </div>
          {#if check && check.fit === 'mismatch'}
            {#snippet otherAction()}
              {#if check.other}
                {@const other = check.other}
                <Button size="sm" icon="play" onclick={() => join(selected, other.id)}>{t('Join with {name}', { name: other.name })}</Button>
              {/if}
            {/snippet}
            <InspectorNotice action={otherAction}>
              <strong>{t('{name} may not get in.', { name: selInstance.name })}</strong>
              {#if check.exact}
                {t('The server doesn’t take Minecraft')} <span class="text-mono">{selInstance.minecraft}</span>.
              {:else}
                {t('The server runs')} <span class="text-mono">{check.version}</span>{t(', and {name} uses', { name: selInstance.name })} <span class="text-mono">{selInstance.minecraft}</span>.
              {/if}
            </InspectorNotice>
          {/if}
          {#if s?.online && motdOf(s)}
            <InspectorSection title={t('Message of the day')}>
              <Motd text={s.motdFormatted || motdOf(s)} />
            </InspectorSection>
          {/if}
          <InspectorSection title={t('Details')}>
            <dl class="cb-ins-facts">
              <dt>{t('Instance')}</dt>
              <dd><InstanceIcon {...iconOf(selInstance)} seed={selInstance.name} size={24} /><span>{selInstance.name}</span><MetaChip>{selInstance.minecraft}</MetaChip></dd>
              {#if s?.version}<dt>{t('Version')}</dt><dd><span class="text-mono" title={s.version}>{s.version}</span></dd>{/if}
              {#if check}
                <dt>{t('Fit')}</dt>
                <dd>
                  {#if check.fit === 'match' || check.fit === 'accepted'}
                    <InspectorStatus tone="ok">{t('Can join')}</InspectorStatus>
                  {:else if check.fit === 'range'}
                    <InspectorStatus tone="ok">{t('Should work')}</InspectorStatus>
                  {:else if check.fit === 'mismatch'}
                    <InspectorStatus tone="warn">{t('May not get in')}</InspectorStatus>
                  {:else}
                    <InspectorStatus tone="off">{s?.online ? t('Version not given') : t('Known when online')}</InspectorStatus>
                  {/if}
                </dd>
              {/if}
            </dl>
          </InspectorSection>
        {:else}
          {#snippet svEmpty()}<span class="cb-ins-media cb-ins-media--empty"><Icon name="servers" size={27} /></span>{/snippet}
          <InspectorHead eyebrow={t('Server')} title={entries?.length ? t('No server selected') : t('No servers yet')} media={svEmpty} empty />
          <p class="cb-ins-hint">{entries?.length ? t('Select a server to see who’s online and join it.') : t('Add a server to see its status here.')}</p>
        {/if}
      </Inspector>
    </aside>
  </div>
</div>

{#if confirmRemove}
  <Dialog title={t('Remove {name}?', { name: confirmRemove.server.name })} icon="trash" tone="danger" onClose={() => (confirmRemove = null)} actions={removeActions}>
    <p>{t('It leaves the Multiplayer list of {instanceId}.', { instanceId: nameOf(confirmRemove.instanceId) })}</p>
    <p>{t('Nothing changes on the server, and you can add it again.')}</p>
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .layout {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
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
    min-height: 0;
    overflow: auto;
    width: var(--size-inspector);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .aside :global(.aside-inner) {
    position: sticky;
    top: 0;
    max-height: calc(100vh - var(--size-titlebar));
    min-height: 0;
    overflow: auto;
  }
</style>
