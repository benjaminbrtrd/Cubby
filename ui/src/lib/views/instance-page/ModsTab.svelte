<script lang="ts" module>
  import type { Project } from '../../types'

  /** Modrinth projects fetched for the details panel, kept while the app runs. */
  const projects = new Map<string, Project>()
</script>

<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, InstanceIcon, MetaChip, ModRow, ProgressBar, SegmentedControl, StatusTag, TextField } from '../../cubby'
  import { bytes, fold, num, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { ContentSource, ContentUpdate, Instance, ModFile } from '../../types'
  import { baseFile, modName, type InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  type Filter = 'all' | 'updates' | 'warnings' | 'disabled'

  let query = $state('')
  let filter = $state<Filter>('all')
  let selected = $state<string | null>(null)
  let limit = $state(60)
  let removing = $state<ModFile | null>(null)
  let removeBusy = $state(false)
  let asideTitle: HTMLElement | undefined = $state()

  type Row = { mod: ModFile; key: string; name: string; search: string; source?: ContentSource; update?: ContentUpdate; twins?: ModFile[] }

  // Recomputed when the files, sources or updates change, not on each keystroke.
  // A row keeps its key (the file name without ".disabled") when the mod is turned on or off, so focus stays on it;
  // if both "x.jar" and "x.jar.disabled" exist, the second one is keyed by its full name.
  const rows = $derived.by<Row[]>(() => {
    const seen = new Set<string>()
    return (data.mods ?? [])
      .map((mod) => {
        const name = modName(mod)
        const base = baseFile(mod.fileName)
        const key = seen.has(base) ? mod.fileName : base
        seen.add(base)
        return {
          mod,
          key,
          name,
          search: fold(`${name} ${mod.fileName} ${mod.description ?? ''} ${mod.authors.join(' ')}`),
          source: data.source(mod.fileName),
          update: data.update(mod.fileName),
          twins: data.twins.get(mod.fileName),
        }
      })
      .sort((a, b) => fold(a.name).localeCompare(fold(b.name)))
  })

  const counts = $derived({
    all: rows.length,
    updates: rows.filter((r) => r.update).length,
    warnings: rows.filter((r) => r.twins).length,
    disabled: rows.filter((r) => !r.mod.enabled).length,
  })
  const filters = $derived([
    { value: 'all', label: t('All {n}', { n: num(counts.all) }) },
    { value: 'updates', label: t('Updates {n}', { n: num(counts.updates) }), icon: 'update' as const },
    { value: 'warnings', label: t('Warnings {n}', { n: num(counts.warnings) }), icon: 'warning' as const },
    { value: 'disabled', label: t('Disabled {n}', { n: num(counts.disabled) }), icon: 'minimize' as const },
  ])

  const q = $derived(fold(query.trim()))
  const shown = $derived(
    rows.filter(
      (r) =>
        (filter === 'all' || (filter === 'updates' && r.update) || (filter === 'warnings' && r.twins) || (filter === 'disabled' && !r.mod.enabled)) &&
        (!q || r.search.includes(q)),
    ),
  )
  const visible = $derived(shown.slice(0, limit))

  const sel = $derived(rows.find((r) => r.key === selected) ?? null)
  const updateTask = $derived(app.tasks[`update:${id}`])
  const installTask = $derived(app.tasks[`install:${id}`])
  const running = $derived(!!app.running[id])

  function select(r: Row) {
    selected = r.key
  }

  async function details(r: Row) {
    selected = r.key
    await tick()
    asideTitle?.focus()
  }

  async function toggle(r: Row, on: boolean) {
    const file = r.mod.fileName
    data.mods = (data.mods ?? []).map((m) => (m.fileName === file ? { ...m, enabled: on } : m))
    try {
      await api.setModEnabled(id, file, on)
    } catch (e) {
      app.error(e)
    }
    // the file is renamed (".disabled"), so read the folder again
    await data.loadMods()
  }

  async function remove() {
    const m = removing
    if (!m) return
    removeBusy = true
    try {
      await api.removeMod(id, m.fileName)
      data.mods = (data.mods ?? []).filter((x) => x.fileName !== m.fileName)
      app.forgetFile(id, m.fileName)
      if (selected === baseFile(m.fileName)) selected = null
      app.toast(t('{m} moved to the instance’s trash folder.', { m: modName(m) }), 'success')
      removing = null
      void data.loadMods()
    } catch (e) {
      app.error(e)
    } finally {
      removeBusy = false
    }
  }

  function updateAll() {
    void data.applyUpdates(rows.filter((r) => r.update).map((r) => r.update!.fileName))
  }

  function clear() {
    query = ''
    filter = 'all'
  }

  // Details panel: the Modrinth project of the selected mod, fetched only for it
  let project = $state<Project | null>(null)
  let projectFor = ''
  $effect(() => {
    const pid = sel?.source?.projectId
    if (!pid) {
      projectFor = ''
      project = null
      return
    }
    if (pid === projectFor) return
    projectFor = pid
    const cached = projects.get(pid)
    project = cached ?? null
    if (cached) return
    api
      .getProject(pid)
      .then((p) => {
        projects.set(pid, p)
        if (projectFor === pid) project = p
      })
      .catch(() => {})
  })

  const kind = $derived.by(() => {
    if (!project) return null
    if (project.categories.includes('library')) return t('Library: other mods need it')
    if (project.categories.includes('optimization')) return t('Performance')
    return t('Content')
  })

  /** A Markdown changelog as short plain lines. */
  function notes(md: string): string[] {
    return md
      .split('\n')
      .map((l) => l.replace(/^\s*(#+|[-*+]|\d+\.)\s*/, '').replace(/[*_`]/g, '').trim())
      .filter(Boolean)
      .slice(0, 8)
  }

  const authors = (m: ModFile) => (m.authors.length ? `by ${m.authors.join(', ')}` : undefined)
</script>

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Mods')}>
  <div class="toolbar">
    <div class="search">
      <TextField label={t('Search mods')} hideLabel icon="search" placeholder={t('Search {n}', { n: plural(rows.length, 'mod', 'mods') })} bind:value={query} oninput={() => (limit = 60)} />
    </div>
    <Button icon="folder" onclick={() => data.openFolder('mods')}>{t('Open folder')}</Button>
    <Button variant="primary" icon="plus" onclick={() => data.discover('mod')}>{t('Add mods')}</Button>
  </div>
  <div class="toolbar">
    <SegmentedControl label={t('Show')} options={filters} value={filter} onChange={(v) => ((filter = v as Filter), (limit = 60))} />
    <span class="grow"></span>
    {#if counts.updates}
      <Button icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={updateAll}>{t('Update all ({n})', { n: num(counts.updates) })}</Button>
    {:else if data.checkingUpdates}
      <Button icon="update" busy disabled>{t('Checking for updates')}</Button>
    {:else if !data.updates}
      <Button icon="update" onclick={() => data.checkUpdates()}>{t('Check for updates')}</Button>
    {:else}
      <Button variant="ghost" icon="update" onclick={() => data.checkUpdates(true)}>{t('Check again')}</Button>
    {/if}
  </div>

  {#if updateTask || installTask}
    {@const t = (updateTask ?? installTask)!}
    <ProgressBar tone="diamond" label={t.label} value={t.total ? (t.done / t.total) * 100 : undefined} />
  {/if}

  {#if instance.loader.kind === 'vanilla' && rows.length}
    {#snippet loaderAction()}<Button size="sm" icon="wrench" onclick={() => app.openInstance(id, 'settings')}>{t('Change loader')}</Button>{/snippet}
    <Banner tone="warning" title={t('These mods don’t load')} actions={loaderAction}>
      {t('This instance runs Vanilla Minecraft, which ignores the mods folder. Pick Fabric or Quilt in Settings to load them.')}
    </Banner>
  {:else if counts.warnings}
    {@const twin = rows.find((r) => r.twins)}
    {#snippet twinAction()}<Button size="sm" icon="warning" onclick={() => (filter = 'warnings')}>{t('Show them')}</Button>{/snippet}
    <Banner tone="warning" title={counts.warnings === 2 && twin ? `${twin.name} is installed twice` : t('Some mods are installed twice')} actions={twinAction}>
      {t('Two files load the same mod, and the game won’t start with both. Remove the older file.')}
    </Banner>
  {/if}

  <p class="hint text-body"><Icon name="info" size={18} />{t('Turn a mod off to test without it. Nothing is deleted.')}</p>

  {#if data.mods === null}
    <p class="muted text-body">{t('Reading the mods folder…')}</p>
  {:else if !rows.length}
    <div class="empty">
      {#if instance.loader.kind === 'vanilla'}
        <p class="text-heading">{t('Vanilla runs without mods')}</p>
        <p class="muted text-body">{t('To add mods, pick a loader like Fabric in Settings, then come back here.')}</p>
        <Button icon="wrench" onclick={() => app.openInstance(id, 'settings')}>{t('Change loader')}</Button>
      {:else}
        <p class="text-heading">{t('No mods yet')}</p>
        <p class="muted text-body">{t('Find mods made for {minecraft} on Modrinth, or drop .jar files in the mods folder.', { minecraft: instance.minecraft })}</p>
        <Button variant="primary" icon="plus" onclick={() => data.discover('mod')}>{t('Add mods')}</Button>
      {/if}
    </div>
  {:else if !shown.length}
    <div class="empty">
      <p class="text-heading">{t('No mods match')}</p>
      <Button variant="ghost" icon="close" onclick={clear}>{t('Clear search and filter')}</Button>
    </div>
  {:else}
    <div class="list">
      {#each visible as r (r.key)}
        <!-- Click anywhere on a row to show it in the panel; the ⋯ button does the same from the keyboard. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class={['row', selected === r.key && 'is-selected']} onclick={() => select(r)}>
          <ModRow
            name={r.name}
            version={r.mod.version ?? undefined}
            authors={authors(r.mod)}
            description={r.mod.description ?? undefined}
            iconSrc={r.source?.iconUrl ?? undefined}
            enabled={r.mod.enabled}
            onToggle={(on) => toggle(r, on)}
            toggleLabel={t('Load {name} when you play', { name: r.name })}
            update={r.update ? t('Update to {latestVersion}', { latestVersion: r.update.latestVersion }) : undefined}
            warning={r.twins ? t('Installed twice') : undefined}
            moreLabel={t('Show details for {name}', { name: r.name })}
            onMore={() => details(r)}
          />
        </div>
      {/each}
    </div>
    <div class="toolbar">
      <span class="muted text-small">{t('Showing {n} of {n2}', { n: num(visible.length), n2: num(rows.length) })}</span>
      {#if shown.length > visible.length}
        <Button size="sm" variant="ghost" icon="chevron-down" onclick={() => (limit = Infinity)}>{t('Show all {n}', { n: num(shown.length) })}</Button>
      {/if}
    </div>
  {/if}
</div>

<aside class="ip-aside" aria-label={t('Mod details')}>
  <div class="ip-aside__inner">
    <p class="text-label muted">{t('Selected mod')}</p>
    {#if sel}
      {@const m = sel.mod}
      <div class="who">
        {#if sel.source?.iconUrl}
          <InstanceIcon src={sel.source.iconUrl} size={64} raised />
        {:else}
          <InstanceIcon {...projectArt(sel.source?.slug ?? sel.name)} size={64} raised />
        {/if}
        <div class="who__text">
          <h2 class="text-heading" tabindex="-1" bind:this={asideTitle}>{sel.name}</h2>
          {#if m.authors.length}<p class="muted text-small">{authors(m)}</p>{/if}
        </div>
      </div>
      <div class="chips">
        {#if m.version}<MetaChip>{m.version}</MetaChip>{/if}
        {#if !m.enabled}
          <StatusTag state="disabled">{t('Off')}</StatusTag>
        {:else if sel.update}
          <StatusTag state="update">{t('On · update ready')}</StatusTag>
        {:else if data.updates && sel.source}
          <StatusTag state="ready">{t('On · up to date')}</StatusTag>
        {:else}
          <StatusTag state="ready">{t('On')}</StatusTag>
        {/if}
      </div>
      <section class="part">
        <h3 class="text-label muted">{t('What it does')}</h3>
        <p class="text-body">{m.description || project?.summary || t('This file doesn’t describe itself.')}</p>
      </section>
      <dl class="facts">
        {#if kind}
          <dt class="text-small muted">{t('Type')}</dt>
          <dd class="text-body">{kind}</dd>
        {/if}
        <dt class="text-small muted">{t('Source')}</dt>
        <dd class="text-body">{sel.source ? 'Modrinth' : t('Not found on Modrinth')}</dd>
        {#if project}
          <dt class="text-small muted">{t('Updated')}</dt>
          <dd class="text-body">{relativeTime(project.updated)}</dd>
        {/if}
        <dt class="text-small muted">{t('Size')}</dt>
        <dd class="text-body">{bytes(m.size)}</dd>
        <dt class="text-small muted full">{t('File')}</dt>
        <dd class="text-mono full wrap">{m.fileName}</dd>
      </dl>
      {#if sel.twins}
        <p class="warn text-small flag"><Icon name="warning" size={18} label={t('Warning')} /><span>{t('Also loaded from')} <span class="text-mono">{sel.twins.map((t) => t.fileName).join(', ')}</span>{t('. Keep one.')}</span></p>
      {/if}
      {#if sel.update}
        <section class="part">
          <h3 class="text-label muted">{t('New in {latestVersion}', { latestVersion: sel.update.latestVersion })}</h3>
          {#if notes(sel.update.changelog).length}
            <ul class="changes text-small">
              {#each notes(sel.update.changelog) as line, i (i)}<li>{line}</li>{/each}
            </ul>
          {:else}
            <p class="muted text-small">{t('No changelog for this version.')}</p>
          {/if}
        </section>
      {/if}
      <div class="actions">
        {#if sel.source}<Button size="sm" icon="globe" onclick={() => sel.source && data.project(sel.source.slug, 'mod')}>{t('Open page')}</Button>{/if}
        {#if sel.update}
          <Button size="sm" icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={() => sel.update && data.applyUpdates([sel.update.fileName])}>{t('Update')}</Button>
        {/if}
        <Button size="sm" icon={m.enabled ? 'minimize' : 'check'} onclick={() => toggle(sel, !m.enabled)}>{m.enabled ? t('Turn off') : t('Turn on')}</Button>
        <Button size="sm" variant="danger" icon="trash" onclick={() => (removing = m)}>{t('Remove')}</Button>
      </div>
    {:else}
      <p class="muted text-body">{t('Pick a mod to see what it does, its file and its page on Modrinth.')}</p>
    {/if}

  </div>
</aside>

{#if removing}
  {@const m = removing}
  {#snippet removeActions()}
    <Button variant="ghost" onclick={() => (removing = null)}>{t('Cancel')}</Button>
    <Button variant="danger" icon="trash" busy={removeBusy} disabled={removeBusy} onclick={remove}>{t('Remove')}</Button>
  {/snippet}
  <Dialog tone="danger" icon="trash" title={t('Remove {m}?', { m: modName(m) })} onClose={() => (removing = null)} actions={removeActions}>
    <p>{t('Moves')} <span class="text-mono">{m.fileName}</span> {t('to the trash folder of this instance.')}</p>
    <p>{t('Mods that need it may stop working.')}</p>
  </Dialog>
{/if}

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .search {
    flex: 1 1 240px;
    min-width: 200px;
  }
  .grow {
    flex: 1;
  }
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
  .warn {
    color: var(--gold-text);
  }
  .wrap {
    overflow-wrap: anywhere;
  }
  .hint {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--ink-muted);
  }

  .list {
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .row {
    position: relative;
  }
  .row + .row {
    border-top: var(--border-hair) solid var(--line);
  }
  .row.is-selected::after {
    content: '';
    position: absolute;
    inset: 0;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
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
  .empty :global(.cb-btn) {
    margin-top: var(--space-2);
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
    gap: 2px;
  }
  .who__text h2 {
    overflow-wrap: anywhere;
    outline: none;
  }
  .chips,
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .part {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .facts {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--space-2) var(--space-4);
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .facts .full {
    grid-column: 1 / -1;
  }
  .flag {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .changes {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding-inline-start: 20px;
    list-style: square;
  }
</style>
