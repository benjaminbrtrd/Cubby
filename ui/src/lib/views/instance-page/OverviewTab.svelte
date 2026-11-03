<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Icon, IconButton, PlayButton, ProgressBar, ServerRow } from '../../cubby'
  import { bytes, date, megabytes, num, plural, relativeTime } from '../../format'
  import { app, type InstanceTab } from '../../state.svelte'
  import type { Instance, ServerStatus } from '../../types'
  import { baseFile, gigabytes, modName, when, type InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  const go = (tab: InstanceTab) => app.openInstance(id, tab)
  const running = $derived(!!app.running[id])
  const preparing = $derived(!!app.instanceTask(id))

  untrack(() => {
    if (!data.backups) void data.loadBackups()
  })

  // Continue
  const world = $derived(data.lastWorld ?? data.worlds?.[0] ?? null)

  // Stats
  const playStat = $derived.by(() => {
    const h = Math.floor(instance.totalPlayedSecs / 3600)
    return h >= 1 ? { value: num(h), unit: 'h' } : { value: num(Math.floor(instance.totalPlayedSecs / 60)), unit: 'min' }
  })
  const stats = $derived([
    { label: 'Play time', ...playStat },
    { label: 'Worlds', value: data.worlds ? num(data.worlds.length) : '—', unit: '' },
    { label: 'Mods on', value: data.mods ? num(data.mods.filter((m) => m.enabled).length) : '—', unit: '' },
    { label: 'Screenshots', value: data.screenshots ? num(data.screenshots.length) : '—', unit: '' },
  ])

  // Health: only what Cubby can check from the files and settings it reads
  type Check = { ok: boolean; text: string; action?: { label: string; run: () => void } }
  const checks = $derived.by<Check[]>(() => {
    const list: Check[] = []
    if (!instance.installed) list.push({ ok: false, text: 'The game isn’t downloaded yet. Play downloads it first.', action: { label: 'Download now', run: () => void data.repair() } })
    else list.push({ ok: true, text: 'Game files downloaded' })
    if (!app.defaultAccount) list.push({ ok: false, text: 'No Microsoft account yet. Play starts the demo; sign in for the full game.', action: { label: 'Sign in', run: () => app.navigate({ name: 'accounts' }) } })
    const mods = data.mods
    if (mods && mods.length) {
      const on = mods.filter((m) => m.enabled).length
      if (instance.loader.kind === 'vanilla') {
        list.push({ ok: false, text: `${plural(mods.length, 'mod doesn’t', 'mods don’t')} load: this instance has no mod loader.`, action: { label: 'Change loader', run: () => go('settings') } })
      } else if (data.twins.size) {
        const first = mods.find((m) => data.twins.has(m.fileName))
        const names = new Set(mods.filter((m) => data.twins.has(m.fileName)).map((m) => modName(m).toLowerCase()))
        list.push({ ok: false, text: names.size === 1 && first ? `${modName(first)} is installed twice.` : `${num(names.size)} mods are installed twice.`, action: { label: 'See in Mods', run: () => go('mods') } })
      } else {
        list.push({ ok: true, text: `${plural(on, 'mod', 'mods')} on` })
      }
    }
    const shaderOn = data.shaderPacks?.some((p) => p.enabled)
    const sm = data.shaderMods
    if (shaderOn && mods && !(sm.iris?.enabled || sm.optifine?.enabled))
      list.push({ ok: false, text: 'A shader pack is on, but Iris isn’t installed or is turned off.', action: { label: 'See in Shader packs', run: () => go('shaders') } })
    const max = instance.javaOverrides.maxMemoryMb ?? app.settings?.java.maxMemoryMb
    const total = app.info?.totalMemoryMb
    if (max && total) {
      if (max > total * 0.75) list.push({ ok: false, text: `Memory is set to ${gigabytes(max)}, most of your ${gigabytes(total)}.`, action: { label: 'Change', run: () => go('settings') } })
      else list.push({ ok: true, text: `Memory ${gigabytes(max)} of ${gigabytes(total)}` })
    }
    return list
  })
  const problems = $derived(checks.filter((c) => !c.ok))
  const fine = $derived(checks.filter((c) => c.ok))

  // Updates
  const updates = $derived(data.updates)
  const updateTask = $derived(app.tasks[`update:${id}`])
  /** What an updated file is, and the tab where it lives. */
  function kindOf(fileName: string): { label: string; tab: InstanceTab } {
    const base = baseFile(fileName)
    if (data.shaderPacks?.some((p) => p.fileName === base)) return { label: 'Shader pack', tab: 'shaders' }
    if (data.resourcePacks?.some((p) => p.fileName === base)) return { label: 'Resource pack', tab: 'resourcepacks' }
    return { label: 'Mod', tab: 'mods' }
  }

  // Servers: ping the first one
  const server = $derived(data.servers?.[0] ?? null)
  let status = $state<ServerStatus | null>(null)
  let pinged = ''
  $effect(() => {
    const address = server?.address
    if (!address || address === pinged) return
    pinged = address
    status = null
    api
      .pingServer(address)
      .then((s) => {
        if (pinged === address) status = s
      })
      .catch(() => {
        if (pinged === address) status = { online: false, motd: '', playersOnline: 0, playersMax: 0, version: null, pingMs: null, iconUrl: null }
      })
  })

  // Backups
  const backups = $derived(data.backups)
  const backupBytes = $derived((backups ?? []).reduce((s, b) => s + b.sizeBytes, 0))
  let backingUp = $state(false)

  async function backUp() {
    if (!world || backingUp) return
    backingUp = true
    try {
      await api.backupWorld(id, world.folder)
      await data.loadBackups()
      app.toast(`${world.name} is backed up.`, 'success')
    } catch (e) {
      app.error(e)
    } finally {
      backingUp = false
    }
  }

  // Notes: saved shortly after typing stops, and when leaving the page
  let notes = $state(untrack(() => instance.notes ?? ''))
  let timer: ReturnType<typeof setTimeout> | undefined

  function saveNotes() {
    clearTimeout(timer)
    timer = undefined
    const current = app.instance(id)
    if (!current) return
    const next = notes.trim() ? notes : null
    if ((current.notes ?? null) === next) return
    void app.updateInstance({ ...$state.snapshot(current), notes: next })
  }

  function typed() {
    clearTimeout(timer)
    timer = setTimeout(saveNotes, 600)
  }

  onDestroy(() => {
    if (timer) saveNotes()
  })

  // Details
  const javaPath = $derived(instance.javaOverrides.executable)
  const memory = $derived(instance.javaOverrides.maxMemoryMb ?? app.settings?.java.maxMemoryMb ?? null)
  const folder = $derived.by(() => {
    const root = app.info?.dataDir
    if (!root) return null
    const sep = root.includes('\\') ? '\\' : '/'
    return [root.replace(/[\\/]+$/, ''), 'instances', id, '.minecraft'].join(sep)
  })
</script>

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label="Overview">
<div class="overview">
  <div class="col">
    <section class="block continue" aria-labelledby="ov-continue">
      {#if world?.iconUrl}
        <img class="continue__art" src={world.iconUrl} alt="" width="90" height="90" />
      {:else}
        <span class="continue__art continue__art--empty" aria-hidden="true"><Icon name="cube" size={27} /></span>
      {/if}
      <div class="continue__text">
        <h2 id="ov-continue" class="text-label muted">Continue</h2>
        {#if data.worlds === null}
          <span class="muted text-small">Looking for worlds…</span>
        {:else if world}
          <span class="text-heading clip" title={world.name}>{world.name}</span>
          <span class="muted text-small">{world.lastPlayed ? `Last played ${relativeTime(world.lastPlayed).toLowerCase()}` : 'Not played yet'} · {bytes(world.sizeBytes)}</span>
        {:else}
          <span class="text-heading">No worlds yet</span>
          <span class="muted text-small">Play, then create one in Singleplayer.</span>
        {/if}
        <div class="row">
          <PlayButton
            size="sm"
            state={running ? 'running' : preparing ? 'installing' : 'ready'}
            onclick={() => (running ? app.kill(id) : preparing ? undefined : app.launch(id))}
          />
          {#if data.worlds?.length}
            <button type="button" class="link text-body-strong" onclick={() => go('worlds')}>{data.worlds.length === 1 ? 'See the world' : `All ${num(data.worlds.length)} worlds`}</button>
          {/if}
        </div>
        {#if world}<p class="muted text-small note">The game opens on its title screen. Pick {world.name} in Singleplayer.</p>{/if}
      </div>
    </section>

    <section class="block" aria-labelledby="ov-health">
      <div class="block__head">
        <h2 id="ov-health" class="text-label muted">Health</h2>
        {#if problems.length}
          <span class="text-body-strong warn">{plural(problems.length, 'thing', 'things')} to look at</span>
        {:else}
          <span class="text-body-strong ok">Nothing to look at</span>
        {/if}
      </div>
      {#each problems as c (c.text)}
        <div class="line">
          <span class="warn icon"><Icon name="warning" size={18} label="Warning" /></span>
          <span class="text-body grow">{c.text}</span>
          {#if c.action}<button type="button" class="link text-body-strong" onclick={c.action.run}>{c.action.label}</button>{/if}
        </div>
      {/each}
      {#if fine.length}
        <div class="checks">
          {#each fine as c (c.text)}
            <span class="text-small check"><span class="ok icon"><Icon name="check" size={18} label="OK" /></span>{c.text}</span>
          {/each}
        </div>
      {/if}
    </section>

    <section class="stack" aria-labelledby="ov-servers">
      <div class="block__head">
        <h2 id="ov-servers" class="text-label muted">Servers</h2>
        <span class="grow"></span>
        {#if data.servers?.length}
          <button type="button" class="link text-body-strong" onclick={() => go('servers')}>{data.servers.length === 1 ? 'Manage' : `All ${num(data.servers.length)} servers`}</button>
        {/if}
      </div>
      {#if data.servers === null}
        <p class="muted text-small">Loading servers…</p>
      {:else if !server}
        <div class="block empty">
          <span class="text-body">No servers saved for this instance.</span>
          <Button size="sm" icon="plus" onclick={() => go('servers')}>Add a server</Button>
        </div>
      {:else if !status}
        <div class="block empty"><span class="text-body">Checking {server.name}…</span></div>
      {:else}
        <ServerRow
          name={server.name}
          address={server.address}
          iconSrc={status.iconUrl ?? server.iconUrl ?? undefined}
          motd={status.motd}
          online={status.online}
          players={status.online ? { online: status.playersOnline, max: status.playersMax } : undefined}
          playersLabel="players online"
          ping={status.pingMs ?? undefined}
          joinLabel={`Join ${server.name} with ${instance.name}`}
          onJoin={() => app.launch(id, false, server.address)}
        />
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-details">
      <h2 id="ov-details" class="text-label muted">Details</h2>
      <dl class="details">
        <dt class="text-small muted">Java</dt>
        {#if javaPath}
          <dd class="text-mono wrap" title={javaPath}>{javaPath}</dd>
        {:else}
          <dd class="text-body">Picked automatically</dd>
        {/if}
        <dt class="text-small muted">Memory</dt>
        <dd class="text-body">{memory ? megabytes(memory) : '—'}{instance.javaOverrides.maxMemoryMb == null ? ' (global)' : ''}</dd>
        <dt class="text-small muted">Created</dt>
        <dd class="text-body">{date(instance.created)}</dd>
        <dt class="text-small muted">Account</dt>
        <dd class="text-body">{app.defaultAccount?.username ?? 'None yet'}</dd>
        {#if instance.modpack}
          <dt class="text-small muted">Modpack</dt>
          <dd class="text-body span">
            <button type="button" class="link text-body" onclick={() => instance.modpack && data.project(instance.modpack.projectId, 'modpack')}>{instance.modpack.title}</button>
            <span class="text-mono muted">{instance.modpack.versionNumber}</span>
          </dd>
        {/if}
        <dt class="text-small muted">Folder</dt>
        <dd class="span folder">
          {#if folder}<span class="text-mono wrap grow">{folder}</span>{:else}<span class="text-body grow">The instance’s game folder</span>{/if}
          <IconButton icon="folder" size="sm" variant="secondary" label="Open folder" onclick={() => data.openFolder()} />
        </dd>
      </dl>
    </section>
  </div>

  <div class="col">
    <section class="block stats" aria-label="Stats">
      {#each stats as s (s.label)}
        <div class="stat">
          <span class="text-label muted">{s.label}</span>
          <span><span class="text-stat">{s.value}</span>{#if s.unit}<span class="text-heading unit">{s.unit}</span>{/if}</span>
        </div>
      {/each}
    </section>

    <section class="block" aria-labelledby="ov-updates">
      <div class="block__head">
        <h2 id="ov-updates" class="text-label muted">Updates</h2>
        {#if updates?.length}<span class="text-small info">{num(updates.length)} ready</span>{/if}
        <span class="grow"></span>
        {#if updates?.length}
          <Button size="sm" icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={() => data.applyUpdates(updates.map((u) => u.fileName))}>Update all</Button>
        {:else if updates && !data.checkingUpdates}
          <Button size="sm" variant="ghost" icon="update" onclick={() => data.checkUpdates(true)}>Check again</Button>
        {/if}
      </div>
      {#if updateTask}
        <ProgressBar tone="diamond" label={updateTask.label} value={updateTask.total ? (updateTask.done / updateTask.total) * 100 : undefined} />
      {:else if data.checkingUpdates}
        <p class="muted text-small">Checking Modrinth…</p>
      {:else if !updates}
        <div class="line">
          <span class="text-body grow">Not checked yet.</span>
          <Button size="sm" icon="update" onclick={() => data.checkUpdates()}>Check for updates</Button>
        </div>
      {:else if !updates.length}
        <p class="line text-body"><span class="ok icon"><Icon name="check" size={18} /></span>Everything Cubby found on Modrinth is up to date.</p>
      {:else}
        {#each updates.slice(0, 6) as u (u.fileName)}
          {@const k = kindOf(u.fileName)}
          <div class="line ruled">
            <span class="info icon"><Icon name="update" size={18} /></span>
            <span class="text-body-strong grow clip" title={u.title}>{u.title}</span>
            <button type="button" class="link text-small" onclick={() => go(k.tab)}>{k.label}</button>
            <span class="text-mono muted">{u.currentVersion}</span>
            <span class="muted icon" aria-hidden="true"><Icon name="chevron-right" size={18} /></span>
            <span class="cb-visually-hidden">to</span>
            <span class="text-mono">{u.latestVersion}</span>
          </div>
        {/each}
        {#if updates.length > 6}
          <button type="button" class="link text-body-strong" onclick={() => go('mods')}>See all {num(updates.length)} in Mods</button>
        {/if}
        <p class="text-small muted line"><Icon name="info" size={18} />Cubby backs up the old files before updating.</p>
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-shots">
      <div class="block__head">
        <h2 id="ov-shots" class="text-label muted">Recent screenshots</h2>
        <span class="grow"></span>
        {#if data.screenshots?.length}
          <button type="button" class="link text-body-strong" onclick={() => go('screenshots')}>All {num(data.screenshots.length)}</button>
        {/if}
      </div>
      {#if data.screenshots === null}
        <p class="muted text-small">Loading screenshots…</p>
      {:else if !data.screenshots.length}
        <p class="muted text-small">No screenshots yet. Press F2 in game to take one.</p>
      {:else}
        <div class="shots">
          {#each data.screenshots.slice(0, 4) as s (s.fileName)}
            <button type="button" class="shot" aria-label={`Screenshot from ${when(s.taken, true)}`} onclick={() => go('screenshots')}>
              <img src={s.url} alt="" loading="lazy" />
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-backups">
      <div class="block__head">
        <h2 id="ov-backups" class="text-label muted">World backups</h2>
        <span class="grow"></span>
        {#if data.worlds?.length}<button type="button" class="link text-body-strong" onclick={() => go('worlds')}>Manage in Worlds</button>{/if}
      </div>
      {#if backups === null}
        <p class="muted text-small">Loading backups…</p>
      {:else if !backups.length}
        <p class="text-body">No backups yet.</p>
      {:else}
        <p class="text-body">Last backup {when(backups[0].created, true)} · {plural(backups.length, 'backup', 'backups')} · {bytes(backupBytes)}</p>
      {/if}
      {#if world}
        <div class="row">
          <Button size="sm" icon="copy" busy={backingUp} disabled={backingUp} onclick={backUp}>Back up {world.name}</Button>
        </div>
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-notes">
      <div class="block__head">
        <label id="ov-notes" for="ov-notes-text" class="text-label muted">Notes</label>
        <span class="text-small muted">Only you see these.</span>
      </div>
      <textarea id="ov-notes-text" class="notes text-body" rows="3" placeholder="Coordinates, seeds, what to build next" bind:value={notes} oninput={typed} onblur={saveNotes}></textarea>
    </section>
  </div>
</div>
</div>

<style>
  .overview {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: var(--space-4);
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: var(--space-3) 14px;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .block__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    min-height: 28px;
  }
  h2,
  p,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .ok {
    color: var(--grass-text);
  }
  .warn {
    color: var(--gold-text);
  }
  .info {
    color: var(--diamond-text);
  }
  .icon {
    display: inline-flex;
    flex: none;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .clip {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .wrap {
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-1);
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
  }
  .line.ruled {
    border-top: var(--border-hair) solid var(--line);
  }
  .link {
    padding: 0;
    color: var(--grass-text);
    background: none;
    border: 0;
    text-align: start;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }

  .continue {
    flex-direction: row;
    gap: var(--space-4);
  }
  .continue__art {
    display: block;
    flex: none;
    width: 90px;
    height: 90px;
    object-fit: cover;
    image-rendering: pixelated;
    border: var(--border-block) solid var(--edge);
  }
  .continue__art--empty {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-muted);
    background: var(--field);
  }
  .continue__text {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-1);
    min-width: 0;
  }
  .note {
    margin-top: var(--space-1);
  }

  .checks {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--space-4);
    padding-top: 6px;
    border-top: var(--border-hair) solid var(--line);
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .empty {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    box-shadow: var(--shadow-block-sm);
  }

  .details {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 6px var(--space-4);
  }
  .details .span {
    grid-column: span 3;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .details .folder {
    flex-wrap: nowrap;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-3);
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .unit {
    margin-inline-start: var(--space-1);
  }

  .shots {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
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
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block);
  }
  .shot:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .shot img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
  }

  .notes {
    box-sizing: border-box;
    width: 100%;
    margin: 0;
    padding: 6px 10px;
    resize: vertical;
    color: var(--ink);
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    border-radius: 0;
    box-shadow: var(--shadow-well);
  }
  .notes::placeholder {
    color: var(--ink-muted);
  }
  .notes:focus-visible {
    outline: none;
    box-shadow: var(--shadow-well), var(--focus-ring);
  }
</style>
