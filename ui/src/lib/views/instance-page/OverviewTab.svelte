<script lang="ts">
  import { t } from '../../i18n.svelte'
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
    { label: t('Play time'), ...playStat },
    { label: t('Worlds'), value: data.worlds ? num(data.worlds.length) : '—', unit: '' },
    { label: t('Mods on'), value: data.mods ? num(data.mods.filter((m) => m.enabled).length) : '—', unit: '' },
    { label: t('Screenshots'), value: data.screenshots ? num(data.screenshots.length) : '—', unit: '' },
  ])

  // Health: only what Cubby can check from the files and settings it reads
  type Check = { ok: boolean; text: string; action?: { label: string; run: () => void } }
  const checks = $derived.by<Check[]>(() => {
    const list: Check[] = []
    if (!instance.installed) list.push({ ok: false, text: t('The game isn’t downloaded yet. Play downloads it first.'), action: { label: t('Download now'), run: () => void data.repair() } })
    else list.push({ ok: true, text: t('Game files downloaded') })
    if (app.demoOnly) list.push({ ok: false, text: t('No Microsoft account yet. Play starts the demo; sign in for the full game.'), action: { label: t('Sign in'), run: () => app.navigate({ name: 'accounts' }) } })
    else if (!app.defaultAccount) list.push({ ok: true, text: t('Development build: plays the full game offline, without an account') })
    const mods = data.mods
    if (mods && mods.length) {
      const on = mods.filter((m) => m.enabled).length
      if (instance.loader.kind === 'vanilla') {
        list.push({ ok: false, text: t('{n} load: this instance has no mod loader.', { n: plural(mods.length, 'mod doesn’t', 'mods don’t') }), action: { label: t('Change loader'), run: () => go('settings') } })
      } else if (data.twins.size) {
        const first = mods.find((m) => data.twins.has(m.fileName))
        const names = new Set(mods.filter((m) => data.twins.has(m.fileName)).map((m) => modName(m).toLowerCase()))
        list.push({ ok: false, text: names.size === 1 && first ? t('{first} is installed twice.', { first: modName(first) }) : t('{n} mods are installed twice.', { n: num(names.size) }), action: { label: t('See in Mods'), run: () => go('mods') } })
      } else {
        list.push({ ok: true, text: `${plural(on, 'mod', 'mods')} on` })
      }
    }
    const shaderOn = data.shaderPacks?.some((p) => p.enabled)
    const sm = data.shaderMods
    if (shaderOn && mods && !(sm.iris?.enabled || sm.optifine?.enabled))
      list.push({ ok: false, text: t('A shader pack is on, but Iris isn’t installed or is turned off.'), action: { label: t('See in Shader packs'), run: () => go('shaders') } })
    const max = instance.javaOverrides.maxMemoryMb ?? app.settings?.java.maxMemoryMb
    const total = app.info?.totalMemoryMb
    if (max && total) {
      if (max > total * 0.75) list.push({ ok: false, text: t('Memory is set to {n}, most of your {n2}.', { n: gigabytes(max), n2: gigabytes(total) }), action: { label: t('Change'), run: () => go('settings') } })
      else list.push({ ok: true, text: t('Memory {n} of {n2}', { n: gigabytes(max), n2: gigabytes(total) }) })
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
    if (data.shaderPacks?.some((p) => p.fileName === base)) return { label: t('Shader pack'), tab: 'shaders' }
    if (data.resourcePacks?.some((p) => p.fileName === base)) return { label: t('Resource pack'), tab: 'resourcepacks' }
    return { label: t('Mod'), tab: 'mods' }
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
      app.toast(t('{name} is backed up.', { name: world.name }), 'success')
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

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Overview')}>
<div class="overview">
  <div class="col">
    <section class="block continue" aria-labelledby="ov-continue">
      {#if world?.iconUrl}
        <img class="continue__art" src={world.iconUrl} alt="" width="90" height="90" />
      {:else}
        <span class="continue__art continue__art--empty" aria-hidden="true"><Icon name="cube" size={27} /></span>
      {/if}
      <div class="continue__text">
        <h2 id="ov-continue" class="text-label muted">{t('Continue')}</h2>
        {#if data.worlds === null}
          <span class="muted text-small">{t('Looking for worlds…')}</span>
        {:else if world}
          <span class="text-heading clip" title={world.name}>{world.name}</span>
          <span class="muted text-small">{world.lastPlayed ? t('Last played {toLowerCase}', { toLowerCase: relativeTime(world.lastPlayed).toLowerCase() }) : t('Not played yet')} · {bytes(world.sizeBytes)}</span>
        {:else}
          <span class="text-heading">{t('No worlds yet')}</span>
          <span class="muted text-small">{t('Play, then create one in Singleplayer.')}</span>
        {/if}
        <div class="row">
          <PlayButton
            size="sm"
            state={running ? 'running' : preparing ? 'installing' : 'ready'}
            onclick={() => (running ? app.kill(id) : preparing ? undefined : app.launch(id))}
          />
          {#if data.worlds?.length}
            <button type="button" class="link text-body-strong" onclick={() => go('worlds')}>{data.worlds.length === 1 ? t('See the world') : t('All {n} worlds', { n: num(data.worlds.length) })}</button>
          {/if}
        </div>
      </div>
    </section>

    <section class="block" aria-labelledby="ov-health">
      <div class="block__head">
        <h2 id="ov-health" class="text-label muted">{t('Health')}</h2>
        {#if problems.length}
          <span class="text-body-strong warn">{t('{n} to look at', { n: plural(problems.length, 'thing', 'things') })}</span>
        {:else}
          <span class="text-body-strong ok">{t('Nothing to look at')}</span>
        {/if}
      </div>
      {#each problems as c (c.text)}
        <div class="line">
          <span class="warn icon"><Icon name="warning" size={18} label={t('Warning')} /></span>
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
        <h2 id="ov-servers" class="text-label muted">{t('Servers')}</h2>
        <span class="grow"></span>
        {#if data.servers?.length}
          <button type="button" class="link text-body-strong" onclick={() => go('servers')}>{data.servers.length === 1 ? 'Manage' : t('All {n} servers', { n: num(data.servers.length) })}</button>
        {/if}
      </div>
      {#if data.servers === null}
        <p class="muted text-small">{t('Loading servers…')}</p>
      {:else if !server}
        <div class="block empty">
          <span class="text-body">{t('No servers saved for this instance.')}</span>
          <Button size="sm" icon="plus" onclick={() => go('servers')}>{t('Add a server')}</Button>
        </div>
      {:else if !status}
        <div class="block empty"><span class="text-body">{t('Checking {name}…', { name: server.name })}</span></div>
      {:else}
        <ServerRow
          name={server.name}
          address={server.address}
          iconSrc={status.iconUrl ?? server.iconUrl ?? undefined}
          motd={status.motd}
          online={status.online}
          players={status.online ? { online: status.playersOnline, max: status.playersMax } : undefined}
          playersLabel={t('players online')}
          ping={status.pingMs ?? undefined}
          joinLabel={t('Join {name} with {name2}', { name: server.name, name2: instance.name })}
          onJoin={() => app.launch(id, false, server.address)}
        />
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-details">
      <h2 id="ov-details" class="text-label muted">{t('Details')}</h2>
      <dl class="details">
        <dt class="text-small muted">Java</dt>
        {#if javaPath}
          <dd class="text-mono wrap" title={javaPath}>{javaPath}</dd>
        {:else}
          <dd class="text-body">{t('Picked automatically')}</dd>
        {/if}
        <dt class="text-small muted">{t('Memory')}</dt>
        <dd class="text-body">{memory ? megabytes(memory) : '—'}{instance.javaOverrides.maxMemoryMb == null ? t(' (global)') : ''}</dd>
        <dt class="text-small muted">{t('Created')}</dt>
        <dd class="text-body">{date(instance.created)}</dd>
        <dt class="text-small muted">{t('Account')}</dt>
        <dd class="text-body">{app.defaultAccount?.username ?? t('None yet')}</dd>
        {#if instance.modpack}
          <dt class="text-small muted">{t('Modpack')}</dt>
          <dd class="text-body span">
            <button type="button" class="link text-body" onclick={() => instance.modpack && data.project(instance.modpack.projectId, 'modpack')}>{instance.modpack.title}</button>
            <span class="text-mono muted">{instance.modpack.versionNumber}</span>
          </dd>
        {/if}
        <dt class="text-small muted">{t('Folder')}</dt>
        <dd class="span folder">
          {#if folder}<span class="text-mono wrap grow">{folder}</span>{:else}<span class="text-body grow">{t('The instance’s game folder')}</span>{/if}
          <IconButton icon="folder" size="sm" variant="secondary" label={t('Open folder')} onclick={() => data.openFolder()} />
        </dd>
      </dl>
    </section>
  </div>

  <div class="col">
    <section class="block stats" aria-label={t('Stats')}>
      {#each stats as s (s.label)}
        <div class="stat">
          <span class="text-label muted">{s.label}</span>
          <span><span class="text-stat">{s.value}</span>{#if s.unit}<span class="text-heading unit">{s.unit}</span>{/if}</span>
        </div>
      {/each}
    </section>

    <section class="block" aria-labelledby="ov-updates">
      <div class="block__head">
        <h2 id="ov-updates" class="text-label muted">{t('Updates')}</h2>
        {#if updates?.length}<span class="text-small info">{t('{n} ready', { n: num(updates.length) })}</span>{/if}
        <span class="grow"></span>
        {#if updates?.length}
          <Button size="sm" icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={() => data.applyUpdates(updates.map((u) => u.fileName))}>{t('Update all')}</Button>
        {:else if updates && !data.checkingUpdates}
          <Button size="sm" variant="ghost" icon="update" onclick={() => data.checkUpdates(true)}>{t('Check again')}</Button>
        {/if}
      </div>
      {#if updateTask}
        <ProgressBar tone="diamond" label={updateTask.label} value={updateTask.total ? (updateTask.done / updateTask.total) * 100 : undefined} />
      {:else if data.checkingUpdates}
        <p class="muted text-small">{t('Checking for updates…')}</p>
      {:else if !updates}
        <div class="line">
          <span class="text-body grow">{t('Not checked yet.')}</span>
          <Button size="sm" icon="update" onclick={() => data.checkUpdates()}>{t('Check for updates')}</Button>
        </div>
      {:else if !updates.length}
        <p class="line text-body"><span class="ok icon"><Icon name="check" size={18} /></span>{t('Everything is up to date.')}</p>
      {:else}
        {#each updates.slice(0, 6) as u (u.fileName)}
          {@const k = kindOf(u.fileName)}
          <div class="line ruled">
            <span class="info icon"><Icon name="update" size={18} /></span>
            <span class="text-body-strong grow clip" title={u.title}>{u.title}</span>
            <button type="button" class="link text-small" onclick={() => go(k.tab)}>{k.label}</button>
            <span class="text-mono muted">{u.currentVersion}</span>
            <span class="muted icon" aria-hidden="true"><Icon name="chevron-right" size={18} /></span>
            <span class="cb-visually-hidden">{t('to')}</span>
            <span class="text-mono">{u.latestVersion}</span>
          </div>
        {/each}
        {#if updates.length > 6}
          <button type="button" class="link text-body-strong" onclick={() => go('mods')}>{t('See all {n} in Mods', { n: num(updates.length) })}</button>
        {/if}
        <p class="text-small muted line"><Icon name="info" size={18} />{t('Cubby backs up the old files before updating.')}</p>
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-shots">
      <div class="block__head">
        <h2 id="ov-shots" class="text-label muted">{t('Recent screenshots')}</h2>
        <span class="grow"></span>
        {#if data.screenshots?.length}
          <button type="button" class="link text-body-strong" onclick={() => go('screenshots')}>{t('All {n}', { n: num(data.screenshots.length) })}</button>
        {/if}
      </div>
      {#if data.screenshots === null}
        <p class="muted text-small">{t('Loading screenshots…')}</p>
      {:else if !data.screenshots.length}
        <p class="muted text-small">{t('No screenshots yet. Press F2 in game to take one.')}</p>
      {:else}
        <div class="shots">
          {#each data.screenshots.slice(0, 4) as s (s.fileName)}
            <button type="button" class="shot" aria-label={t('Screenshot from {true}', { true: when(s.taken, true) })} onclick={() => go('screenshots')}>
              <img src={s.url} alt="" loading="lazy" />
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-backups">
      <div class="block__head">
        <h2 id="ov-backups" class="text-label muted">{t('World backups')}</h2>
        <span class="grow"></span>
        {#if data.worlds?.length}<button type="button" class="link text-body-strong" onclick={() => go('worlds')}>{t('Manage in Worlds')}</button>{/if}
      </div>
      {#if backups === null}
        <p class="muted text-small">{t('Loading backups…')}</p>
      {:else if !backups.length}
        <p class="text-body">{t('No backups yet.')}</p>
      {:else}
        <p class="text-body">{t('Last backup {true} · {n} · {n2}', { true: when(backups[0].created, true), n: plural(backups.length, 'backup', 'backups'), n2: bytes(backupBytes) })}</p>
      {/if}
      {#if world}
        <div class="row">
          <Button size="sm" icon="copy" busy={backingUp} disabled={backingUp} onclick={backUp}>{t('Back up {name}', { name: world.name })}</Button>
        </div>
      {/if}
    </section>

    <section class="block" aria-labelledby="ov-notes">
      <div class="block__head">
        <label id="ov-notes" for="ov-notes-text" class="text-label muted">{t('Notes')}</label>
        <span class="text-small muted">{t('Only you see these.')}</span>
      </div>
      <textarea id="ov-notes-text" class="notes text-body" rows="3" placeholder={t('Coordinates, seeds, what to build next')} bind:value={notes} oninput={typed} onblur={saveNotes}></textarea>
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
