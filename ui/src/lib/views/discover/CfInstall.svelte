<!-- Install a CurseForge project from a Discover card: a mod, shader pack or resource pack goes into the target
     instance with the projects it requires (api.curseforgePlan, then api.curseforgeInstall); a datapack goes into
     a world. Files whose author only allows downloads on curseforge.com get a link to their page instead. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon, InstanceIcon, ProgressBar } from '../../cubby'
  import { bytes, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { CfFile, CfPlan, ProjectHit } from '../../types'
  import Picker from './Picker.svelte'
  import TargetPicker from './TargetPicker.svelte'
  import WorldPicker from './WorldPicker.svelte'
  import { CHANNEL, KINDS, fits, loaderLabel, mcRange } from './discover'

  interface Props {
    project: ProjectHit
    /** The project's page on curseforge.com. */
    pageUrl: string
    /** Called once an install into an instance has started. */
    onInstall?: (instanceId: string) => void
    /** Called when one of the panel's links leaves the current screen (to close a dialog). */
    onLeave?: () => void
    /** The project's files, all versions (a project page loads them). Leave it out and the panel loads them; null = loading. */
    files?: CfFile[] | null
    filesError?: string | null
    /** Bindable: the file to install. Null = the best one for the target. */
    fileId?: string | null
    /** On a project page: with a title. */
    compact?: boolean
    /** Changing the version of a datapack already in the world: its file, moved to the trash once the new one is in. */
    replaces?: string
    /** Opened from the instance's own page: no links back to its tabs. */
    inInstance?: boolean
  }

  let { project, pageUrl, onInstall, onLeave, files: given, filesError: givenError = null, fileId = $bindable(null), compact = true, replaces, inInstance = false }: Props = $props()

  const kind = $derived(project.kind)
  const words = $derived(KINDS[kind])
  const isDatapack = $derived(kind === 'datapack')
  const target = $derived(app.target)
  const targetId = $derived(target?.id)
  const targetMc = $derived(target?.minecraft ?? null)

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  function openPage(url: string) {
    api.openUrl(url).catch((e) => app.error(e))
  }

  // files
  /** Files of every Minecraft version, not just the instance's. */
  let everyVersion = $state(false)
  let loaded = $state<CfFile[] | null>(null)
  let loadError = $state<string | null>(null)
  let reloads = $state(0)
  /** The page gave every file: nothing to load, every version is there. */
  const all = $derived(given !== undefined)
  const files = $derived(all ? given : loaded)
  const filesError = $derived(all ? givenError : loadError)

  $effect(() => {
    if (all) return
    const id = project.id
    const mc = everyVersion ? null : targetMc
    void reloads
    let stale = false
    loaded = null
    loadError = null
    api
      .curseforgeFiles(id, mc)
      .then((list) => {
        if (!stale) loaded = list
      })
      .catch((e) => {
        if (!stale) loadError = message(e)
      })
    return () => {
      stale = true
    }
  })

  /** Datapacks: only the .zip files can go into a world. */
  const usable = $derived(files ? (isDatapack ? files.filter((f) => f.fileName.toLowerCase().endsWith('.zip')) : files) : null)
  const fitting = $derived(usable ? usable.filter((f) => fits(f, kind, target)) : [])
  const others = $derived(usable ? usable.filter((f) => !fits(f, kind, target)) : [])
  const best = $derived(fitting.find((f) => f.channel === 'release') ?? fitting[0])
  const chosen = $derived(usable?.find((f) => f.id === fileId) ?? best ?? usable?.[0] ?? null)
  const chosenFits = $derived(!!chosen && fits(chosen, kind, target))

  // a new instance gets its own best file
  let lastTarget: string | undefined
  $effect(() => {
    const id = targetId
    if (id !== lastTarget) {
      if (lastTarget !== undefined) fileId = null
      lastTarget = id
    }
  })

  const items = $derived([
    ...fitting.map((f) => ({ id: f.id, f, group: others.length && target ? t('For {name}', { name: target.name }) : undefined })),
    ...others.map((f) => ({ id: f.id, f, group: t('Other versions') })),
  ])

  function fileMeta(f: CfFile): string {
    const parts = [CHANNEL[f.channel].word]
    if (f.gameVersions.length) parts.push(mcRange(f.gameVersions))
    if (kind === 'mod' && f.loaders.length) parts.push(f.loaders.map(loaderLabel).join(', '))
    return parts.join(' · ')
  }

  // plan (mods, shader packs, resource packs)
  let plan = $state<CfPlan | null>(null)
  let planError = $state<string | null>(null)
  const cfFiles = $derived(targetId ? app.cfInstalled[targetId] : undefined)

  $effect(() => {
    const id = targetId
    const file = chosen?.id
    if (isDatapack || !id || !file) return
    void cfFiles // plan again once the instance's content changes
    let stale = false
    plan = null
    planError = null
    api
      .curseforgePlan(id, kind, project.id, file)
      .then((p) => {
        if (!stale) plan = p
      })
      .catch((e) => {
        if (!stale) planError = message(e)
      })
    return () => {
      stale = true
    }
  })

  const current = $derived(plan && target && chosen && plan.instanceId === target.id && plan.items[0]?.fileId === chosen.id ? plan : null)
  const main = $derived(current?.items[0] ?? null)
  const deps = $derived(current ? current.items.slice(1) : [])
  const manualDeps = $derived(deps.filter((d) => d.manualUrl))

  // install
  const running = $derived(!!target && !!app.running[target.id])
  const task = $derived(target ? app.taskFor(`install:${target.id}`) : undefined)
  let busy = $state(false)
  let failed = $state<string | null>(null)
  /** "<instance>/<file>" (or "<instance>/<world>/<file>") installed from this panel. */
  let done = $state<string[]>([])
  const key = $derived(isDatapack ? `${targetId}/${app.targetWorld}/${chosen?.id}` : `${targetId}/${chosen?.id}`)
  /** Installed, with nothing left that Cubby could download. */
  const isDone = $derived(done.includes(key) || (!isDatapack && !!main?.installed && !deps.some((d) => !d.manualUrl)))

  async function install() {
    if (!target || !chosen) return
    const k = key
    busy = true
    failed = null
    try {
      if (isDatapack) {
        if (!app.targetWorld) return
        const file = await api.curseforgeInstallDatapack(target.id, app.targetWorld, project.id, chosen.id)
        if (replaces && replaces !== file) await api.removeDatapack(target.id, app.targetWorld, replaces).catch(() => {})
        app.toast(t('{title} is in the world. It loads the next time you open it.', { title: project.title }), 'success')
      } else {
        onInstall?.(target.id)
        const ran = await api.curseforgeInstall(target.id, kind, project.id, chosen.id)
        const left = ran.items.filter((i) => i.manualUrl && !i.installed).length
        if (left) app.toast(t('{title} is in {name}. {n} still need a download on CurseForge.', { title: project.title, name: target.name, n: plural(left, 'file', 'files') }), 'info')
        else app.toast(t('{title} is in {name}.', { title: project.title, name: target.name }), 'success')
      }
      done = [...done, k]
    } catch (e) {
      failed = message(e)
    } finally {
      busy = false
    }
  }

  function manage() {
    if (!target) return
    onLeave?.()
    if (isDatapack) app.openInstance(target.id, 'worlds')
    else if (words.tab) app.openInstance(target.id, words.tab)
  }

  const canInstall = $derived(
    !!target && !!chosen && chosen.downloadable && !running && !busy && !task && (isDatapack ? !!app.targetWorld : !!current),
  )
</script>

<section class="panel" aria-label={t('Install')}>
  {#if !compact}<h2 class="text-heading">{t('Install')}</h2>{/if}
  <div class="field">
    <span class="text-label muted">{t('Install to')}</span>
    <TargetPicker />
  </div>
  {#if isDatapack}
    <div class="field">
      <span class="text-label muted">{t('Into world')}</span>
      <WorldPicker />
    </div>
  {/if}

  {#if !target}
    <p class="text-small muted">{t('Create an instance first: {many} go into one.', { many: words.many })}</p>
  {:else}
    {#if filesError}
      {#snippet retry()}{#if !all}<Button size="sm" icon="update" onclick={() => reloads++}>{t('Retry')}</Button>{/if}{/snippet}
      <Banner tone="danger" title={t('Couldn’t load the files')} actions={retry}>{filesError}</Banner>
    {:else if usable === null}
      <p class="text-small muted">{t('Loading files…')}</p>
    {:else if !usable.length}
      <p class="text-small muted">
        {everyVersion || all
          ? t('{title} has no file Cubby can install.', { title: project.title })
          : t('{title} has no file for Minecraft {mc}.', { title: project.title, mc: target.minecraft })}
      </p>
      {#if !everyVersion && !all}<Button size="sm" onclick={() => (everyVersion = true)}>{t('Show files for every version')}</Button>{/if}
    {:else if chosen}
      <div class="field">
        <span class="text-label muted">{t('File')}</span>
        <Picker {items} value={chosen.id} onSelect={(id) => (fileId = id)} label={t('File')} listWidth={380}>
          {#snippet trigger()}
            <span class="stack">
              <span class="text-body-strong">{chosen.name}</span>
              <span class="text-small muted"><span class="text-mono">{fileMeta(chosen)}</span></span>
            </span>
          {/snippet}
          {#snippet option(item)}
            <span class="stack">
              <span class="text-body-strong">{item.f.name}</span>
              <span class="text-small muted"><span class="text-mono">{fileMeta(item.f)}</span>{#if item.f.released} · {relativeTime(item.f.released, '')}{/if}</span>
            </span>
          {/snippet}
        </Picker>
        {#if !everyVersion && !all}
          <button type="button" class="link text-small" onclick={() => (everyVersion = true)}>{t('Show files for every version')}</button>
        {/if}
      </div>

      {#if !chosenFits}
        <p class="warn text-small line"><Icon name="warning" size={18} />
          {kind === 'mod' ? t('This file isn’t made for {name}: the game may not start with it.', { name: target.name }) : t('This file isn’t made for Minecraft {mc}. It may not load.', { mc: target.minecraft })}
        </p>
      {/if}

      {#if !chosen.downloadable}
        <Banner tone="warning" title={t('Only on CurseForge')}>
          {t('Its author only lets it be downloaded on curseforge.com. Download it there, then add it to {name}.', { name: target.name })}
        </Banner>
        <Button variant="primary" iconEnd="chevron-right" onclick={() => openPage(`${pageUrl}/download/${chosen.id}`)}>{t('Download on CurseForge')}</Button>
      {:else}
        {#if !isDatapack}
          <section class="block" aria-busy={!current && !planError}>
            {#if planError}
              <p class="text-small bad line"><Icon name="error" size={18} />{t('Couldn\'t check it: {planError}', { planError })}</p>
            {:else if !current}
              <span class="placeholder" aria-hidden="true"></span>
            {:else}
              {#if main?.replaces && !main.installed}
                <p class="text-small muted line"><Icon name="update" size={18} />{t('{name} has another version:', { name: target.name })} <span class="text-mono">{main.replaces}</span>{t('. Cubby replaces it.')}</p>
              {/if}
              {#if deps.length || current.missing.length}
                <h3 class="text-label muted">{t('Also needed')}</h3>
                <ul class="deps">
                  {#each deps as d (d.projectId)}
                    <li>
                      <InstanceIcon {...d.iconUrl ? { src: d.iconUrl } : projectArt(d.title)} size={24} />
                      <span class="stack">
                        <span class="text-body-strong">{d.title}</span>
                        <span class="text-small muted">{d.manualUrl ? t('Only on CurseForge: download it by hand') : t('Added with it · {size}', { size: bytes(d.size) })}</span>
                      </span>
                      {#if d.manualUrl}<Button size="sm" iconEnd="chevron-right" onclick={() => openPage(d.manualUrl!)}>{t('Download')}</Button>{/if}
                    </li>
                  {/each}
                  {#each current.missing as m (m.projectId)}
                    <li>
                      <span class="ic bad"><Icon name="error" size={18} /></span>
                      <span class="stack">
                        <span class="text-body-strong">{m.title}</span>
                        <span class="text-small bad">{t('No file for {name}', { name: target.name })}</span>
                      </span>
                      <Button size="sm" iconEnd="chevron-right" onclick={() => openPage(m.pageUrl)}>{t('Its page')}</Button>
                    </li>
                  {/each}
                </ul>
              {:else if !main?.installed}
                <p class="text-small muted line"><Icon name="check" size={18} />{t('Nothing else needed: everything it requires is already in {name}.', { name: target.name })}</p>
              {/if}
              {#if current.downloadBytes > 0}
                <p class="text-small muted"><span class="text-body-strong ink">{t('{n} to download', { n: bytes(current.downloadBytes) })}</span>{manualDeps.length ? t(', and {n} to get from CurseForge.', { n: plural(manualDeps.length, 'file', 'files') }) : '.'}</p>
              {/if}
            {/if}
          </section>
        {/if}

        {#if task && !isDatapack}
          <div class="progress" aria-live="polite">
            <ProgressBar tone="diamond" label={task.label} value={task.total ? Math.round((task.done / task.total) * 100) : undefined} />
            <p class="text-small muted">{t('Installing to {name}. You can keep browsing.', { name: target.name })}</p>
          </div>
        {:else if isDone}
          <div class="row">
            <span class="ok text-small line"><Icon name="check" size={18} />{isDatapack ? t('In the world') : t('In {name}', { name: target.name })}</span>
            {#if (isDatapack || words.tab) && !inInstance}
              <Button size="sm" variant="ghost" iconEnd="chevron-right" onclick={manage}>{isDatapack ? t('See its datapacks') : t('Manage {many} in {name}', { many: words.many, name: target.name })}</Button>
            {/if}
          </div>
        {:else}
          {#if failed}<Banner tone="danger" title={t('Couldn\'t install {title}', { title: project.title })}>{failed}</Banner>{/if}
          <Button variant="primary" size="lg" icon={main?.replaces ? 'update' : 'download'} busy={busy} disabled={!canInstall} onclick={install} class="wide">
            {isDatapack ? t('Install · {size}', { size: bytes(chosen.size ?? 0) }) : t('Install to {name}', { name: target.name })}
          </Button>
          {#if running}<p class="text-small muted">{t('Close the game first.')}</p>{/if}
        {/if}
      {/if}
    {/if}
  {/if}

  {#if compact}<Button variant="ghost" size="sm" iconEnd="chevron-right" onclick={() => openPage(pageUrl)}>{t('See it on CurseForge')}</Button>{/if}
</section>

<style>
  p,
  h2,
  h3,
  ul {
    margin: 0;
  }
  .panel,
  .field,
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .field {
    gap: var(--space-1);
  }
  .block {
    gap: var(--space-2);
  }
  .stack {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .line {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .line :global(.cb-icon) {
    flex: none;
    margin-top: 2px;
  }
  .muted {
    color: var(--ink-muted);
  }
  .ink {
    color: var(--ink);
  }
  .warn {
    color: var(--gold-text);
  }
  .ok {
    color: var(--grass-text);
  }
  .bad {
    color: var(--redstone-text);
  }
  .link {
    align-self: flex-start;
    padding: 0;
    color: var(--grass-text);
    text-decoration: underline;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .deps {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0;
    list-style: none;
    background: var(--line);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .deps li {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    padding: 9px var(--space-3);
    background: var(--surface);
  }
  .ic {
    display: flex;
    justify-content: center;
  }
  .placeholder {
    display: block;
    height: 48px;
    background: var(--surface);
    border: var(--border-block) dashed var(--line);
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .panel :global(.wide) {
    width: 100%;
  }
</style>
