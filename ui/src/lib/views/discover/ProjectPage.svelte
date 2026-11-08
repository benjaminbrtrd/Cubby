<!-- A Modrinth project: one page for mods, modpacks, shader packs and resource packs, with the install
     panel on the side. Everything shown comes from Modrinth or from the instance; nothing is estimated.
     Its parts live in ./project: the header, one component per tab and the side panel. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon, Tabs } from '../../cubby'
  import { num } from '../../format'
  import { app } from '../../state.svelte'
  import type { Project, ProjectKind, ProjectVersion } from '../../types'
  import { KINDS, bestVersion, fits } from './discover'
  import ChangelogTab from './project/ChangelogTab.svelte'
  import DependenciesTab from './project/DependenciesTab.svelte'
  import DescriptionTab from './project/DescriptionTab.svelte'
  import GalleryTab from './project/GalleryTab.svelte'
  import ProjectAside from './project/ProjectAside.svelte'
  import ProjectHeader from './project/ProjectHeader.svelte'
  import VersionsTab from './project/VersionsTab.svelte'

  interface Props {
    idOrSlug: string
    kind: ProjectKind
  }

  let { idOrSlug, kind: routeKind }: Props = $props()

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  // data
  let project = $state<Project | null>(null)
  let error = $state<string | null>(null)
  let versions = $state<ProjectVersion[] | null>(null)
  let versionsError = $state<string | null>(null)
  let tries = $state(0)

  $effect(() => {
    const id = idOrSlug
    void tries
    let stale = false
    error = null
    versionsError = null
    api
      .getProject(id)
      .then((p) => {
        if (!stale) project = p
      })
      .catch((e) => {
        if (!stale) error = message(e)
      })
    api
      .listProjectVersions(id, null, null)
      .then((v) => {
        if (!stale) versions = v
      })
      .catch((e) => {
        if (!stale) versionsError = message(e)
      })
    return () => {
      stale = true
    }
  })

  // Modrinth calls most datapacks "mod" projects: a page opened as a datapack stays one.
  const kind = $derived(routeKind === 'datapack' ? 'datapack' : (project?.kind ?? routeKind))
  const words = $derived(KINDS[kind])
  const target = $derived(kind === 'modpack' ? undefined : app.target)

  $effect(() => {
    if (target) void app.loadSources(target.id)
  })

  /** The version shown in the install panel (null = the best one for the target). */
  let versionId = $state<string | null>(null)
  const best = $derived(versions ? bestVersion(versions, kind, target) : undefined)
  const chosen = $derived(versions?.find((v) => v.id === versionId) ?? best ?? versions?.[0])
  const deps = $derived(chosen?.dependencies ?? [])

  /** Versions and Changelog share the "Only for <instance>" switch. */
  let onlyFits = $state(true)
  const listed = $derived(versions ? (onlyFits && target ? versions.filter((v) => fits(v, kind, target)) : versions) : [])

  type Tab = 'description' | 'gallery' | 'versions' | 'changelog' | 'dependencies'
  let tab = $state<Tab>('description')
  const tabs = $derived.by(() => {
    const out: { id: Tab; label: string; count?: string; panel: string }[] = [{ id: 'description', label: t('Description'), panel: 'pp-description' }]
    if (project?.gallery.length) out.push({ id: 'gallery', label: t('Gallery'), count: num(project.gallery.length), panel: 'pp-gallery' })
    out.push({ id: 'versions', label: t('Versions'), count: project ? num(Math.max(project.versionCount, versions?.length ?? 0)) : undefined, panel: 'pp-versions' })
    out.push({ id: 'changelog', label: t('Changelog'), panel: 'pp-changelog' })
    if (kind === 'mod') out.push({ id: 'dependencies', label: t('Dependencies'), count: num(deps.filter((d) => d.kind !== 'embedded').length), panel: 'pp-dependencies' })
    return out
  })

  function backToDiscover(sameKind: boolean) {
    if (sameKind && app.discover.kind !== kind) {
      app.discover.kind = kind
      app.discover.categories = []
      app.discover.environment = null
      app.discover.page = 0
    }
    app.navigate({ name: 'discover' })
  }
</script>

<div class="screen">
  <div class="layout">
    <div class="main">
      <nav aria-label={t('Breadcrumb')}>
        <ol class="crumbs">
          <li><button type="button" class="link text-body-strong" onclick={() => backToDiscover(false)}>{t('Discover')}</button></li>
          <li aria-hidden="true" class="sep"><Icon name="chevron-right" size={18} /></li>
          <li><button type="button" class="link text-body" onclick={() => backToDiscover(true)}>{words.label}</button></li>
          <li aria-hidden="true" class="sep"><Icon name="chevron-right" size={18} /></li>
          <li class="text-body" aria-current="page">{project?.title ?? idOrSlug}</li>
        </ol>
      </nav>

      {#if error && !project}
        {#snippet errorActions()}
          <Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>
          <Button size="sm" variant="ghost" onclick={() => backToDiscover(false)}>{t('Back to Discover')}</Button>
        {/snippet}
        <Banner tone="warning" icon="offline" title={t('Couldn\'t open this {one}', { one: words.one })} actions={errorActions}>
          {t('{error}. Check your internet connection, then try again.', { error: error.replace(/\.\s*$/, '') })}
        </Banner>
      {:else if !project}
        <div class="header-skeleton" aria-hidden="true">
          <span class="sk-icon"></span>
          <span class="sk-lines"><span class="sk w40 tall"></span><span class="sk w90"></span><span class="sk w60"></span></span>
        </div>
        <span class="cb-visually-hidden" role="status">{t('Loading the {one}', { one: words.one })}</span>
      {:else}
        <ProjectHeader {project} {kind} {chosen} />

        <Tabs label={t('{label} sections', { label: words.label })} items={tabs} value={tab} onChange={(id) => (tab = id as Tab)} />

        {#if tab === 'description'}
          <DescriptionTab {project} {kind} {target} {chosen} {deps} />
        {:else if tab === 'gallery'}
          <GalleryTab {project} />
        {:else if tab === 'versions'}
          <VersionsTab
            {kind}
            {target}
            {versions}
            {versionsError}
            {listed}
            {best}
            {chosen}
            bind:onlyFits
            onRetry={() => tries++}
            onChoose={(v) => (versionId = v.id)}
          />
        {:else if tab === 'changelog'}
          <ChangelogTab {target} {versions} {versionsError} {listed} bind:onlyFits />
        {:else if tab === 'dependencies'}
          <DependenciesTab {target} {chosen} {deps} loading={!versions && !versionsError} />
        {/if}
      {/if}
    </div>

    <aside aria-label={t('Install {title}', { title: project?.title ?? '' })}>
      {#if project}
        <ProjectAside {project} {kind} {versions} {versionsError} bind:versionId />
      {:else}
        <span class="aside-skeleton" aria-hidden="true"></span>
      {/if}
    </aside>
  </div>
</div>

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
    align-items: stretch;
    min-width: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  aside {
    flex: none;
    min-height: 0;
    overflow: auto;
    box-sizing: border-box;
    width: var(--size-inspector);
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }

  /* Shared by the parts in ./project: plain text, tones, links and the tab panels. */
  .layout :global(:is(h1, h2, h3, p, ol, ul, dl, dd, figure)) {
    margin: 0;
  }
  .layout :global(.muted) {
    color: var(--ink-muted);
  }
  .layout :global(:is(.good, .ok)) {
    color: var(--grass-text);
  }
  .layout :global(.bad) {
    color: var(--redstone-text);
  }
  .layout :global(.busy) {
    color: var(--diamond-text);
  }
  .layout :global(.grow) {
    flex: 1;
  }
  .layout :global(.link) {
    padding: 0;
    font: inherit;
    color: var(--grass-text);
    text-align: start;
    text-decoration: underline;
    text-underline-offset: 3px;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .layout :global(.link:hover) {
    color: var(--ink);
  }
  .layout :global(:is(.link, a):focus-visible) {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .layout :global(.status) {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .layout :global(.panel) {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
    margin-top: var(--space-1);
  }
  .layout :global(.panel > .cb-btn) {
    align-self: flex-start;
  }
  .layout :global(.panel-head) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
  }
  .layout :global(.empty) {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .layout :global(.table-skeleton) {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .layout :global(.sk) {
    display: block;
    height: 14px;
    background: var(--surface);
  }
  .layout :global(.sk.tall) {
    height: 28px;
  }

  /* breadcrumb and loading */
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 0;
    list-style: none;
  }
  .crumbs .link {
    text-decoration: none;
  }
  .crumbs .link:hover {
    text-decoration: underline;
  }
  .sep {
    display: flex;
    color: var(--ink-muted);
  }
  .header-skeleton {
    display: flex;
    gap: var(--space-5);
  }
  .sk-icon {
    flex: none;
    width: 64px;
    height: 64px;
    background: var(--surface);
  }
  .sk-lines {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    flex: 1;
  }
  .w40 {
    width: 40%;
  }
  .w60 {
    width: 60%;
  }
  .w90 {
    width: 90%;
  }
  .aside-skeleton {
    display: block;
    height: 320px;
    background: var(--surface);
  }

  @media (max-width: 1100px) {
    aside {
      width: 300px;
      padding-inline: var(--space-4);
    }
  }
</style>
