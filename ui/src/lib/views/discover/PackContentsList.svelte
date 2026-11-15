<!-- The mods, resource packs and shader packs one version of a modpack installs, for the Modrinth project page and the
     pack page of the other sources, in the rows of an instance's Mods tab (without the switch). A row opens the
     project's page in Cubby (Modrinth, CurseForge) or in the browser. -->
<script lang="ts">
  import { t, tn } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, Icon, TextField, type IconName } from '../../cubby'
  import { fold, num } from '../../format'
  import { app } from '../../state.svelte'
  import type { PackContentItem, PackContentKind, PackContents, ProviderId } from '../../types'
  import { loadPackContents } from './packContents'

  interface Props {
    provider: ProviderId
    packId: string
    /** The version shown; nothing loads without one. */
    versionId: string | null
    /** Shown in the line above the list. */
    versionName?: string | null
    /** id of the heading that names the section, when the page has one. */
    labelledby?: string
    /** The page is still loading the versions. */
    loadingVersions?: boolean
  }

  let { provider, packId, versionId, versionName = null, labelledby, loadingVersions = false }: Props = $props()

  /** Widths (%) of the name, version and description bars of the placeholder rows, so they don't look stamped. */
  const SKELETON: [number, number, number][] = [
    [22, 9, 62],
    [30, 7, 48],
    [18, 11, 70],
    [26, 8, 40],
    [34, 6, 56],
    [20, 10, 66],
  ]

  /** More than this many, and a search box helps. */
  const SEARCH_FROM = 12

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  let contents = $state<PackContents | null>(null)
  let error = $state<string | null>(null)
  let tries = $state(0)
  let query = $state('')
  /** Rows shown before "Show all", like an instance's Mods tab. */
  const PAGE = 60
  let limit = $state(PAGE)

  $effect(() => {
    const v = versionId
    const p = provider
    const id = packId
    const again = tries > 0
    contents = null
    error = null
    query = ''
    limit = PAGE
    if (!v) return
    let stale = false
    loadPackContents(p, id, v, again)
      .then((c) => {
        if (!stale) contents = c
      })
      .catch((e) => {
        if (!stale) error = message(e)
      })
    return () => {
      stale = true
    }
  })

  const GROUPS: { kind: PackContentKind; title: string }[] = [
    { kind: 'mod', title: t('Mods') },
    { kind: 'resourcepack', title: t('Resource packs') },
    { kind: 'shader', title: t('Shader packs') },
  ]

  const items = $derived(contents?.items ?? [])
  const needle = $derived(fold(query.trim()))
  const shown = $derived(needle ? items.filter((i) => fold(`${i.name} ${i.fileName ?? ''} ${i.description ?? ''} ${i.authors.join(' ')}`).includes(needle)) : items)
  const visible = $derived(shown.slice(0, limit))
  const mixed = $derived(items.some((i) => i.kind !== 'mod'))
  const optionalCount = $derived(items.filter((i) => i.optional).length)

  const summary = $derived.by(() => {
    const count = (k: PackContentKind) => items.filter((i) => i.kind === k).length
    const parts = [tn(count('mod'), 'mod', 'mods')]
    if (count('resourcepack')) parts.push(tn(count('resourcepack'), 'resource pack', 'resource packs'))
    if (count('shader')) parts.push(tn(count('shader'), 'shader pack', 'shader packs'))
    return parts.join(' · ')
  })

  /** Opens the project in Cubby when it can, else its web page. */
  function open(i: PackContentItem) {
    if (i.site === 'modrinth' && i.projectId) app.openProject(i.projectId, i.kind, undefined, i.name)
    else if (i.site === 'curseforge' && i.projectId) app.openProject(i.projectId, i.kind, 'curseforge', i.name)
    else if (i.pageUrl) void api.openUrl(i.pageUrl).catch((e) => app.error(e))
  }

  const openable = (i: PackContentItem) => !!((i.site && i.projectId) || i.pageUrl)
  const inCubby = (i: PackContentItem) => !!(i.site && i.projectId)

  /** What kind of mod it is, from its categories, as in an instance's Mods tab. */
  function modKind(i: PackContentItem): { icon: IconName; label: string } | null {
    if (i.kind !== 'mod' || !i.categories.length) return null
    if (i.categories.includes('library')) return { icon: 'wrench', label: t('Library') }
    if (i.categories.includes('optimization')) return { icon: 'memory', label: t('Performance') }
    return { icon: 'cube', label: t('Content') }
  }

  function openLabel(i: PackContentItem): string {
    return inCubby(i) ? t('Open {name} in Cubby', { name: i.name }) : t('Open the page of {name} in your browser', { name: i.name })
  }
</script>

<div class="pc" role="region" aria-labelledby={labelledby} aria-label={labelledby ? undefined : t('Content')} aria-busy={(!!versionId && !contents && !error) || (!versionId && loadingVersions)}>
  {#if !versionId && !loadingVersions}
    <p class="text-small muted">{t('This pack has no version to look into yet.')}</p>
  {:else if error}
    <div class="empty">
      <p class="text-body bad line"><Icon name="error" size={18} />{t('Couldn\'t load the contents of this version. {error}', { error })}</p>
      <Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>
    </div>
  {:else if !versionId || !contents}
    <div class="head">
      <p class="text-small muted" role="status">{versionId ? t('Reading the pack\'s list of files…') : t('Loading the versions…')}</p>
    </div>
    <div class="list skeleton" aria-hidden="true">
      {#each SKELETON as w, i (i)}
        <div class="cb-mod sk-row" style:--delay="{i * 120}ms">
          <span class="cb-mod__icon sk-block"></span>
          <div class="cb-mod__body sk-body">
            <span class="sk-head"><span class="sk-bar sk-name" style:width="{w[0]}%"></span><span class="sk-bar sk-version" style:width="{w[1]}%"></span></span>
            <span class="sk-bar sk-desc" style:width="{w[2]}%"></span>
          </div>
        </div>
      {/each}
    </div>
  {:else if contents.unavailable}
    <div class="empty"><p class="text-body line"><Icon name="info" size={18} />{t(contents.unavailable)}</p></div>
  {:else if !items.length}
    <div class="empty"><p class="text-body">{t('This version doesn\'t list any mods.')}</p></div>
  {:else}
    <div class="head">
      <p class="text-small muted">
        {#if versionName}{t('Version')} <span class="text-mono">{versionName}</span>{' · '}{/if}{summary}{#if optionalCount}{' · '}{tn(optionalCount, 'optional', 'optional')}{/if}
      </p>
      {#if items.length > SEARCH_FROM}
        <TextField label={t('Search the contents')} hideLabel icon="search" type="search" size="sm" placeholder={t('Search {n}', { n: tn(items.length, 'item', 'items') })} autocomplete="off" bind:value={query} class="search" />
      {/if}
    </div>

    {#if !shown.length}
      <div class="empty">
        <p class="text-body">{t('Nothing in this version matches “{query}”.', { query: query.trim() })}</p>
        <Button size="sm" onclick={() => (query = '')}>{t('Clear the search')}</Button>
      </div>
    {:else}
      {#each GROUPS as g (g.kind)}
        {@const list = visible.filter((i) => i.kind === g.kind)}
        {#if list.length}
          <section class="group" aria-label={g.title}>
            {#if mixed}<h3 class="cb-ins-eyebrow">{g.title} <span class="muted">{num(shown.filter((i) => i.kind === g.kind).length)}</span></h3>{/if}
            <div class="list">
              {#each list as i, n (n + i.name + (i.fileName ?? ''))}
                {@const kind = modKind(i)}
                <div class={['row', openable(i) && 'is-link']}>
                  {#if openable(i)}
                    <!-- Stretched over the whole row: a click anywhere on it opens the project. -->
                    <button type="button" class="row__hit" aria-label={openLabel(i)} onclick={() => open(i)}></button>
                  {/if}
                  <div class="cb-mod">
                    <span class="cb-mod__icon">{#if i.iconUrl}<img src={i.iconUrl} alt="" loading="lazy" />{:else}<Icon name="mods" size={18} />{/if}</span>
                    <div class="cb-mod__body">
                      <div class="cb-mod__head">
                        <span class="cb-mod__name text-body-strong">{i.name}</span>
                        {#if i.version ?? i.fileName}<span class="cb-mod__version text-mono" title={i.fileName ?? undefined}>{i.version ?? i.fileName}</span>{/if}
                        {#if i.authors.length}<span class="cb-mod__authors text-small">{t('by {authors}', { authors: i.authors.join(', ') })}</span>{/if}
                        {#if i.optional}<span class="text-small tag">{t('optional')}</span>{/if}
                        {#if kind}
                          <span class="cb-mod__meta">
                            <span class="cb-mod__kind" title={kind.label}><Icon name={kind.icon} size={18} /><span class="cb-visually-hidden">{kind.label}</span></span>
                          </span>
                        {/if}
                      </div>
                      {#if i.description}<p class="cb-mod__desc text-small">{i.description}</p>{/if}
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      {/each}
      {#if shown.length > visible.length}
        <div class="more">
          <span class="muted text-small">{t('Showing {n} of {n2}', { n: num(visible.length), n2: num(shown.length) })}</span>
          <Button size="sm" variant="ghost" icon="chevron-down" onclick={() => (limit = Infinity)}>{t('Show all {n}', { n: num(shown.length) })}</Button>
        </div>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .pc {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2) var(--space-4);
  }
  .head :global(.search) {
    flex: 0 1 260px;
    min-width: 180px;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .group h3 {
    margin: 0;
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
  .row__hit {
    all: unset;
    position: absolute;
    inset: 0;
    z-index: 1;
    cursor: pointer;
  }
  .row__hit:focus-visible {
    box-shadow: inset 0 0 0 3px var(--focus);
  }
  .row :global(.cb-mod) {
    border-bottom: 0;
  }
  .row.is-link:hover :global(.cb-mod) {
    background: var(--surface-hover);
  }
  .row.is-link:hover :global(.cb-mod__name) {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .more {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .tag {
    flex: none;
    padding: 0 6px;
    color: var(--ink-muted);
    border: var(--border-hair) solid var(--line);
  }
  .muted {
    color: var(--ink-muted);
  }
  .bad {
    color: var(--redstone-text);
  }
  .line {
    display: flex;
    align-items: flex-start;
    gap: var(--space-1);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .empty p {
    margin: 0;
  }
  .head p {
    margin: 0;
  }
  /* placeholder rows: the shape of the real ones, pulsing in two steps like the rest of Cubby */
  .sk-row {
    border-bottom: 0;
    animation: sk-pulse 1.6s steps(2, jump-none) infinite;
    animation-delay: var(--delay);
  }
  .sk-row + .sk-row {
    border-top: var(--border-hair) solid var(--line);
  }
  .sk-block {
    background: var(--bg-chrome);
  }
  .sk-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 3px;
  }
  .sk-head {
    display: flex;
    gap: var(--space-2);
  }
  .sk-bar {
    display: block;
    height: 12px;
    background: var(--line);
  }
  .sk-name {
    height: 14px;
    background: var(--ink-muted);
    opacity: 0.35;
  }
  .sk-version,
  .sk-desc {
    opacity: 0.8;
  }
  @keyframes sk-pulse {
    50% {
      opacity: 0.55;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sk-row {
      animation: none;
    }
  }
</style>
