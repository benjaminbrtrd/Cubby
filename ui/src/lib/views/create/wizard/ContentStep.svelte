<!-- Wizard › Mods, Looks or Datapacks: search Modrinth or CurseForge for what fits the version and loader picked,
     and add or remove projects. They install once the instance is created. -->
<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../../i18n.svelte'
  import { api } from '../../../api'
  import { Banner, Button, Icon, InstanceIcon, SourceMark, Tabs, TextField } from '../../../cubby'
  import { compactNum, loaderName } from '../../../format'
  import { projectArt } from '../../../icon'
  import { app } from '../../../state.svelte'
  import type { ProjectHit } from '../../../types'
  import { wizard as w, type Pick, type PickKind } from './state.svelte'
  import { typeOf } from './kinds'

  let { tabs }: { tabs: PickKind[] } = $props()

  const LABEL: Record<PickKind, string> = { mod: t('Mods'), resourcepack: t('Resource packs'), shader: t('Shader pack'), datapack: t('Datapacks') }
  const PAGE = 20
  const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))

  // Back from a project's page finds this step as it was left: tab, source, search, type and results
  const mem = untrack(() => `wz-content:${tabs.join()}:`)
  const recalled = <T,>(key: string, or: T): T => untrack(() => app.recall<T>(mem + key) ?? or)

  let tab = $state<PickKind>(recalled('tab', untrack(() => tabs[0])))
  $effect(() => {
    if (!tabs.includes(tab)) tab = tabs[0]
  })
  const cfReady = $derived(!!app.providers?.find((p) => p.id === 'curseforge')?.available)
  let source = $state<'modrinth' | 'curseforge'>(recalled('source', 'modrinth'))
  const sources = $derived([
    { value: 'modrinth' as const, label: 'Modrinth' },
    ...(cfReady ? [{ value: 'curseforge' as const, label: 'CurseForge' }] : []),
  ])

  const loader = $derived(w.gameLoader)
  const game = $derived(w.gameVersion)
  /** Mods need a loader; shaders need Iris, a mod. */
  const blocked = $derived(w.vanilla && (tab === 'mod' || tab === 'shader'))

  let text = $state(recalled('text', ''))
  let query = $state(untrack(() => text.trim()))
  $effect(() => {
    app.keep(mem + 'tab', tab)
    app.keep(mem + 'source', source)
    app.keep(mem + 'text', text)
    app.keep(mem + 'type', modType)
  })
  $effect(() => {
    const q = text
    const timer = setTimeout(() => (query = q.trim()), 250)
    return () => clearTimeout(timer)
  })

  let hits = $state<ProjectHit[] | null>(null)
  let total = $state(0)
  /** Results read from the source so far: more than `hits` when Content leaves some out. */
  let read = $state(0)
  let error = $state<string | null>(null)
  let loadingMore = $state(false)
  let tries = $state(0)

  function search(offset: number) {
    const q = {
      query,
      kind: tab,
      gameVersion: game,
      loader: tab === 'mod' ? loader : null,
      categories: typeCategories,
      environment: null,
      sort: query ? ('relevance' as const) : ('downloads' as const),
      offset,
      limit: PAGE,
    }
    return source === 'curseforge' ? api.curseforgeSearch(q) : api.searchProjects(q)
  }

  $effect(() => {
    void tab
    void source
    void query
    void typeCategories
    void modType
    void game
    void loader
    void tries
    hits = null
    error = null
    if (blocked) return
    const key = untrack(searchKey)
    const cached = untrack(() => app.recall<Results>(mem + 'results'))
    if (cached?.key === key) {
      hits = cached.hits
      read = cached.read
      total = cached.total
      return
    }
    let stale = false
    read = 0
    fill([], 0, () => stale)
      .then((r) => {
        if (stale) return
        hits = r.hits
        read = r.read
        total = r.total
        app.keep(mem + 'results', { key, ...r })
      })
      .catch((e) => {
        if (!stale) error = message(e)
      })
    return () => {
      stale = true
    }
  })

  interface Results {
    key: string
    hits: ProjectHit[]
    read: number
    total: number
  }
  /** What the results depend on: the same key, the same list. */
  const searchKey = () => JSON.stringify([tab, source, query, typeCategories, modType, game, loader])

  /** Reads pages from `from` until a page's worth of results pass the type filter, or the source has no more.
   *  Content is checked on the results, so one page of the source can keep only a few. */
  async function fill(have: ProjectHit[], from: number, stale: () => boolean) {
    const seen = new Set(have.map((h) => h.id))
    const kept: ProjectHit[] = []
    let at = from
    let all = Infinity
    for (let reads = 0; reads < 6 && kept.length < PAGE && at < all && !stale(); reads++) {
      const r = await search(at)
      all = r.total
      at += r.hits.length
      for (const h of r.hits) if (!seen.has(h.id) && keep(h)) seen.add(h.id) && kept.push(h)
      // an empty page is the end, whatever the total said
      if (!r.hits.length) {
        all = at
        break
      }
    }
    return { hits: [...have, ...kept], read: at, total: Number.isFinite(all) ? all : at }
  }

  async function more() {
    if (!hits) return
    loadingMore = true
    try {
      const r = await fill(hits, read, () => false)
      hits = r.hits
      read = r.read
      total = r.total
      app.keep(mem + 'results', { key: searchKey(), ...r })
    } catch (e) {
      app.error(e)
    } finally {
      loadingMore = false
    }
  }

  function toPick(h: ProjectHit): Pick {
    // Modrinth projects by slug: the quick picks use slugs too, so a mod is never added twice.
    return { source, projectId: source === 'modrinth' ? h.slug : h.id, kind: tab, title: h.title, author: h.author, iconUrl: h.iconUrl }
  }

  // types of mod: Modrinth's categories, CurseForge's matching categories found by name
  type ModType = 'all' | 'content' | 'library' | 'performance'
  let modType = $state<ModType>(recalled('type', 'all'))
  let cfCats = $state<{ id: string; name: string }[] | null>(null)
  $effect(() => {
    if (tab !== 'mod' || source !== 'curseforge' || cfCats) return
    api
      .curseforgeCategories('mod')
      .then((list) => (cfCats = list))
      .catch(() => (cfCats = []))
  })
  const cfLibrary = $derived(cfCats?.find((c) => /api|librar/i.test(c.name))?.id ?? null)
  const cfPerformance = $derived(cfCats?.find((c) => /performance/i.test(c.name))?.id ?? null)
  const types = $derived(
    (
      [
        ['all', t('All'), null],
        ['content', t('Content'), 'cube'],
        ['library', t('Library'), 'wrench'],
        ['performance', t('Performance'), 'memory'],
      ] as const
    ).filter(([id]) => source === 'modrinth' || id === 'all' || (id === 'library' && cfLibrary) || (id === 'performance' && cfPerformance)),
  )
  $effect(() => {
    // CurseForge's types come with its categories: wait for them before dropping a type kept from before
    if (tab === 'mod' && source === 'curseforge' && !cfCats) return
    if (!types.some(([id]) => id === modType)) modType = 'all'
  })
  /** The categories the search asks for, for the type picked. */
  const typeCategories = $derived.by((): string[] => {
    if (tab !== 'mod' || modType === 'all' || modType === 'content') return []
    if (source === 'modrinth') return [modType === 'library' ? 'library' : 'optimization']
    const id = modType === 'library' ? cfLibrary : cfPerformance
    return id ? [id] : []
  })
  /** Content: neither a library nor a performance mod (Modrinth tells; checked on the results). */
  const keep = (h: ProjectHit) => !(tab === 'mod' && modType === 'content') || !h.categories.some((c) => c === 'library' || c === 'optimization')

  const gameLine = $derived([game, tab === 'mod' && loader ? loaderName(loader) : null].filter(Boolean).join(' · '))
  const tabItems = $derived(tabs.map((k) => ({ id: k, label: LABEL[k], count: String(w.picked(k).length), panel: 'wz-content' })))
</script>

<div class="intro">
  {#if tabs[0] === 'mod'}
    <h2 class="text-heading">{w.path === 'modpack' ? t('Add mods on top of the pack') : t('Add mods')}</h2>
    <p class="text-small muted">{t('Only what’s made for {game} is shown. The mods they need are added for you.', { game: gameLine || '—' })}</p>
  {:else if tabs[0] === 'datapack'}
    <h2 class="text-heading">{t('Datapacks for new worlds')}</h2>
    <p class="text-small muted">{t('Datapacks add biomes, structures or rules to a world, without mods. Cubby adds these to each new world of this instance when you close the game, so biome and structure datapacks shape the parts you explore after that.')}</p>
    {#if w.gameLoader && w.gameLoader !== 'vanilla'}
      <p class="text-small muted">{t('Datapacks that also come as a mod for {loader} are installed as that mod: they shape every world from the start.', { loader: loaderName(w.gameLoader) })}</p>
    {/if}
  {:else}
    <h2 class="text-heading">{t('Make it look your way')}</h2>
    <p class="text-small muted">{t('Resource packs change textures and sounds. A shader pack changes light, shadows and water. Neither changes the gameplay.')}</p>
  {/if}
</div>

{#if tabs.length > 1}
  <Tabs label={t('Kind of pack')} items={tabItems} value={tab} onChange={(id) => (tab = id as PickKind)} />
{/if}

{#if blocked}
  {#snippet toGame()}<Button size="sm" iconEnd="chevron-right" onclick={() => (w.step = 'game')}>{t('Pick a loader')}</Button>{/snippet}
  <Banner tone="warning" title={tab === 'mod' ? t('Vanilla runs no mods') : t('Vanilla can’t load shader packs')} actions={toGame}>
    {tab === 'mod' ? t('Pick Fabric, Quilt or NeoForge in Game to add mods, or skip this step.') : t('Shader packs run through Iris, a mod: pick Fabric or NeoForge in Game.')}
  </Banner>
{:else}
  {#if tab === 'shader' && w.shaderNeedsIris}
    {#snippet addIris()}<Button size="sm" icon="plus" onclick={() => w.addIris()}>{t('Add Iris and Sodium')}</Button>{/snippet}
    <Banner tone="warning" title={t('Shader packs need Iris')} actions={addIris}>{t('Iris is the mod that loads shader packs, and Sodium makes it fast. Add both with the shader pack.')}</Banner>
  {/if}

  {#if tab === 'mod' && types.length > 1}
    <div class="quick" role="radiogroup" aria-label={t('Type of mod')}>
      <span class="text-label muted">{t('Type')}</span>
      {#each types as [id, label, icon] (id)}
        {@const on = modType === id}
        <button type="button" role="radio" aria-checked={on} class={['pill', on && 'is-on']} onclick={() => (modType = id)}>
          {#if icon}<Icon name={icon} size={18} />{/if}{label}
        </button>
      {/each}
    </div>
  {/if}

  <div class="bar">
    <div class="search">
      <TextField label={t('Search {what}', { what: LABEL[tab].toLowerCase() })} hideLabel icon="search" placeholder={t('Search {what}', { what: LABEL[tab].toLowerCase() })} bind:value={text} />
    </div>
    {#if sources.length > 1}
      <div class="sources" role="radiogroup" aria-label={t('Source')}>
        {#each sources as o (o.value)}
          <button type="button" role="radio" aria-checked={source === o.value} class={['src', source === o.value && 'is-on']} onclick={() => (source = o.value)}>
            <SourceMark tone={o.value} label={o.label} size={20} /><span class="text-body-strong">{o.label}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <div id="wz-content">
    {#if error}
      {#snippet retry()}<Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>{/snippet}
      <Banner tone="warning" title={t('Search didn\'t work')} actions={retry}>{error}</Banner>
    {:else if hits === null}
      <p class="text-small muted">{t('Searching…')}</p>
    {:else if !hits.length}
      <p class="text-small muted">{query ? t('Nothing matches “{query}”. Check the spelling or try fewer words.', { query }) : t('Nothing here works with {game} yet.', { game: gameLine })}</p>
    {:else}
      <ul class="list">
        {#each hits as h (h.id)}
          {@const p = toPick(h)}
          {@const on = w.has(p)}
          <li class={['row', on && 'is-on']}>
            <InstanceIcon {...h.iconUrl ? { src: h.iconUrl } : projectArt(h.slug)} size={40} />
            <div class="row__txt">
              <span class="row__head">
                <button type="button" class="name text-body-strong" onclick={() => app.openProject(p.projectId, h.kind, source === 'curseforge' ? 'curseforge' : undefined, h.title)} aria-label={t('See {name}', { name: h.title })}>{h.title}</button>
                {#if h.author}<span class="text-small muted">{t('by {author}', { author: h.author })}</span>{/if}
              </span>
              <span class="text-small muted summary">{h.summary}</span>
            </div>
            <span class="row__end">
              {#if h.downloads}<span class="text-small muted">{compactNum(h.downloads)}</span>{/if}
              {#if typeOf(h.categories, h.kind)}
                {@const ty = typeOf(h.categories, h.kind)!}
                <span class="ty" title={ty.label}><Icon name={ty.icon} size={18} /><span class="cb-visually-hidden">{ty.label}</span></span>
              {/if}
              <Button size="sm" variant={on ? 'primary' : 'secondary'} icon={on ? 'check' : 'plus'} aria-pressed={on} onclick={() => w.toggle(p)}>
                {on ? t('Added') : tab === 'shader' ? t('Use this one') : t('Add')}
              </Button>
            </span>
          </li>
        {/each}
      </ul>
      {#if read < total}
        <Button variant="ghost" icon="chevron-down" busy={loadingMore} disabled={loadingMore} onclick={more}>{t('Show more')}</Button>
      {/if}
    {/if}
  </div>
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
  .intro {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    max-width: 820px;
  }
  .quick {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 32px;
    padding: 4px 12px;
    font: inherit;
    font-size: 14px;
    font-weight: 500;
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .pill:hover {
    background: var(--surface-hover);
  }
  .pill.is-on {
    color: var(--on-grass);
    background: var(--grass);
  }
  .pill:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .search {
    flex: 1 1 320px;
  }
  #wz-content {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: stretch;
  }
  .list {
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    padding: 10px 14px;
    background: var(--surface);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row.is-on {
    background: var(--surface-hover);
  }
  .row__txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .row__head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--space-2);
  }
  .summary {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .row__end {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .ty {
    display: inline-flex;
    color: var(--ink-muted);
  }
  .name {
    padding: 0;
    font: inherit;
    font-weight: 500;
    color: var(--ink);
    text-align: start;
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 3px;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .name:hover {
    text-decoration-color: currentColor;
  }
  .name:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .sources {
    display: flex;
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
</style>
