<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog, Icon, IconButton, InstanceIcon, ProgressBar, Slider, StatusTag } from '../../cubby'
  import { bytes, num, timeOfDay } from '../../format'
  import { iconOf, projectArt } from '../../icon'
  import { queue } from '../../queue.svelte'
  import { app, taskKind, type FinishedTask, type InstanceTab } from '../../state.svelte'
  import type { DataFolder } from '../../types'

  /** Task kinds that are bookkeeping, not downloads ("session" = play time saved). */
  const HIDDEN = new Set(['session'])
  const RUNNING: Record<string, string> = {
    launch: t('Getting ready'),
    install: t('Installing'),
    modpack: t('Installing modpack'),
    update: t('Updating'),
    repair: t('Repairing'),
    backup: t('Backing up'),
  }
  const DONE: Record<string, string> = {
    launch: t('Ready to play'),
    install: t('Content installed'),
    modpack: t('Modpack installed'),
    update: t('Content updated'),
    repair: t('Game files checked'),
    backup: t('World backed up'),
  }
  const FAILED: Record<string, string> = {
    launch: t('Getting ready failed'),
    install: t('Install failed'),
    modpack: t('Modpack install failed'),
    update: t('Update failed'),
    repair: t('Repair failed'),
    backup: t('Backup failed'),
  }

  function instanceIdOf(taskId: string): string {
    return taskId.slice(taskKind(taskId).length + 1)
  }

  let now = $state(Date.now())

  const active = $derived(
    Object.entries(app.tasks)
      .filter(([key]) => !HIDDEN.has(taskKind(key)))
      .map(([key, t]) => ({ key, kind: taskKind(key), instanceId: instanceIdOf(key), ...t }))
      .sort((a, b) => a.started - b.started),
  )

  $effect(() => {
    if (!active.length) return
    now = Date.now()
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  const queued = $derived(queue.items.filter((i) => i.state !== 'staged'))
  const QSTATE: Record<string, string> = { running: t('Downloading'), done: t('Done'), failed: t('Failed') }

  const finished = $derived(app.finished.filter((f) => !HIDDEN.has(taskKind(f.id))))
  const failed = $derived(finished.filter((f) => f.error))
  const done = $derived(finished.filter((f) => !f.error))

  function elapsed(ms: number): string {
    const s = Math.max(0, Math.floor(ms / 1000))
    const pad = (n: number) => String(n).padStart(2, '0')
    return s >= 3600 ? `${Math.floor(s / 3600)}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}` : `${Math.floor(s / 60)}:${pad(s % 60)}`
  }

  function pctOf(t: { done: number; total: number }): number | undefined {
    return t.total ? (t.done / t.total) * 100 : undefined
  }

  function at(ms: number): string {
    return timeOfDay(new Date(ms).toISOString())
  }

  function nameOf(instanceId: string): string {
    return app.instance(instanceId)?.name ?? instanceId
  }

  function dismiss(f: FinishedTask) {
    app.finished = app.finished.filter((x) => x !== f && !(x.id === f.id && x.at === f.at))
  }

  function clearDone() {
    app.finished = app.finished.filter((f) => f.error || HIDDEN.has(taskKind(f.id)))
  }

  function retry(f: FinishedTask) {
    const kind = taskKind(f.id)
    const instanceId = instanceIdOf(f.id)
    dismiss(f)
    if (kind === 'launch') void app.launch(instanceId)
    else if (kind === 'repair') api.repairInstance(instanceId).catch((e) => app.error(e))
  }

  function openFor(f: FinishedTask) {
    const kind = taskKind(f.id)
    const tab: InstanceTab = kind === 'install' || kind === 'update' ? 'mods' : 'overview'
    app.openInstance(instanceIdOf(f.id), tab)
  }

  // Storage
  const LABELS: Record<DataFolder['key'], string> = {
    instances: t('Instances'),
    libraries: t('Libraries'),
    assets: t('Game assets'),
    java: 'Java',
    meta: t('Cache'),
    logs: t('Launcher logs'),
    trash: t('Trash'),
  }

  let folders = $state<DataFolder[] | null>(null)
  let foldersFailed = $state(false)
  let confirmClean = $state(false)
  let cleaning = $state(false)
  let confirmTrash = $state(false)
  let emptying = $state(false)
  const trash = $derived(folders?.find((f) => f.key === 'trash') ?? null)

  async function loadFolders() {
    try {
      folders = await api.dataFolders()
      foldersFailed = false
    } catch (e) {
      foldersFailed = true
      folders ??= []
      app.error(e)
    }
  }

  $effect(() => {
    untrack(() => void loadFolders())
  })

  const total = $derived((folders ?? []).reduce((sum, f) => sum + f.sizeBytes, 0))
  const cache = $derived(folders?.find((f) => f.key === 'meta') ?? null)
  const barLabel = $derived((folders ?? []).map((f) => `${LABELS[f.key] ?? f.key} ${bytes(f.sizeBytes)}`).join(', '))

  function openData(key: DataFolder['key']) {
    api.openDataFolder(key).catch((e) => app.error(e))
  }

  async function clean() {
    cleaning = true
    try {
      await api.clearCache()
      confirmClean = false
      app.toast(t('Cache cleaned up.'), 'success')
      await loadFolders()
    } catch (e) {
      app.error(e)
    } finally {
      cleaning = false
    }
  }

  async function emptyTrash() {
    emptying = true
    try {
      await api.clearTrash()
      confirmTrash = false
      app.toast(t('Trash emptied.'), 'success')
      await loadFolders()
    } catch (e) {
      app.error(e)
    } finally {
      emptying = false
    }
  }

  // Parallel downloads, saved shortly after the slider stops moving
  let draft = $state<number | null>(null)
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  const concurrency = $derived(draft ?? app.settings?.downloadConcurrency ?? 16)

  function setConcurrency(v: number) {
    draft = v
    clearTimeout(saveTimer)
    saveTimer = setTimeout(async () => {
      await app.saveSettings({ downloadConcurrency: v })
      if (draft === v) draft = null
    }, 300)
  }

  $effect(() => () => clearTimeout(saveTimer))
</script>

{#snippet trashActions()}
  <Button variant="ghost" onclick={() => (confirmTrash = false)}>{t('Cancel')}</Button>
  <Button variant="danger" icon="trash" busy={emptying} disabled={emptying} onclick={emptyTrash}>{t('Empty trash')}</Button>
{/snippet}

{#snippet cleanActions()}
  <Button variant="ghost" onclick={() => (confirmClean = false)}>{t('Cancel')}</Button>
  <Button variant="danger" icon="trash" busy={cleaning} disabled={cleaning} onclick={clean}>{t('Clean up cache')}</Button>
{/snippet}

<div class="screen">
  <div class="layout">
    <div class="content">
      <h1 class="text-title">{t('Downloads')}</h1>

      {#if queued.length}
        <section class="section" aria-labelledby="queue-title">
          <div class="sec-head">
            <h2 id="queue-title" class="text-label muted grow">{t('Queue')}</h2>
            <Button size="sm" variant="ghost" icon="close" onclick={() => queue.clearFinished()}>{t('Clear')}</Button>
          </div>
          <ul class="card list">
            {#each queued as i (i.key)}
              <li class="qrow">
                <InstanceIcon {...(i.iconUrl ? { src: i.iconUrl } : projectArt(i.title))} size={24} />
                <div class="qname">
                  <span class="text-body-strong">{i.title}</span>
                  <span class="text-small muted">{nameOf(i.instanceId)}{i.neededBy ? ' · ' + t('needed by {name}', { name: i.neededBy }) : ''}{i.bytes ? ' · ' + bytes(i.bytes) : ''}</span>
                  {#if i.state === 'failed'}<span class="text-small is-bad">{i.error}</span>{/if}
                </div>
                <StatusTag state={i.state === 'done' ? 'ready' : i.state === 'failed' ? 'crashed' : 'installing'}>{QSTATE[i.state]}</StatusTag>
                {#if i.state === 'failed'}
                  <Button size="sm" icon="update" onclick={() => void queue.startAll(i.instanceId)}>{t('Retry')}</Button>
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <section class="section" aria-labelledby="active-title">
        <h2 id="active-title" class="text-label muted">{t('Downloading now')}</h2>
        {#if active.length}
          <ul class="tasks">
            {#each active as task (task.key)}
              {@const inst = app.instance(task.instanceId)}
              {@const pct = pctOf(task)}
              <li class="card task">
                <div class="task-head">
                  {#if inst}
                    <InstanceIcon {...iconOf(inst)} seed={inst.name} size={32} raised />
                  {:else}
                    <span class="icon-box" aria-hidden="true"><Icon name="download" size={18} /></span>
                  {/if}
                  <div class="task-name">
                    <span class="text-body-strong">{inst?.name ?? task.instanceId}</span>
                    <StatusTag state="installing">{RUNNING[task.kind] ?? t('Working')}</StatusTag>
                  </div>
                  <span class="elapsed text-small muted">
                    <Icon name="clock" size={18} /><span class="cb-visually-hidden">{t('Running for')}</span><span class="text-counter">{elapsed(now - task.started)}</span>
                  </span>
                </div>
                <ProgressBar
                  tone="diamond"
                  value={pct}
                  label={task.label}
                  valueText={pct != null ? `${Math.round(pct)}%` : undefined}
                  ariaValueText={pct != null ? t('{pct}%, {n} of {n2}', { pct: Math.round(pct), n: num(task.done), n2: num(task.total) }) : t('{label}, no estimate yet', { label: task.label })}
                  detail={task.total ? `${num(task.done)} of ${num(task.total)}` : undefined}
                />
              </li>
            {/each}
          </ul>
        {:else}
          <div class="card empty">
            <Icon name="download" size={27} />
            <div class="empty-text">
              <p class="text-body-strong">{t('Nothing is downloading right now')}</p>
              <p class="text-small muted">{t('Installs, updates and game files show their progress here.')}</p>
            </div>
            <Button icon="discover" onclick={() => app.navigate({ name: 'discover' })}>{t('Discover mods')}</Button>
          </div>
        {/if}
      </section>

      <div class="columns">
        {#if failed.length}
          <section class="section col" aria-labelledby="failed-title">
            <h2 id="failed-title" class="text-label muted">{t('Failed')}</h2>
            <ul class="card list">
              {#each failed as f (f.id + f.at)}
                {@const kind = taskKind(f.id)}
                {@const instanceId = instanceIdOf(f.id)}
                <li class="item">
                  <span class="item-icon is-bad"><Icon name="error" size={18} /></span>
                  <div class="item-body">
                    <div class="item-head">
                      <span class="text-body-strong grow">{FAILED[kind] ?? t('Failed')}</span>
                      <span class="text-small muted">{at(f.at)}</span>
                    </div>
                    <span class="text-small is-bad">{f.error}</span>
                    <span class="text-small muted">{t('For {id}', { id: nameOf(instanceId) })}{f.label !== f.id ? t(' · last step: {label}', { label: f.label }) : ''}</span>
                    <div class="item-actions">
                      {#if (kind === 'launch' || kind === 'repair') && app.instance(instanceId)}
                        <Button size="sm" icon="update" onclick={() => retry(f)}>{t('Retry')}</Button>
                      {:else if app.instance(instanceId)}
                        <Button size="sm" icon="cube" onclick={() => openFor(f)}>{t('Open {instanceId}', { instanceId: nameOf(instanceId) })}</Button>
                      {/if}
                      <Button size="sm" variant="ghost" onclick={() => dismiss(f)}>{t('Dismiss')}</Button>
                    </div>
                  </div>
                </li>
              {/each}
            </ul>
          </section>
        {/if}

        <section class="section col" aria-labelledby="done-title">
          <div class="sec-head">
            <h2 id="done-title" class="text-label muted grow">{t('Done this session')}</h2>
            {#if done.length}<Button size="sm" variant="ghost" icon="close" onclick={clearDone}>{t('Clear')}</Button>{/if}
          </div>
          {#if done.length}
            <ul class="card list">
              {#each done as f (f.id + f.at)}
                {@const kind = taskKind(f.id)}
                <li class="item">
                  <span class="item-icon is-ok"><Icon name="check" size={18} /></span>
                  <div class="item-body">
                    <div class="item-head">
                      <span class="text-body-strong grow">{DONE[kind] ?? t('Done')}</span>
                      <span class="text-small muted">{at(f.at)}</span>
                    </div>
                    <span class="text-small muted">{t('For {id}', { id: nameOf(instanceIdOf(f.id)) })}</span>
                  </div>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="text-small muted">{t('What finishes while Cubby is open shows up here.')}</p>
          {/if}
        </section>
      </div>
    </div>

    <aside class="aside" aria-label={t('Storage')}>
      <h2 class="text-heading">{t('Storage')}</h2>
      {#if folders == null}
        <p class="text-small muted">{t('Measuring Cubby’s folders…')}</p>
      {:else if foldersFailed && !folders.length}
        <p class="text-small muted">{t('Cubby couldn’t measure its folders.')}</p>
        <div><Button size="sm" icon="update" onclick={loadFolders}>{t('Try again')}</Button></div>
      {:else}
        <div class="total">
          <span class="text-small muted">{t('Cubby uses')}</span>
          <span class="text-stat">{bytes(total)}</span>
          {#if app.info?.dataDir}<span class="text-mono muted path">{app.info.dataDir}</span>{/if}
        </div>
        {#if total > 0}
          <div class="bar" role="img" aria-label={barLabel}>
            {#each folders.filter((f) => f.sizeBytes > 0) as f (f.key)}
              <span class={['seg', `seg--${f.key}`]} style:flex="{Math.max(1, Math.round((f.sizeBytes / total) * 1000))} 1 0px"></span>
            {/each}
          </div>
        {/if}
        <ul class="legend">
          {#each folders as f (f.key)}
            <li class="legend-row">
              <span class={['swatch', `seg--${f.key}`]} aria-hidden="true"></span>
              <span class="text-body grow">{LABELS[f.key] ?? f.label}</span>
              <span class="text-body-strong">{bytes(f.sizeBytes)}</span>
              <IconButton icon="folder" size="sm" label={t('Open the {toLowerCase} folder', { toLowerCase: (LABELS[f.key] ?? f.label).toLowerCase() })} onclick={() => openData(f.key)} />
            </li>
          {/each}
        </ul>
        <div class="clean">
          <Button size="sm" icon="trash" onclick={() => (confirmClean = true)}>{t('Clean up cache')}</Button>
          <span class="text-small muted">{cache ? t('Frees {n}. ', { n: bytes(cache.sizeBytes) }) : ''}{t('Your instances stay as they are.')}</span>
        </div>
        {#if trash && trash.sizeBytes > 0}
          <div class="clean">
            <Button size="sm" variant="danger" icon="trash" onclick={() => (confirmTrash = true)}>{t('Empty trash')}</Button>
            <span class="text-small muted">{t('Frees {n} for good.', { n: bytes(trash.sizeBytes) })}</span>
          </div>
        {/if}
      {/if}

      <div class="settings">
        <Slider
          label={t('Parallel downloads')}
          min={1}
          max={16}
          step={1}
          value={Math.min(16, concurrency)}
          valueText={t('{n} at a time', { n: concurrency })}
          marks={['1', '16']}
          onChange={setConcurrency}
        />
        <div class="settings__more">
          <Button size="sm" iconEnd="chevron-right" onclick={() => app.navigate({ name: 'settings', section: 'downloads' })}>{t('All download settings')}</Button>
        </div>
      </div>
    </aside>
  </div>
</div>

{#if confirmTrash}
  <Dialog title={t('Empty the trash?')} icon="trash" tone="danger" onClose={() => (confirmTrash = false)} actions={trashActions}>
    <p>{t('Permanently deletes what you removed: {size}. This can’t be undone.', { size: bytes(trash?.sizeBytes ?? 0) })}</p>
  </Dialog>
{/if}

{#if confirmClean}
  <Dialog title={t('Clean up the cache?')} icon="trash" tone="warning" onClose={() => (confirmClean = false)} actions={cleanActions}>
    <p>
      {cache ? t('Deletes the version lists and loader files Cubby keeps ({size}). They download again when needed.', { size: bytes(cache.sizeBytes) }) : t('Deletes the version lists and loader files Cubby keeps. They download again when needed.')}
    </p>
    <p>{t('Your instances, worlds and mods stay as they are.')}</p>
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
  ul {
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
  .card {
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .sec-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--size-control-sm);
  }

  .qrow {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--size-control);
  }
  .qname {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .tasks {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .task {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
  }
  .task-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .task-name {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }
  .elapsed {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .icon-box {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 36px;
    height: 36px;
    color: var(--ink-muted);
    background: var(--bg-chrome);
    border: var(--border-block) solid var(--edge);
  }

  .empty {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-4);
    color: var(--ink-muted);
  }
  .empty-text {
    flex: 1 1 240px;
    min-width: 0;
  }
  .empty-text .text-body-strong {
    color: var(--ink);
  }

  .columns {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-4);
  }
  .col {
    flex: 1 1 300px;
    min-width: 0;
  }
  .item {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .item:last-child {
    border-bottom: 0;
  }
  .item-icon {
    flex: none;
    display: flex;
    padding-top: 2px;
  }
  .item-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item-body > span {
    overflow-wrap: anywhere;
  }
  .item-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-2);
  }
  .item-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }
  .is-bad {
    color: var(--redstone-text);
  }
  .is-ok {
    color: var(--grass-text);
  }

  .aside {
    flex: none;
    min-height: 0;
    overflow: auto;
    width: var(--size-inspector);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .total {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .path {
    overflow-wrap: anywhere;
  }
  .bar {
    display: flex;
    gap: 2px;
    height: 24px;
    padding: 2px;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-sizing: border-box;
  }
  .seg {
    min-width: 4px;
  }
  .legend {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .legend-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .swatch {
    flex: none;
    width: 14px;
    height: 14px;
    border: var(--border-block) solid var(--edge);
  }
  .seg--instances {
    background: var(--grass);
  }
  .seg--libraries {
    background: var(--diamond);
  }
  .seg--assets {
    background: var(--gold);
  }
  .seg--java {
    background: var(--ink);
  }
  .seg--meta {
    background: var(--ink-muted);
  }
  .seg--logs {
    background: var(--outline);
  }
  .seg--trash {
    background: var(--line);
  }
  .clean {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    text-align: end;
  }
  .settings {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding-top: var(--space-4);
    border-top: var(--border-block) solid var(--line);
  }
  .settings__more {
    display: flex;
    justify-content: flex-end;
  }
  .settings :global(.cb-slider) {
    min-width: 0;
  }
</style>
