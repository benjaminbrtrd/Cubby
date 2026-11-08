<script lang="ts" module>
  import type { Project } from '../../types'

  /** Modrinth projects fetched for the details panel, kept while the app runs. */
  const projects = new Map<string, Project>()
</script>

<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, Inspector, InspectorHead, InspectorNotice, InspectorPath, InspectorSection, InspectorStatus, InstanceIcon, MetaChip, ModRow, ProgressBar, SegmentedControl, TextField } from '../../cubby'
  import { bytes, fold, num, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { ContentSource, ContentUpdate, Instance, ModFile } from '../../types'
  import { baseFile, modName, when, type InstanceData } from './InstancePage.svelte'
  import Markdown from '../discover/Markdown.svelte'

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
    void focusPanel()
  }

  async function focusPanel() {
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

  // Updating: the review lists each new version with its changelog before anything changes.
  let reviewing = $state(false)
  const pending = $derived(rows.filter((r) => r.update))

  function updateAll() {
    reviewing = false
    void data.applyUpdates(pending.map((r) => r.update!.fileName))
  }

  // Undoing the last update
  untrack(() => {
    if (!data.updateBackups) void data.loadUpdateBackups()
  })
  const lastUpdate = $derived(data.updateBackups?.[0] ?? null)
  let undoing = $state(false)
  let undoBusy = $state(false)

  async function undo() {
    const b = lastUpdate
    if (!b) return
    undoBusy = true
    try {
      const n = await api.undoUpdate(id, b.name)
      undoing = false
      app.toast(t('Put back {n} from before the update.', { n: plural(n, 'file', 'files') }), 'success')
      await Promise.all([data.loadMods(), data.loadPacks(), data.loadUpdateBackups(), data.checkUpdates(true)])
    } catch (e) {
      app.error(e)
    } finally {
      undoBusy = false
    }
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
  /** Changelog lines as plain text: Markdown links keep their text ("([#3916](https://…))" → "(#3916)"), bare URLs go. */
  function notes(md: string): string[] {
    return md
      .split('\n')
      .map((l) =>
        l
          .replace(/^\s*(#+|[-*+]|\d+\.)\s*/, '')
          .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
          .replace(/\(?\s*https?:\/\/\S+?\s*\)?(?=\s|$)/g, '')
          .replace(/[*_`]/g, '')
          .replace(/\(\s*\)/g, '')
          .trim(),
      )
      .filter(Boolean)
  }

  const NOTES_SHOWN = 4
  /** The selected mod whose whole changelog is shown. */
  let notesOpen = $state<string | null>(null)

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
    {#if lastUpdate && !updateTask}
      <Button variant="ghost" icon="clock" disabled={running} onclick={() => (undoing = true)}>{t('Undo last update')}</Button>
    {/if}
    {#if counts.updates}
      <Button icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={() => (reviewing = true)}>{t('Update all ({n})', { n: num(counts.updates) })}</Button>
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
        <p class="muted text-body">{t('Find mods made for {minecraft}, or drop .jar files in the mods folder.', { minecraft: instance.minecraft })}</p>
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
        <div class={['row', selected === r.key && 'is-selected']}>
          <!-- Stretched over the whole row: a click anywhere on it, apart from the switch, shows the mod in the panel. -->
          <button type="button" class="row__hit" aria-pressed={selected === r.key} aria-label={t('Show details for {name}', { name: r.name })} onclick={() => select(r)}></button>
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
  <Inspector class="ip-aside__inner" danger={sel ? { label: t('Remove'), hint: t('Moves the file to the instance’s trash.'), onclick: () => sel && (removing = sel.mod) } : null}>
    {#if sel}
      {@const m = sel.mod}
      {#snippet modMedia()}
        <span class="cb-ins-media" style:opacity={m.enabled ? undefined : 0.5}>
          {#if sel.source?.iconUrl}
            <InstanceIcon src={sel.source.iconUrl} size={52} />
          {:else}
            <InstanceIcon {...projectArt(sel.source?.slug ?? sel.name)} size={52} />
          {/if}
        </span>
      {/snippet}
      {#snippet modMeta()}
        {#if m.version}<MetaChip>{m.version}</MetaChip>{/if}
        {#if m.enabled}<InspectorStatus tone="ok">{t('On')}</InspectorStatus>{:else}<InspectorStatus tone="off">{t('Off')}</InspectorStatus>{/if}
        {#if sel.update}
          <InspectorStatus tone="update">{t('Update available')}</InspectorStatus>
        {:else if data.updates && sel.source}
          <InspectorStatus tone="off">{t('Up to date')}</InspectorStatus>
        {/if}
      {/snippet}
      <InspectorHead eyebrow={t('Mod')} title={sel.name} sub={authors(m)} media={modMedia} meta={modMeta} bind:titleEl={asideTitle} />
      {#if sel.twins}
        <InspectorNotice>
          <strong>{t('Installed twice.')}</strong> {t('Also loaded from')} <span class="text-mono">{sel.twins.map((x) => x.fileName).join(', ')}</span>{t('. Keep one.')}
        </InspectorNotice>
      {/if}
      <div class="cb-ins-acts">
        {#if sel.update}
          <Button variant="primary" class="wide" icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={() => sel.update && data.applyUpdates([sel.update.fileName])}>{t('Update to {latestVersion}', { latestVersion: sel.update.latestVersion })}</Button>
        {:else if !m.enabled}
          <Button variant="primary" class="wide" icon="check" onclick={() => toggle(sel, true)}>{t('Turn on')}</Button>
        {/if}
        {#if m.enabled}<Button icon="minimize" onclick={() => toggle(sel, false)}>{t('Turn off')}</Button>{/if}
        {#if sel.source}<Button icon="globe" onclick={() => sel.source && data.project(sel.source.slug, 'mod')}>{t('Open page')}</Button>{/if}
      </div>
      {#if sel.update}
        {@const lines = notes(sel.update.changelog)}
        {@const all = notesOpen === sel.key}
        <InspectorSection title={t('What’s new')} count={sel.update.latestVersion}>
          {#if lines.length}
            <ul class="cb-ins-list">
              {#each all ? lines : lines.slice(0, NOTES_SHOWN) as line, i (i)}<li>{line}</li>{/each}
            </ul>
            {#if lines.length > NOTES_SHOWN}
              <div>
                <Button size="sm" variant="ghost" icon={all ? 'minimize' : 'chevron-down'} onclick={() => (notesOpen = all ? null : sel.key)}>
                  {all ? t('Show less') : t('Show all {n} changes', { n: num(lines.length) })}
                </Button>
              </div>
            {/if}
          {:else}
            <p class="cb-ins-hint">{t('No changelog for this version.')}</p>
          {/if}
        </InspectorSection>
      {/if}
      <InspectorSection title={t('What it does')}>
        <p class="cb-ins-text">{m.description || project?.summary || t('This file doesn’t describe itself.')}</p>
      </InspectorSection>
      <InspectorSection title={t('Details')}>
        <dl class="cb-ins-facts">
          {#if kind}<dt>{t('Type')}</dt><dd><span>{kind}</span></dd>{/if}
          <dt>{t('Source')}</dt><dd><span>{sel.source ? 'Modrinth' : t('Not found on Modrinth')}</span></dd>
          {#if project}<dt>{t('Updated')}</dt><dd><span>{relativeTime(project.updated)}</span></dd>{/if}
          <dt>{t('Size')}</dt><dd><span>{bytes(m.size)}</span></dd>
          <InspectorPath path={m.fileName} openLabel={t('Open folder')} onopen={() => data.openFolder('mods')} />
        </dl>
      </InspectorSection>
    {:else}
      {#snippet emptyMedia()}<span class="cb-ins-media cb-ins-media--empty"><Icon name="mods" size={27} /></span>{/snippet}
      <InspectorHead eyebrow={t('Mod')} title={t('No mod selected')} sub={plural(rows.length, 'mod', 'mods')} media={emptyMedia} empty />
      <p class="cb-ins-hint">{t('Pick a mod in the list to see what it does, its version and its file.')}</p>
      {#if counts.updates}
        <div class="cb-ins-acts">
          <Button icon="update" busy={!!updateTask} disabled={!!updateTask || running} onclick={updateAll}>{t('Update all ({n})', { n: num(counts.updates) })}</Button>
        </div>
      {/if}
    {/if}
  </Inspector>
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

{#if reviewing}
  {#snippet reviewActions()}
    <Button variant="ghost" onclick={() => (reviewing = false)}>{t('Cancel')}</Button>
    <Button variant="primary" icon="update" disabled={!!updateTask || running || !pending.length} onclick={updateAll}>{t('Update {n}', { n: plural(pending.length, 'file', 'files') })}</Button>
  {/snippet}
  <Dialog wide icon="update" title={t('What’s new')} onClose={() => (reviewing = false)} actions={reviewActions}>
    <p class="text-small muted">{t('Cubby keeps the old files, so you can undo the update if something breaks.')}</p>
    <div class="review">
      {#each pending as r (r.key)}
        {@const u = r.update!}
        <details class="review__item">
          <summary>
            <span class="text-body-strong">{r.name}</span>
            <span class="text-small muted text-mono">{u.currentVersion} → {u.latestVersion}</span>
          </summary>
          {#if u.changelog.trim()}
            <div class="review__notes text-small"><Markdown source={u.changelog} level={4} /></div>
          {:else}
            <p class="text-small muted">{t('No changelog for this version.')}</p>
          {/if}
        </details>
      {/each}
    </div>
  </Dialog>
{/if}

{#if undoing && lastUpdate}
  {@const b = lastUpdate}
  {#snippet undoActions()}
    <Button variant="ghost" onclick={() => (undoing = false)}>{t('Cancel')}</Button>
    <Button variant="primary" icon="clock" busy={undoBusy} disabled={undoBusy || running} onclick={undo}>{t('Undo update')}</Button>
  {/snippet}
  <Dialog tone="warning" icon="clock" title={t('Undo the update from {when}?', { when: when(b.created, true) })} onClose={() => (undoing = false)} actions={undoActions}>
    <p>{t('These files go back to the version you had before. The newer files move to the instance’s trash folder.')}</p>
    <ul class="undo text-small">
      {#each b.files as f (f.folder + f.old)}
        <li><span class="text-mono">{f.new}</span> → <span class="text-mono">{f.old}</span></li>
      {/each}
    </ul>
  </Dialog>
{/if}

<style>
  .review {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-height: 55vh;
    overflow-y: auto;
    min-width: min(600px, 80vw);
  }
  .review__item {
    padding: var(--space-2) var(--space-3);
    background: var(--surface);
    border: var(--border-hair) solid var(--line);
  }
  .review__item summary {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-2);
    cursor: pointer;
  }
  .review__notes {
    margin-top: var(--space-2);
    overflow-wrap: anywhere;
  }
  .undo {
    margin: var(--space-2) 0 0;
    padding-left: var(--space-4);
    max-height: 40vh;
    overflow-y: auto;
    overflow-wrap: anywhere;
  }
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
  dl,
  dd {
    margin: 0;
  }
  .muted {
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
  .row__hit {
    all: unset;
    position: absolute;
    inset: 0;
    cursor: pointer;
  }
  .row__hit:focus-visible {
    box-shadow: inset 0 0 0 3px var(--focus);
  }
  /* the row's content sits above the stretched button; only its own controls take clicks */
  .row > :global(.cb-mod) {
    position: relative;
    pointer-events: none;
  }
  .row :global(.cb-mod button),
  .row :global(.cb-mod input),
  .row :global(.cb-mod a) {
    pointer-events: auto;
  }
  .row:hover :global(.cb-mod) {
    background: var(--surface-hover);
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
</style>
