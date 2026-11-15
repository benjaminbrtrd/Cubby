<!-- Wizard › Modpack: pick a pack from a provider, then one of its versions. The pack sets Minecraft and the loader. -->
<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../../i18n.svelte'
  import { api } from '../../../api'
  import { Banner, Button, InstanceIcon, MetaChip, TextField } from '../../../cubby'
  import { compactNum, loaderName, relativeTime } from '../../../format'
  import { projectArt } from '../../../icon'
  import { app } from '../../../state.svelte'
  import type { PackSummary, PackVersion } from '../../../types'
  import Picker from '../../discover/Picker.svelte'
  import ProviderMark from '../ProviderMark.svelte'
  import { wizard as w } from './state.svelte'

  const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))
  const providers = $derived(app.providers ?? [])
  const provider = $derived(providers.find((p) => p.id === w.provider) ?? providers[0])

  // Back from a pack's page finds the search as it was left
  let text = $state(untrack(() => app.recall<string>('wz-pack-text') ?? ''))
  $effect(() => app.keep('wz-pack-text', text))
  let query = $state(untrack(() => text.trim()))
  $effect(() => {
    const q = text
    const timer = setTimeout(() => (query = q.trim()), 250)
    return () => clearTimeout(timer)
  })

  let hits = $state<PackSummary[] | null>(null)
  let error = $state<string | null>(null)
  let tries = $state(0)
  $effect(() => {
    const p = provider
    const q = query
    void tries
    hits = null
    error = null
    if (!p) return
    if (!p.available) {
      error = p.reason ?? t('{name} isn\'t available right now.', { name: p.name })
      return
    }
    const key = `${p.id}|${q}`
    const cached = app.recall<{ key: string; hits: PackSummary[] }>('wz-pack-hits')
    if (cached?.key === key) {
      hits = cached.hits
      return
    }
    let stale = false
    const sort = q ? 'relevance' : p.sorts.includes('downloads') ? 'downloads' : (p.sorts[0] ?? 'relevance')
    api
      .packSearch(p.id, q, sort, 0)
      .then((page) => {
        if (stale) return
        hits = page.hits.slice(0, 24)
        app.keep('wz-pack-hits', { key, hits })
      })
      .catch((e) => {
        if (!stale) error = message(e)
      })
    return () => {
      stale = true
    }
  })

  function pick(p: PackSummary) {
    if (provider) w.choosePack(provider.id, p)
  }

  function setProvider(id: typeof w.provider) {
    if (id === w.provider) return
    w.provider = id
    w.pack = null
    w.packVersions = null
    w.packVersionId = null
  }

  const items = $derived((w.packVersions ?? []).map((v) => ({ id: v.id, v })))
  function meta(v: PackVersion): string {
    return [v.minecraft, v.loader ? loaderName(v.loader) : null].filter(Boolean).join(' · ') || t('Version unknown')
  }
</script>

<div class="intro">
  <h2 class="text-heading">{t('Pick a modpack')}</h2>
  <p class="text-small muted">{t('It sets the Minecraft version, the loader and the mods. You can add more mods after.')}</p>
</div>

<div class="sources" role="tablist" aria-label={t('Source')}>
  {#each providers as p (p.id)}
    <button type="button" role="tab" aria-selected={p.id === provider?.id} class={['src', p.id === provider?.id && 'is-on']} onclick={() => setProvider(p.id)}>
      <ProviderMark id={p.id} size={22} />
      <span class="text-body-strong">{p.name}</span>
    </button>
  {/each}
</div>

<TextField label={t('Search modpacks')} hideLabel icon="search" placeholder={t('Search modpacks')} bind:value={text} />

{#if w.pack}
  <section class="picked" aria-label={t('Picked modpack')}>
    <InstanceIcon {...w.pack.iconUrl ? { src: w.pack.iconUrl } : projectArt(w.pack.id)} size={44} raised />
    <div class="picked__txt">
      <span class="text-body-strong">{w.pack.name}</span>
      <span class="text-small muted">{t('by {author}', { author: w.pack.authors.join(', ') || '—' })}</span>
    </div>
    <div class="picked__ver">
      <span class="text-label muted">{t('Version')}</span>
      {#if w.packVersionsError}
        <span class="text-small bad">{w.packVersionsError}</span>
      {:else if !w.packVersions}
        <span class="text-small muted">{t('Loading versions…')}</span>
      {:else if !w.packVersions.length}
        <span class="text-small bad">{t('This pack has no version to install.')}</span>
      {:else}
        <Picker {items} value={w.packVersionId} onSelect={(id) => (w.packVersionId = id)} label={t('Version')} listWidth={320}>
          {#snippet trigger()}
            {@const v = w.packVersion}
            <span class="stack">
              <span class="text-body-strong">{v?.name ?? '—'}</span>
              {#if v}<span class="text-small muted"><span class="text-mono">{meta(v)}</span></span>{/if}
            </span>
          {/snippet}
          {#snippet option(item)}
            <span class="stack">
              <span class="text-body-strong">{item.v.name}</span>
              <span class="text-small muted"><span class="text-mono">{meta(item.v)}</span>{#if item.v.released} · {relativeTime(item.v.released, '')}{/if}</span>
            </span>
          {/snippet}
        </Picker>
      {/if}
    </div>
  </section>
{/if}

{#if error}
  {#snippet retry()}<Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>{/snippet}
  <Banner tone="warning" title={t('Search didn\'t work')} actions={provider?.available ? retry : undefined}>{error}</Banner>
{:else if hits === null}
  <p class="text-small muted">{t('Searching {name}…', { name: provider?.name ?? '' })}</p>
{:else if !hits.length}
  <p class="text-small muted">{t('No modpack matches “{query}”.', { query })}</p>
{:else}
  <ul class="grid">
    {#each hits as p (p.id)}
      {@const on = w.pack?.id === p.id}
      <li class={['card', on && 'is-on']}>
        <!-- its page: the mods inside, the versions, and Pick, which comes back here -->
        <button type="button" class="card__head" onclick={() => provider && app.openPack(provider.id, p)} aria-label={t('See {name}', { name: p.name })}>
          <InstanceIcon {...p.iconUrl ? { src: p.iconUrl } : projectArt(p.id)} size={40} />
          <span class="card__txt">
            <span class="text-body-strong name">{p.name}</span>
            {#if p.authors.length}<span class="text-small muted">{t('by {author}', { author: p.authors[0] })}</span>{/if}
          </span>
        </button>
        <p class="text-small muted summary">{p.summary}</p>
        <div class="card__foot">
          {#if p.minecraft[0]}<MetaChip>{p.minecraft[0]}</MetaChip>{/if}
          {#if p.loaders[0]}<MetaChip>{loaderName(p.loaders[0])}</MetaChip>{/if}
          {#if p.downloads}<span class="text-small muted">{t('{n} downloads', { n: compactNum(p.downloads) })}</span>{/if}
          <span class="grow"></span>
          <Button size="sm" variant={on ? 'primary' : 'secondary'} icon={on ? 'check' : undefined} onclick={() => pick(p)}>{on ? t('Picked') : t('Pick')}</Button>
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  h2,
  p,
  ul {
    margin: 0;
  }
  ul {
    padding: 0;
    list-style: none;
  }
  .muted {
    color: var(--ink-muted);
  }
  .bad {
    color: var(--redstone-text);
  }
  .grow {
    flex: 1;
  }
  .intro {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .sources {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .src {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--size-control);
    padding: 4px 12px;
    font: inherit;
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .src:hover {
    background: var(--surface-hover);
  }
  .src.is-on {
    outline: var(--border-block) solid var(--ink);
    outline-offset: 2px;
  }
  .src:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .picked {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .picked__txt,
  .card__txt,
  .stack {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .picked__txt {
    flex: 1 1 200px;
  }
  .picked__ver {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 240px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--space-3);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .card.is-on {
    outline: var(--border-block) solid var(--ink);
    outline-offset: 2px;
  }
  .card__head {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    padding: 0;
    font: inherit;
    color: inherit;
    text-align: start;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .card__head:hover .name {
    text-decoration: underline;
  }
  .card__head:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .name {
    overflow-wrap: anywhere;
  }
  .summary {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card__foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: auto;
  }
</style>
