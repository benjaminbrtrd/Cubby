<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog, Icon, IconButton, Inspector, InspectorHead, InspectorNotice, InspectorSection, InspectorStatus, MetaChip, Motd, PlayButton, SegmentedControl, ServerRow, StatusTag, TextField } from '../../cubby'
  import { num, plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, Server, ServerStatus } from '../../types'
  import type { InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  const servers = $derived(data.servers ?? [])
  const running = $derived(!!app.running[id])
  const preparing = $derived(!!app.instanceTask(id))

  // Server List Ping, by address
  let statuses = $state<Record<string, ServerStatus>>({})
  let pending = $state<Record<string, boolean>>({})
  let generation = 0
  const OFFLINE: ServerStatus = { online: false, motd: '', playersOnline: 0, playersMax: 0, version: null, pingMs: null, iconUrl: null, compatible: null }
  const anyPending = $derived(Object.values(pending).some(Boolean))

  async function ping(address: string) {
    const gen = generation
    pending[address] = true
    let s: ServerStatus
    try {
      s = await api.pingServer(address, id)
    } catch {
      s = OFFLINE
    }
    if (gen !== generation) return
    statuses[address] = s
    pending[address] = false
  }

  function refresh() {
    generation++
    statuses = {}
    pending = {}
    for (const a of new Set(servers.map((s) => s.address))) void ping(a)
  }

  let pingedOnce = false
  $effect(() => {
    if (!data.servers || pingedOnce) return
    pingedOnce = true
    untrack(refresh)
  })

  // Selection and the details panel
  let selected = $state(0)
  const sel = $derived(servers[selected] ?? servers[0] ?? null)
  const selIndex = $derived(sel ? servers.indexOf(sel) : -1)
  const selStatus = $derived(sel ? statuses[sel.address] : undefined)
  let asideTitle: HTMLElement | undefined = $state()

  async function details(i: number) {
    selected = i
    editing = false
    await tick()
    asideTitle?.focus()
  }

  // Saving servers.dat (the whole list, in this order)
  async function save(next: Server[]): Promise<boolean> {
    const before = data.servers
    data.servers = next
    try {
      await api.saveServers(id, $state.snapshot(next) as Server[])
      return true
    } catch (e) {
      data.servers = before
      app.error(e)
      return false
    }
  }

  function move(i: number, to: number) {
    if (to < 0 || to >= servers.length) return
    const next = [...servers]
    const [item] = next.splice(i, 1)
    next.splice(to, 0, item)
    selected = to
    void save(next)
  }

  // Add
  let address = $state('')
  let name = $state('')
  let addError = $state('')

  async function add(e: SubmitEvent) {
    e.preventDefault()
    const addr = address.trim()
    if (!addr || /\s/.test(addr)) {
      addError = t('Enter the server address, like play.example.net')
      return
    }
    addError = ''
    const server: Server = { name: name.trim() || addr, address: addr, iconUrl: null, acceptTextures: null }
    if (await save([...servers, server])) {
      address = ''
      name = ''
      selected = servers.length - 1
      void ping(addr)
      app.toast(t('{name} added.', { name: server.name }), 'success')
    }
  }

  // Edit
  let editing = $state(false)
  let editName = $state('')
  let editAddress = $state('')
  let editError = $state('')

  function startEdit() {
    if (!sel) return
    editName = sel.name
    editAddress = sel.address
    editError = ''
    editing = true
  }

  async function saveEdit(e: SubmitEvent) {
    e.preventDefault()
    if (!sel) return
    const addr = editAddress.trim()
    if (!addr || /\s/.test(addr)) {
      editError = t('Enter the server address, like play.example.net')
      return
    }
    const changed = addr !== sel.address
    const next = servers.map((s, i) => (i === selIndex ? { ...s, name: editName.trim() || addr, address: addr } : s))
    if (await save(next)) {
      editing = false
      if (changed) void ping(addr)
    }
  }

  function setTextures(v: string) {
    if (!sel) return
    const acceptTextures = v === 'accept' ? true : v === 'refuse' ? false : null
    void save(servers.map((s, i) => (i === selIndex ? { ...s, acceptTextures } : s)))
  }

  // Remove
  let removing = $state<number | null>(null)

  async function remove() {
    if (removing === null) return
    const at = removing
    if (await save(servers.filter((_, i) => i !== at))) {
      removing = null
      selected = Math.max(0, Math.min(at, servers.length - 1))
    }
  }

  // Join
  function join(s: Server) {
    void app.launch(id, false, s.address)
  }

  function onPlay() {
    if (running) void app.kill(id)
    else if (!preparing && sel) join(sel)
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text)
      app.toast(t('Address copied.'), 'success')
    } catch (e) {
      app.error(e)
    }
  }

  const textures = $derived(sel?.acceptTextures === true ? 'accept' : sel?.acceptTextures === false ? 'refuse' : 'ask')
  const versionDiffers = $derived(selStatus?.compatible != null ? selStatus.compatible === false : !!selStatus?.version && !selStatus.version.includes(instance.minecraft))
</script>

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Servers')}>
  <div class="toolbar">
    <div class="toolbar__title">
      <h2 class="text-heading">{data.servers ? plural(servers.length, 'server', 'servers') : t('Servers')}</h2>
    </div>
    <Button icon="update" busy={anyPending} disabled={!servers.length} onclick={refresh}>{t('Refresh')}</Button>
  </div>

  <form class="block" aria-labelledby="add-title" onsubmit={add} novalidate>
    <h3 id="add-title" class="text-label muted">{t('Add a server')}</h3>
    <div class="add">
      <div class="add__address">
        <TextField label={t('Server address')} icon="globe" mono placeholder="play.example.net" bind:value={address} error={addError || undefined} autocomplete="off" spellcheck={false} />
      </div>
      <div class="add__name">
        <TextField label={t('Name')} placeholder={t('Shown in your list')} bind:value={name} autocomplete="off" />
      </div>
      <Button type="submit" variant="primary" icon="plus">{t('Add server')}</Button>
    </div>
  </form>

  {#if data.servers === null}
    <p class="muted text-body">{t('Reading the server list…')}</p>
  {:else if !servers.length}
    <div class="empty">
      <p class="text-heading">{t('No servers yet')}</p>
      <p class="muted text-body">{t('Add one above. It also shows up in the game’s Multiplayer list.')}</p>
    </div>
  {:else}
    <section class="stack" aria-labelledby="saved-title">
      <div class="head">
        <h3 id="saved-title" class="text-label muted">{t('Saved servers')}</h3>
        <span class="text-counter muted">{num(servers.length)}</span>
      </div>
      {#each servers as s, i (i + s.address)}
        {@const st = statuses[s.address]}
        <div class={['srow', i === selIndex && 'is-selected']}>
          <div class="srow__main">
            <!-- Stretched over the whole row: a click anywhere on it, apart from Join, shows the server in the panel. -->
            <button type="button" class="srow__hit" aria-pressed={i === selIndex} aria-label={t('Details for {name}', { name: s.name })} onclick={() => details(i)}></button>
            {#if st}
              <ServerRow
                name={s.name}
                address={s.address}
                iconSrc={st.iconUrl ?? s.iconUrl ?? undefined}
                motd={st.motd}
                motdFormatted={st.motdFormatted}
                online={st.online}
                players={st.online ? { online: st.playersOnline, max: st.playersMax } : undefined}
                playersLabel={t('{n} of {n2} players online', { n: num(st.playersOnline), n2: num(st.playersMax) })}
                ping={st.pingMs ?? undefined}
                joinLabel={t('Join {name} with {name2}', { name: s.name, name2: instance.name })}
                onJoin={() => join(s)}
              />
            {:else}
              <div class="cb-server">
                <span class="cb-server__icon">{#if s.iconUrl}<img src={s.iconUrl} alt="" />{:else}<Icon name="servers" size={27} />{/if}</span>
                <div class="cb-server__body">
                  <div class="cb-server__head">
                    <span class="cb-server__name text-heading">{s.name}</span>
                    <span class="cb-server__address text-mono">{s.address}</span>
                  </div>
                  <p class="cb-server__motd text-mono">Checking the server…</p>
                </div>
                <div class="cb-server__join">
                  <Button icon="play" aria-label={t('Join {name} with {name2}', { name: s.name, name2: instance.name })} onclick={() => join(s)}>{t('Join')}</Button>
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </section>
  {/if}
</div>

<aside class="ip-aside" aria-label={t('Server details')}>
  <Inspector class="ip-aside__inner" danger={sel && !editing ? { label: t('Remove'), hint: t('Takes it off this instance’s server list.'), onclick: () => (removing = selIndex) } : null}>
    {#if sel}
      {@const icon = selStatus?.iconUrl ?? sel.iconUrl}
      {#snippet serverMedia()}
        {#if icon}
          <span class="cb-ins-media"><img src={icon} alt="" /></span>
        {:else}
          <span class="cb-ins-media"><Icon name="servers" size={27} /></span>
        {/if}
      {/snippet}
      {#snippet serverMeta()}
        {#if !selStatus}
          <InspectorStatus tone="off">{t('Checking…')}</InspectorStatus>
        {:else if selStatus.online}
          <InspectorStatus tone="ok">{t('Online')}</InspectorStatus>
          <MetaChip>{num(selStatus.playersOnline)} / {num(selStatus.playersMax)}</MetaChip>
          {#if selStatus.pingMs != null}<MetaChip>{num(selStatus.pingMs)} ms</MetaChip>{/if}
        {:else}
          <InspectorStatus tone="bad">{t('Can’t reach server')}</InspectorStatus>
        {/if}
      {/snippet}
      {#if editing}
        <InspectorHead eyebrow={t('Edit server')} title={sel.name} sub={sel.address} mono media={serverMedia} />
        <form class="edit" onsubmit={saveEdit} novalidate>
          <TextField label={t('Name')} bind:value={editName} autocomplete="off" />
          <TextField label={t('Server address')} mono icon="globe" bind:value={editAddress} error={editError || undefined} autocomplete="off" spellcheck={false} />
          <div class="cb-ins-acts">
            <Button type="submit" variant="primary" icon="check">{t('Save')}</Button>
            <Button onclick={() => (editing = false)}>{t('Cancel')}</Button>
          </div>
        </form>
      {:else}
        <InspectorHead eyebrow={t('Server')} title={sel.name} sub={sel.address} mono media={serverMedia} meta={serverMeta} bind:titleEl={asideTitle} />
        <div class="cb-ins-acts">
          <PlayButton
            state={running ? 'running' : preparing ? 'installing' : 'ready'}
            label={running ? undefined : preparing ? undefined : t('Join')}
            detail={running ? t('The game is running') : t('With {name}', { name: instance.name })}
            aria-label={t('Join {name} with {name2}', { name: sel.name, name2: instance.name })}
            onclick={onPlay}
          />
          <Button icon="copy" aria-label={t('Copy address')} onclick={() => sel && copy(sel.address)}>{t('Copy')}</Button>
          <Button icon="wrench" onclick={startEdit}>{t('Edit')}</Button>
          {#if servers.length > 1}
            <Button icon="chevron-down" class="flip" disabled={selIndex === 0} onclick={() => move(selIndex, selIndex - 1)}>{t('Move up')}</Button>
            <Button icon="chevron-down" disabled={selIndex === servers.length - 1} onclick={() => move(selIndex, selIndex + 1)}>{t('Move down')}</Button>
          {/if}
        </div>
        {#if versionDiffers}
          <InspectorNotice>
            <strong>{t('{name} may not get in.', { name: instance.name })}</strong> {t('The server runs {version}, and this instance uses {minecraft}.', { version: selStatus?.version ?? '?', minecraft: instance.minecraft })}
          </InspectorNotice>
        {/if}
        {#if selStatus?.online && selStatus.motd}
          <InspectorSection title={t('Message of the day')}>
            <Motd text={selStatus.motdFormatted || selStatus.motd} />
          </InspectorSection>
        {/if}
        <InspectorSection title={t('Server resource packs')}>
          <SegmentedControl
            label={t('Server resource packs')}
            options={[
              { value: 'ask', label: t('Ask') },
              { value: 'accept', label: t('Accept') },
              { value: 'refuse', label: t('Refuse') },
            ]}
            value={textures}
            onChange={setTextures}
          />
          <p class="cb-ins-hint">{t('What the game does when this server sends its own resource pack.')}</p>
        </InspectorSection>
        <InspectorSection title={t('Details')}>
          <dl class="cb-ins-facts">
            {#if selStatus?.version}<dt>{t('Version')}</dt><dd><span class="text-mono" title={selStatus.version}>{selStatus.version}</span></dd>{/if}
            {#if selStatus?.online}<dt>{t('Fit')}</dt><dd>{#if versionDiffers}<InspectorStatus tone="warn">{t('May not get in')}</InspectorStatus>{:else}<InspectorStatus tone="ok">{t('Can join')}</InspectorStatus>{/if}</dd>{/if}
            <dt>{t('Position')}</dt><dd><span>{t('{n} of {n2}', { n: num(selIndex + 1), n2: num(servers.length) })}</span></dd>
          </dl>
        </InspectorSection>
      {/if}
    {:else}
      {#snippet noServerMedia()}<span class="cb-ins-media cb-ins-media--empty"><Icon name="servers" size={27} /></span>{/snippet}
      <InspectorHead eyebrow={t('Server')} title={t('No servers yet')} sub={instance.name} media={noServerMedia} empty />
      <p class="cb-ins-hint">{t('Add a server to see who’s online and join it in one click.')}</p>
    {/if}
  </Inspector>
</aside>

{#if removing !== null && servers[removing]}
  {@const s = servers[removing]}
  {#snippet removeActions()}
    <Button variant="ghost" onclick={() => (removing = null)}>{t('Cancel')}</Button>
    <Button variant="danger" icon="trash" onclick={remove}>{t('Remove')}</Button>
  {/snippet}
  <Dialog tone="danger" icon="trash" title={t('Remove {name}?', { name: s.name })} onClose={() => (removing = null)} actions={removeActions}>
    <p>{t('Takes')} <span class="text-mono">{s.address}</span> {t('off the server list of this instance. You can add it again any time.')}</p>
  </Dialog>
{/if}

<style>
  p,
  h2,
  h3,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .toolbar__title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .add {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-3) var(--space-4);
  }
  .add__address {
    flex: 2 1 240px;
    min-width: 0;
  }
  .add__name {
    flex: 1 1 160px;
    min-width: 0;
  }
  /* line the button up with the fields, below their labels */
  .add > :global(.cb-btn) {
    margin-top: 28px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }
  .srow {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .srow__main {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .srow__hit {
    all: unset;
    position: absolute;
    inset: 0;
    cursor: pointer;
  }
  .srow__hit:focus-visible {
    box-shadow: var(--focus-ring);
  }
  /* the row's content sits above the stretched button, and only its own buttons take clicks */
  .srow__main > :global(.cb-server),
  .srow__main > :global(div.cb-server) {
    position: relative;
    pointer-events: none;
  }
  .srow__main :global(.cb-server button) {
    pointer-events: auto;
  }
  .srow__main:hover :global(.cb-server) {
    background: var(--surface-hover);
  }
  .srow.is-selected .srow__main::after {
    content: '';
    position: absolute;
    inset: 0;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
  }
  .edit {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
</style>
