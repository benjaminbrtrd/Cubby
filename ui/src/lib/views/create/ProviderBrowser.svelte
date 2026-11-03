<!-- One modpack provider (ATLauncher, CurseForge, FTB, Modrinth, Technic): search, sort, results, and the pack page. -->
<script lang="ts">
  import { api } from '../../api'
  import { Banner, Button, Icon, InstanceIcon, MetaChip, TextField } from '../../cubby'
  import { compactNum, loaderName, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { PackProvider, PackSort, PackSummary } from '../../types'
  import Picker from '../discover/Picker.svelte'
  import PackPage from './PackPage.svelte'
  import { mcSpan } from './packs'

  interface Props {
    provider: PackProvider
  }

  let { provider }: Props = $props()

  const PAGE = 20
  const SORT_LABEL: Record<PackSort, string> = {
    relevance: 'Relevance',
    downloads: 'Most downloaded',
    updated: 'Recently updated',
    newest: 'Newest',
    name: 'Name',
  }

  const sorts = $derived(provider.sorts.map((id) => ({ id, label: SORT_LABEL[id] })))
  let picked = $state<PackSort | null>(null)
  const sort = $derived<PackSort>(picked && provider.sorts.includes(picked) ? picked : (provider.sorts[0] ?? 'relevance'))
  const sortLabel = $derived(SORT_LABEL[sort])

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  // search

  let text = $state('')
  let query = $state('')
  $effect(() => {
    const q = text
    if (q === query) return
    const t = setTimeout(() => (query = q), 250)
    return () => clearTimeout(t)
  })

  let hits = $state<PackSummary[]>([])
  let total = $state<number | null>(null)
  let loading = $state(true)
  let more = $state(false)
  let error = $state<string | null>(null)
  let moreError = $state<string | null>(null)
  let retries = $state(0)
  let lastPage = $state(0)
  let seq = 0

  $effect(() => {
    if (!provider.available) return
    const q = query.trim()
    const s = sort
    void retries
    const mine = ++seq
    loading = true
    error = null
    moreError = null
    api
      .packSearch(provider.id, q, s, 0)
      .then((page) => {
        if (mine !== seq) return
        hits = page.hits
        total = page.total
        lastPage = page.hits.length
        loading = false
      })
      .catch((e) => {
        if (mine !== seq) return
        hits = []
        error = message(e)
        loading = false
      })
  })

  const hasMore = $derived(provider.paged && hits.length > 0 && (total != null ? hits.length < total : lastPage >= PAGE))

  async function loadMore() {
    if (more) return
    const mine = seq
    more = true
    moreError = null
    try {
      const page = await api.packSearch(provider.id, query.trim(), sort, hits.length)
      if (mine !== seq) return
      hits = [...hits, ...page.hits.filter((h) => !hits.some((x) => x.id === h.id))]
      total = page.total ?? total
      lastPage = page.hits.length
    } catch (e) {
      if (mine === seq) moreError = message(e)
    } finally {
      more = false
    }
  }

  const offline = $derived(!!error && (/(network|connect|dns|resolve|timed? ?out|offline|unreachable|socket|tls|fetch)/i.test(error) || !navigator.onLine))

  // the pack page

  let selected = $state<PackSummary | null>(null)

  const art = (h: PackSummary) => (h.iconUrl ? { src: h.iconUrl } : projectArt(h.name))
</script>

{#if selected}
  <PackPage {provider} summary={selected} onBack={() => (selected = null)} />
{:else}
  <div class="browser">
    <div class="top">
      <h1 class="text-title">{provider.name}</h1>
      {#if provider.available}
        <div class="search">
          <TextField label="Search {provider.name} modpacks" hideLabel icon="search" placeholder="Search {provider.name} modpacks" bind:value={text} />
        </div>
        <span class="grow"></span>
        {#if sorts.length > 1}
          <Picker items={sorts} value={sort} onSelect={(id) => (picked = id as PackSort)} label="Sort by" buttonLabel="Sort by {sortLabel}. Change" variant="ghost" listWidth={200}>
            {#snippet trigger()}<span class="text-body">Sort: <span class="text-body-strong ink">{sortLabel}</span></span>{/snippet}
            {#snippet option(item, on)}<span class={['text-body', on && 'text-body-strong']}>{item.label}</span>{/snippet}
          </Picker>
        {/if}
      {/if}
    </div>

    {#if !provider.available}
      <div class="setup">
        <span class="setup__ic" aria-hidden="true"><Icon name="key" size={27} /></span>
        <h2 class="text-heading">{provider.name} needs setup</h2>
        <p class="text-body muted">{provider.reason ?? `${provider.name} isn't available right now.`}</p>
        {#if provider.id === 'curseforge'}
          <Button variant="primary" icon="settings" onclick={() => app.navigate({ name: 'settings', section: 'accounts' })}>Open Settings</Button>
        {/if}
      </div>
    {:else}
      <p class="text-small muted summary" aria-live="polite">
        {#if loading}
          Searching {provider.name}…
        {:else if error}
          &nbsp;
        {:else if query.trim()}
          {total != null ? plural(total, 'pack', 'packs') : plural(hits.length, 'pack', 'packs')} for “{query.trim()}”.
        {:else}
          {total != null ? `${plural(total, 'pack', 'packs')} on ${provider.name}.` : `Packs on ${provider.name}.`}
          Each one installs as a new instance.
        {/if}
      </p>

      <div class="results" aria-busy={loading}>
        {#if error}
          {#snippet retry()}<Button size="sm" icon="update" onclick={() => retries++}>Try again</Button>{/snippet}
          <Banner tone="warning" icon={offline ? 'offline' : 'warning'} title={offline ? `Can't reach ${provider.name}` : "Search didn't work"} actions={retry}>
            {offline ? 'Check your internet connection, then try again.' : error}
          </Banner>
        {:else if loading}
          <ul class="list" aria-hidden="true">
            {#each { length: 6 }, i (i)}
              <li class="row skeleton"><span class="sk-icon"></span><span class="sk-lines"><span class="sk w40"></span><span class="sk w90"></span><span class="sk w60"></span></span></li>
            {/each}
          </ul>
          <span class="cb-visually-hidden" role="status">Loading packs</span>
        {:else if !hits.length}
          <div class="empty">
            <span class="setup__ic" aria-hidden="true"><Icon name="search" size={27} /></span>
            <h2 class="text-heading">No packs found</h2>
            <p class="text-body muted">
              {query.trim() ? `Nothing on ${provider.name} matches “${query.trim()}”. Check the spelling or try fewer words.` : `${provider.name} has no packs to show right now.`}
            </p>
            {#if query.trim()}<Button onclick={() => (text = '')}>Clear search</Button>{/if}
          </div>
        {:else}
          <ul class="list">
            {#each hits as h (h.id)}
              <li>
                <button type="button" class="row" onclick={() => (selected = h)} aria-label="Open {h.name}">
                  <InstanceIcon {...art(h)} size={48} raised />
                  <span class="text">
                    <span class="line1">
                      <span class="text-body-strong">{h.name}</span>
                      {#if h.authors.length}<span class="text-small muted">by {h.authors.slice(0, 2).join(', ')}{h.authors.length > 2 ? ' and others' : ''}</span>{/if}
                    </span>
                    {#if h.summary}<span class="text-body muted summ">{h.summary}</span>{/if}
                    <span class="chips">
                      {#if h.minecraft.length}<MetaChip>{mcSpan(h.minecraft)}</MetaChip>{/if}
                      {#if h.loaders.length}<MetaChip>{h.loaders.map((l) => loaderName(l)).join(', ')}</MetaChip>{/if}
                      {#if h.downloads != null}<span class="text-small muted stat"><Icon name="download" size={18} />{compactNum(h.downloads)}</span>{/if}
                      {#if h.updated}<span class="text-small muted stat"><Icon name="clock" size={18} />{relativeTime(h.updated, '')}</span>{/if}
                    </span>
                  </span>
                  <span class="go" aria-hidden="true"><Icon name="chevron-right" size={18} /></span>
                </button>
              </li>
            {/each}
          </ul>
          {#if moreError}
            <p class="text-small warn"><Icon name="warning" size={18} />Couldn't load more. {moreError}</p>
          {/if}
          {#if hasMore || moreError}
            <div class="more"><Button icon="download" busy={more} disabled={more} onclick={loadMore}>Load more</Button></div>
          {:else if provider.paged && hits.length > PAGE}
            <p class="text-small muted end">That's everything.</p>
          {/if}
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .browser {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-6);
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .ink {
    color: var(--ink);
  }
  .grow {
    flex: 1;
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
  }
  .search {
    flex: 1 1 240px;
    max-width: 380px;
  }
  .summary {
    min-height: 1.4em;
  }
  .results {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-4);
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-3) var(--space-4);
    color: var(--ink);
    text-align: start;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: pointer;
    transition:
      transform var(--duration-fast) linear,
      box-shadow var(--duration-fast) linear;
  }
  button.row:hover {
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block);
  }
  button.row:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring), var(--shadow-block-sm);
  }
  .row :global(.cb-instance-icon) {
    flex: none;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .line1 {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--space-3);
  }
  .summ {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .go {
    align-self: center;
    display: inline-flex;
    color: var(--ink-muted);
  }
  .more {
    display: flex;
    justify-content: center;
    padding-top: var(--space-2);
  }
  .end {
    text-align: center;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--gold-text);
  }
  .setup,
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    max-width: 560px;
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .setup__ic {
    display: inline-flex;
    color: var(--ink-muted);
  }
  .skeleton {
    align-items: center;
    cursor: default;
  }
  .sk-icon {
    flex: none;
    width: 48px;
    height: 48px;
    background: var(--bg);
  }
  .sk-lines {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .sk {
    display: block;
    height: 12px;
    background: var(--bg);
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
</style>
