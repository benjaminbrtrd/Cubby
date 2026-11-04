<!-- Discover: mods, modpacks, shader packs and resource packs from Modrinth, checked against the
     instance they would go into. Filters live in app.discover so they survive a visit to a project page. -->
<script lang="ts" module>
  import type { ProjectHit } from '../../types'

  /** The "Popular modpacks" and "Make it look better" picks, loaded once per session. */
  let picks: Promise<{ pack: ProjectHit | null; shader: ProjectHit | null; resource: ProjectHit | null }> | null = null
</script>

<script lang="ts">
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, InstanceIcon, MetaChip, Switch, Tabs, TextField } from '../../cubby'
  import { compactNum, instanceMeta, num, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { ProjectKind, ProjectQuery, ProjectSort, SearchResults } from '../../types'
  import ProviderBrowser from '../create/ProviderBrowser.svelte'
  import ProviderMark from '../create/ProviderMark.svelte'
  import InstallPanel from './InstallPanel.svelte'
  import Picker from './Picker.svelte'
  import TargetPicker from './TargetPicker.svelte'
  import { CATEGORIES, KINDS, KIND_ORDER, PAGE_SIZE, categoryLabel, hitFit, type Fit } from './discover'

  const d = $derived(app.discover)
  /** Where this tab can look: every provider for modpacks, Modrinth and CurseForge for the rest. Modrinth first. */
  const sources = $derived(
    [...(app.providers ?? [])]
      .filter((p) => d.kind === 'modpack' || p.id === 'modrinth' || p.id === 'curseforge')
      .sort((a, b) => (a.id === 'modrinth' ? -1 : b.id === 'modrinth' ? 1 : a.name.localeCompare(b.name))),
  )
  /** The chosen source, or Modrinth when this tab doesn't offer it. */
  const source = $derived(sources.some((p) => p.id === d.provider) ? d.provider : 'modrinth')
  /** Another provider's modpacks use its own browser instead of the Modrinth search below. */
  const embedded = $derived(d.kind === 'modpack' && source !== 'modrinth' ? (sources.find((p) => p.id === source) ?? null) : null)
  /** CurseForge mods, shader packs and resource packs are not in Cubby yet. */
  const notYet = $derived(d.kind !== 'modpack' && source === 'curseforge')
  $effect(() => {
    if (!app.providers && !app.providersError) void app.loadProviders()
  })
  const kind = $derived(d.kind)
  const words = $derived(KINDS[kind])
  const target = $derived(app.target)
  /** Results are limited to what fits the target instance. */
  const scoped = $derived(kind !== 'modpack' && d.compatibleOnly && !!target)

  const tabs = KIND_ORDER.map((k) => ({ id: k, label: KINDS[k].label, panel: 'discover-results' }))
  const ENVIRONMENTS: ['client' | 'server', string][] = [
    ['client', 'Client'],
    ['server', 'Server'],
  ]
  const sorts: { id: ProjectSort; label: string }[] = [
    { id: 'relevance', label: 'Relevance' },
    { id: 'downloads', label: 'Most downloaded' },
    { id: 'follows', label: 'Most followed' },
    { id: 'updated', label: 'Recently updated' },
    { id: 'newest', label: 'Newest' },
  ]
  const sortLabel = $derived(sorts.find((s) => s.id === app.discover.sort)?.label ?? 'Relevance')

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  // search
  let text = $state(app.discover.query)
  $effect(() => {
    const q = text
    if (q === app.discover.query) return
    const t = setTimeout(() => setQuery(q), 250)
    return () => clearTimeout(t)
  })

  function setQuery(q: string) {
    if (app.discover.query === q) return
    app.discover.query = q
    app.discover.page = 0
  }

  let results = $state<SearchResults | null>(null)
  let loading = $state(true)
  let error = $state<string | null>(null)
  let retries = $state(0)
  let seq = 0

  // plain values: the instance list is replaced after every task, and that must not search again
  const targetId = $derived(target?.id)
  const targetMc = $derived(target?.minecraft ?? null)
  const targetLoader = $derived(target?.loader.kind ?? null)

  $effect(() => {
    const q: ProjectQuery = {
      query: d.query,
      kind,
      gameVersion: scoped ? targetMc : null,
      loader: scoped && kind === 'mod' ? targetLoader : null,
      categories: [...d.categories],
      environment: kind === 'mod' ? d.environment : null,
      sort: d.sort,
      offset: d.page * PAGE_SIZE,
      limit: PAGE_SIZE,
    }
    void retries
    const mine = ++seq
    loading = true
    error = null
    api
      .searchProjects(q)
      .then((r) => {
        if (mine !== seq) return
        results = r
        loading = false
      })
      .catch((e) => {
        if (mine !== seq) return
        error = message(e)
        loading = false
      })
  })

  $effect(() => {
    if (targetId) void app.loadSources(targetId)
  })

  const offline = $derived(!!error && (/(network|connect|dns|resolve|timed? ?out|offline|unreachable|socket|tls|fetch)/i.test(error) || !navigator.onLine))
  const hits = $derived(results?.hits ?? [])
  const shown = $derived(d.hideInstalled && target && kind !== 'modpack' ? hits.filter((h) => !app.installedIn(target.id, h.id)) : hits)
  const hiddenCount = $derived(hits.length - shown.length)
  const pages = $derived(results ? Math.max(1, Math.ceil(results.total / PAGE_SIZE)) : 1)
  const filtered = $derived(d.categories.length > 0 || d.environment !== null || d.hideInstalled)

  // filters
  function setKind(k: string) {
    const next = k as ProjectKind
    if (next === app.discover.kind) return
    app.discover.kind = next
    app.discover.categories = []
    app.discover.environment = null
    app.discover.page = 0
    installing = null
  }

  function toggleCategory(c: string) {
    const list = app.discover.categories
    app.discover.categories = list.includes(c) ? list.filter((x) => x !== c) : [...list, c]
    app.discover.page = 0
  }

  function setEnvironment(env: 'client' | 'server') {
    app.discover.environment = app.discover.environment === env ? null : env
    app.discover.page = 0
  }

  function setSort(v: string) {
    app.discover.sort = v as ProjectSort
    app.discover.page = 0
  }

  function setScoped(on: boolean) {
    app.discover.compatibleOnly = on
    app.discover.page = 0
  }

  function clearFilters() {
    app.discover.categories = []
    app.discover.environment = null
    app.discover.hideInstalled = false
    app.discover.page = 0
  }

  function clearSearch() {
    text = ''
    setQuery('')
  }

  function goPage(p: number) {
    app.discover.page = Math.max(0, Math.min(pages - 1, p))
    document.querySelector('main')?.scrollTo?.(0, 0)
  }

  /** 1-based page numbers to show, null = a gap. */
  const pageList = $derived.by((): (number | null)[] => {
    const cur = d.page + 1
    const set = new Set([1, pages, cur - 1, cur, cur + 1].filter((p) => p >= 1 && p <= pages))
    const sorted = [...set].sort((a, b) => a - b)
    const out: (number | null)[] = []
    sorted.forEach((p, i) => {
      if (i && p - sorted[i - 1] > 1) out.push(null)
      out.push(p)
    })
    return out
  })

  // cards
  /** projectId → instance id, for installs started from a card. */
  let pending = $state<Record<string, string>>({})
  let installing = $state<ProjectHit | null>(null)

  interface CardState {
    fit: Fit
    action: 'install' | 'other' | 'installed' | 'update' | 'busy'
    version: string | null
  }

  function cardState(h: ProjectHit): CardState {
    if (h.kind === 'modpack' || !target) return { fit: hitFit(h, target), action: 'install', version: null }
    const busy = pending[h.id] === target.id && !!app.taskFor(`install:${target.id}`)
    if (busy) return { fit: { tone: 'busy', icon: 'download', word: 'Installing' }, action: 'busy', version: null }
    const have = app.installedIn(target.id, h.id)
    if (have) {
      const update = app.updates[target.id]?.some((u) => u.projectId === h.id)
      if (update) return { fit: { tone: 'busy', icon: 'update', word: 'Update available' }, action: 'update', version: have.versionNumber }
      return { fit: { tone: 'have', icon: 'check', word: `In ${target.name}` }, action: 'installed', version: have.versionNumber }
    }
    const fit = hitFit(h, target)
    return { fit, action: fit.tone === 'ok' ? 'install' : 'other', version: null }
  }

  function art(h: ProjectHit) {
    return h.iconUrl ? { src: h.iconUrl } : projectArt(h.slug)
  }

  // popular picks
  let pickList = $state<{ pack: ProjectHit | null; shader: ProjectHit | null; resource: ProjectHit | null } | null>(null)
  const showPicks = $derived(kind === 'mod' && d.page === 0 && !d.query && !loading && !error)

  $effect(() => {
    if (!showPicks || pickList) return
    const top = (k: ProjectKind) =>
      api
        .searchProjects({ query: '', kind: k, gameVersion: null, loader: null, categories: [], environment: null, sort: 'downloads', offset: 0, limit: 1 })
        .then((r) => r.hits[0] ?? null)
    picks ??= Promise.all([top('modpack'), top('shader'), top('resourcepack')]).then(([pack, shader, resource]) => ({ pack, shader, resource }))
    picks
      .then((p) => (pickList = p))
      .catch(() => {
        picks = null
      })
  })

  const sentence = (s: string) => s.charAt(0).toUpperCase() + s.slice(1)
</script>

{#snippet fitLine(f: Fit)}
  <span class={['fit', 'text-body', f.tone]}><Icon name={f.icon} size={18} />{f.word}</span>
{/snippet}

{#snippet pickCard(h: ProjectHit)}
  <button type="button" class="pick" onclick={() => app.openProject(h.slug, h.kind)}>
    <InstanceIcon {...art(h)} size={32} />
    <span class="pick-text">
      <span class="text-body-strong">{h.title}</span>
      <span class="text-small muted">{sentence(KINDS[h.kind].one)} · {compactNum(h.downloads)} downloads</span>
    </span>
    <Icon name="chevron-right" size={18} />
  </button>
{/snippet}

{#snippet backToModrinth()}<Button size="sm" onclick={() => (app.discover.provider = 'modrinth')}>Search on Modrinth</Button>{/snippet}

<div class="screen">
  <div class="top">
    <h1 class="text-title">Discover</h1>
    {#if !embedded && !notYet}
    <div class="search">
      <TextField
        label="Search {words.many}"
        hideLabel
        icon="search"
        placeholder="Search {words.many}"
        bind:value={text}
        onkeydown={(e) => {
          if (e.key === 'Enter') setQuery(text)
        }}
      />
    </div>
    {/if}
    <span class="grow"></span>
    {#if kind === 'modpack'}
      <span class="text-small muted line"><Icon name="library" size={18} />Each modpack installs as a new instance</span>
    {:else if target}
      <div class="target">
        <span class="text-small muted">Install to</span>
        <TargetPicker variant="block" />
      </div>
    {:else}
      <Button icon="plus" onclick={() => app.navigate({ name: 'create' })}>New instance</Button>
    {/if}
  </div>

  <div class="bar">
    <div class="tabs">
      <Tabs label="What to discover" items={tabs} value={kind} onChange={setKind} />
    </div>
    {#if !embedded && !notYet}
    <div class="sort">
      <Picker items={sorts} value={d.sort} onSelect={setSort} label="Sort by" buttonLabel="Sort by {sortLabel}. Change" variant="ghost" listWidth={200}>
        {#snippet trigger()}<span class="text-body">Sort: <span class="text-body-strong ink">{sortLabel}</span></span>{/snippet}
        {#snippet option(item, on)}<span class={['text-body', on && 'text-body-strong']}>{item.label}</span>{/snippet}
      </Picker>
    </div>
    {/if}
  </div>

  {#if sources.length > 1}
    <div class="sources" role="group" aria-label="Source">
      <span class="text-label muted">Source</span>
      {#each sources as p (p.id)}
        <button type="button" class={['source', source === p.id && 'is-on']} aria-pressed={source === p.id} onclick={() => (app.discover.provider = p.id)}>
          <ProviderMark id={p.id} size={22} />
          <span class="text-body-strong">{p.name}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if notYet}
    <div class="soon">
      <Banner tone="info" icon="info" title="CurseForge {words.many} aren’t in Cubby yet" actions={backToModrinth}>
        Cubby can install CurseForge modpacks for now. Modrinth has {words.many} for the same games.
      </Banner>
    </div>
  {:else if embedded}
    <div class="embed">
      {#key embedded.id}<ProviderBrowser provider={embedded} />{/key}
    </div>
  {:else}
  <div class="layout">
    <nav class="filters" aria-label="Filters">
      {#each CATEGORIES[kind] as group (group.label)}
        <section>
          <h2 class="text-label muted">{group.label}</h2>
          <ul>
            {#each group.items as c (c)}
              {@const on = d.categories.includes(c)}
              <li>
                <button type="button" class="check" aria-pressed={on} onclick={() => toggleCategory(c)}>
                  <span class={['box', on && 'on']} aria-hidden="true">{#if on}<Icon name="check" size={9} />{/if}</span>
                  <span class="text-body">{categoryLabel(c)}</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/each}

      {#if kind === 'mod'}
        <section>
          <h2 class="text-label muted">Environment</h2>
          <ul>
            {#each ENVIRONMENTS as [env, label] (env)}
              {@const on = d.environment === env}
              <li>
                <button type="button" class="check" aria-pressed={on} onclick={() => setEnvironment(env)}>
                  <span class={['box', on && 'on']} aria-hidden="true">{#if on}<Icon name="check" size={9} />{/if}</span>
                  <span class="text-body">{label}</span>
                </button>
              </li>
            {/each}
          </ul>
          <p class="text-small muted hint">Client: runs in your game. Server: can run on a server you host.</p>
        </section>
      {/if}

      {#if kind !== 'modpack' && target}
        <Switch
          checked={d.hideInstalled}
          onChange={(v) => (app.discover.hideInstalled = v)}
          label="Hide installed"
          hint="Skip {words.many} already in {target.name}"
        />
      {/if}

      {#if filtered}
        <Button variant="ghost" size="sm" icon="close" onclick={clearFilters}>Clear filters</Button>
      {/if}
    </nav>

    <div class="results">
      {#if target && target.loader.kind === 'vanilla' && (kind === 'mod' || kind === 'shader')}
        {#snippet vanillaActions()}
          <Button size="sm" icon="plus" onclick={() => app.navigate({ name: 'create' })}>New instance</Button>
        {/snippet}
        <Banner tone="warning" title="{target.name} is Vanilla" actions={vanillaActions}>
          {kind === 'mod'
            ? 'Mods need a loader like Fabric, Quilt or NeoForge. Pick another instance in Install to, or create one with a loader.'
            : 'Shader packs run through Iris, a mod, so they need a loader like Fabric or NeoForge. Pick another instance, or create one.'}
        </Banner>
      {/if}

      <section class="about" aria-label="About these results">
        <p class="text-body-strong summary" aria-live="polite">
          <span class="info-ic"><Icon name="info" size={18} /></span>
          {#if loading && !results}
            <span>Searching Modrinth…</span>
          {:else if results}
            <span>
              {plural(results.total, 'result', 'results')}.
              {#if kind === 'modpack'}
                Each one installs as a new instance.
              {:else if scoped && target}
                Showing {words.many} that work with {target.name} (<span class="text-mono">{instanceMeta(target)}</span>).
              {:else if target}
                Showing {words.many} for every Minecraft version.
              {/if}
            </span>
            {#if kind !== 'modpack' && target}
              <button type="button" class="link" onclick={() => setScoped(!d.compatibleOnly)}>
                {d.compatibleOnly ? 'Show everything' : `Only what works with ${target.name}`}
              </button>
            {/if}
          {:else}
            <span>Modrinth search</span>
          {/if}
        </p>
        <p class="text-small muted new"><span class="ink">New to {words.many}?</span> {words.help}</p>
      </section>

      <div id="discover-results" role="tabpanel" aria-label={words.label} aria-busy={loading}>
        {#if error}
          {#snippet retryAction()}
            <Button size="sm" icon="update" onclick={() => retries++}>Retry</Button>
          {/snippet}
          <Banner tone="warning" icon={offline ? 'offline' : 'warning'} title={offline ? "Can't reach Modrinth" : "Search didn't work"} actions={retryAction}>
            {offline ? 'Check your internet connection, then try again.' : error}
          </Banner>
        {:else if loading}
          <div class="grid" aria-hidden="true">
            {#each { length: 8 }, i (i)}
              <div class="card skeleton">
                <div class="row"><span class="sk-icon"></span><span class="sk-lines"><span class="sk w60"></span><span class="sk w90"></span></span></div>
                <span class="sk w40"></span>
                <span class="sk w70"></span>
              </div>
            {/each}
          </div>
          <span class="cb-visually-hidden">Loading results</span>
        {:else if !shown.length}
          <div class="empty">
            <span class="empty-ic" aria-hidden="true"><Icon name="search" size={27} /></span>
            <h2 class="text-heading">
              {hits.length ? `Everything here is already in ${target?.name ?? 'your instance'}` : `No ${words.many} found`}
            </h2>
            <p class="text-body muted">
              {#if hits.length}
                Turn off Hide installed, or go to the next page.
              {:else if d.query}
                Nothing matches “{d.query}”{scoped && target ? ` for ${instanceMeta(target)}` : ''}. Check the spelling or try fewer words.
              {:else if scoped && target}
                Nothing here works with {target.name} yet. Show everything to see other versions.
              {:else}
                Try fewer filters.
              {/if}
            </p>
            <div class="empty-actions">
              {#if hits.length && d.page + 1 < pages}<Button iconEnd="chevron-right" onclick={() => goPage(d.page + 1)}>Next page</Button>{/if}
              {#if scoped}<Button onclick={() => setScoped(false)}>Show everything</Button>{/if}
              {#if filtered}<Button onclick={clearFilters}>Clear filters</Button>{/if}
              {#if d.query}<Button onclick={clearSearch}>Clear search</Button>{/if}
            </div>
          </div>
        {:else}
          {#if hiddenCount}
            <p class="text-small muted">{plural(hiddenCount, `${words.one} is`, `${words.many} are`)} hidden on this page: already in {target?.name}.</p>
          {/if}
          <ul class="grid">
            {#each shown as h (h.id)}
              {@const s = cardState(h)}
              <li class="card">
                <div class="row">
                  <InstanceIcon {...art(h)} size={32} />
                  <div class="head">
                    <div class="name-line">
                      <button type="button" class="title text-body-strong" onclick={() => app.openProject(h.slug, h.kind)}>{h.title}</button>
                      <span class="text-small muted author" title="by {h.author}">by {h.author}</span>
                    </div>
                    <p class="text-small muted summary-text" title={h.summary}>{h.summary}</p>
                  </div>
                </div>
                <div class="row spread">
                  <span class="chips">
                    {#each h.categories.slice(0, 2) as c (c)}<MetaChip>{categoryLabel(c)}</MetaChip>{/each}
                  </span>
                  {#if s.version}
                    <span class="text-small muted">Version <span class="text-mono">{s.version}</span></span>
                  {:else}
                    <span class="text-small muted stats">
                      <span class="stat"><Icon name="download" size={18} />{compactNum(h.downloads)}<span class="cb-visually-hidden"> downloads</span></span>
                      <span class="stat"><Icon name="clock" size={18} /><span class="cb-visually-hidden">Updated </span>{relativeTime(h.updated, '')}</span>
                    </span>
                  {/if}
                </div>
                <div class="row spread">
                  {@render fitLine(s.fit)}
                  <span class="act">
                    {#if s.action === 'installed'}
                      <Button size="sm" icon="check" disabled>Installed</Button>
                    {:else if s.action === 'busy'}
                      <Button size="sm" icon="download" disabled>Installing</Button>
                    {:else if s.action === 'update'}
                      <Button size="sm" icon="update" onclick={() => (installing = h)} aria-label="Update {h.title}">Update</Button>
                    {:else if s.action === 'other'}
                      <Button size="sm" onclick={() => (installing = h)} aria-label="Install {h.title} to another instance or version">Install to…</Button>
                    {:else}
                      <Button size="sm" icon="download" onclick={() => (installing = h)} aria-label="Install {h.title}">Install</Button>
                    {/if}
                  </span>
                </div>
              </li>
            {/each}
          </ul>
        {/if}

        {#if results && !error && pages > 1}
          <nav class="pages" aria-label="Result pages">
            <span class="text-small muted">Page {num(d.page + 1)} of {num(pages)}</span>
            <span class="grow"></span>
            <Button size="sm" disabled={d.page === 0 || loading} onclick={() => goPage(d.page - 1)}>Previous</Button>
            {#each pageList as p, i (i)}
              {#if p === null}
                <span class="text-body muted" aria-hidden="true">…</span>
              {:else}
                <button
                  type="button"
                  class={['page', 'text-body-strong', p === d.page + 1 && 'on']}
                  aria-current={p === d.page + 1 ? 'page' : undefined}
                  aria-label="Page {p}"
                  disabled={loading}
                  onclick={() => goPage(p - 1)}>{num(p)}</button
                >
              {/if}
            {/each}
            <Button size="sm" iconEnd="chevron-right" disabled={d.page + 1 >= pages || loading} onclick={() => goPage(d.page + 1)}>Next</Button>
          </nav>
        {/if}
      </div>

      {#if showPicks && pickList && (pickList.pack || pickList.shader || pickList.resource)}
        <div class="picks">
          {#if pickList.pack}
            <section aria-labelledby="dc-packs">
              <div class="picks-head">
                <h2 id="dc-packs" class="text-label muted">Popular modpacks</h2>
                <button type="button" class="link text-small" onclick={() => setKind('modpack')}>See all</button>
              </div>
              {@render pickCard(pickList.pack)}
            </section>
          {/if}
          {#if pickList.shader || pickList.resource}
            <section aria-labelledby="dc-looks">
              <div class="picks-head">
                <h2 id="dc-looks" class="text-label muted">Make it look better</h2>
                <span class="text-small muted">Changes the look, never the gameplay</span>
              </div>
              <div class="picks-two">
                {#if pickList.shader}{@render pickCard(pickList.shader)}{/if}
                {#if pickList.resource}{@render pickCard(pickList.resource)}{/if}
              </div>
            </section>
          {/if}
        </div>
      {/if}
    </div>
  </div>
  {/if}
</div>

{#if installing}
  {@const h = installing}
  {#snippet dialogActions()}
    <Button variant="ghost" iconEnd="chevron-right" onclick={() => ((installing = null), app.openProject(h.slug, h.kind))}>See the full page</Button>
    <Button onclick={() => (installing = null)}>Close</Button>
  {/snippet}
  <Dialog wide title="Install {h.title}" onClose={() => (installing = null)} actions={dialogActions}>
    <InstallPanel project={h} compact onLeave={() => (installing = null)} onInstall={(id) => (pending[h.id] = id)} />
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-height: 100%;
    padding: var(--space-6);
    box-sizing: border-box;
  }
  h1,
  h2,
  p,
  ul {
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
  .line {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-5);
  }
  .search {
    flex: 1 1 260px;
    max-width: 420px;
    min-width: 0;
  }
  .target {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  /* The tabs and the sort share one underline: the bar draws it, the tabs sit on it. */
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0 var(--space-6);
    margin-bottom: var(--space-1);
    border-bottom: var(--border-chrome) solid var(--edge);
  }
  .tabs {
    flex: 1 1 480px;
    min-width: 0;
  }
  .tabs :global(.cb-tabs) {
    border-bottom: 0;
  }
  .tabs :global(.cb-tab) {
    margin-bottom: calc(-1 * var(--border-chrome));
  }
  .sort {
    display: flex;
    align-items: center;
  }
  .ink {
    color: var(--ink);
  }
  /* Modpack sources */
  .sources {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .source {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 36px;
    padding: 0 var(--space-3) 0 var(--space-2);
    font: inherit;
    color: var(--ink-muted);
    background: transparent;
    border: var(--border-block) solid transparent;
    cursor: pointer;
  }
  .source:hover {
    color: var(--ink);
    background: var(--surface);
  }
  .source.is-on {
    color: var(--ink);
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .source:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .soon {
    max-width: 720px;
  }
  .embed {
    display: flex;
    flex-direction: column;
  }
  .embed :global(.browser) {
    padding: 0;
    overflow: visible;
  }
  .layout {
    display: grid;
    grid-template-columns: 230px minmax(0, 1fr);
    gap: var(--space-6);
    align-items: start;
  }

  /* filters */
  .filters {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    min-width: 0;
  }
  .filters h2 {
    margin-bottom: var(--space-2);
  }
  .filters ul {
    display: flex;
    flex-direction: column;
    padding: 0;
    list-style: none;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: var(--size-control-sm);
    padding: 2px 6px;
    font: inherit;
    text-align: start;
    color: var(--ink);
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .check:hover {
    background: var(--surface);
  }
  .check:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .box {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    box-sizing: border-box;
    width: 18px;
    height: 18px;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .box.on {
    color: var(--on-grass);
    background: var(--grass);
    border-color: var(--edge);
    box-shadow: none;
  }
  .hint {
    margin-top: var(--space-2);
    padding-inline: 6px;
  }

  /* results */
  .results {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }
  .about {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 10px var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px var(--space-2);
  }
  .info-ic {
    display: inline-flex;
    color: var(--diamond-text);
  }
  .new {
    padding-top: var(--space-2);
    border-top: var(--border-hair) solid var(--line);
  }
  .link {
    padding: 0;
    font: inherit;
    color: var(--grass-text);
    text-decoration: underline;
    text-underline-offset: 3px;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .link:hover {
    color: var(--ink);
  }
  .link:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  #discover-results {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    padding: 0;
    list-style: none;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: var(--space-2) var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .card:not(.skeleton):hover {
    background: var(--surface-hover);
  }
  .card:has(.title:focus-visible) {
    box-shadow: var(--focus-ring), var(--shadow-block-sm);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .row.spread {
    flex-wrap: wrap;
    justify-content: space-between;
    gap: var(--space-1) var(--space-3);
  }
  .head {
    flex: 1;
    min-width: 0;
  }
  .name-line {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    white-space: nowrap;
  }
  /* the title button covers the whole card, so a click anywhere opens the project */
  .title {
    flex: none;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0;
    color: var(--ink);
    text-align: start;
    text-decoration: underline;
    text-underline-offset: 3px;
    background: none;
    border: 0;
    cursor: pointer;
    outline: none;
  }
  .title::after {
    content: '';
    position: absolute;
    inset: 0;
  }
  .author {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .summary-text {
    display: -webkit-box;
    margin-top: 2px;
    overflow: hidden;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    min-width: 0;
  }
  .stats {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    white-space: nowrap;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .act {
    position: relative;
    z-index: 1;
  }
  .fit {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .fit.ok {
    color: var(--grass-text);
  }
  .fit.warn {
    color: var(--gold-text);
  }
  .fit.bad {
    color: var(--redstone-text);
  }
  .fit.busy,
  .fit.info {
    color: var(--diamond-text);
  }
  .fit.have {
    color: var(--ink);
  }
  .fit.have :global(.cb-icon) {
    color: var(--grass-text);
  }

  /* loading */
  .skeleton {
    gap: var(--space-2);
    min-height: 116px;
  }
  .sk-icon {
    flex: none;
    width: 32px;
    height: 32px;
    background: var(--surface-hover);
  }
  .sk-lines {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
  }
  .sk {
    display: block;
    height: 12px;
    background: var(--surface-hover);
  }
  .w40 {
    width: 40%;
  }
  .w60 {
    width: 60%;
  }
  .w70 {
    width: 70%;
  }
  .w90 {
    width: 90%;
  }

  /* empty */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-6);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .empty-ic {
    display: flex;
    color: var(--ink-muted);
  }
  .empty-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  /* pages */
  .pages {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .page {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    min-width: var(--size-control-sm);
    height: var(--size-control-sm);
    padding: 0 6px;
    font-family: var(--font-sans);
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .page:hover {
    background: var(--surface-hover);
  }
  .page.on {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .page:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .page:disabled {
    cursor: default;
  }

  /* picks */
  .picks {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
    gap: var(--space-3) var(--space-5);
    margin-top: var(--space-1);
  }
  .picks-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }
  .picks-two {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-width: 0;
    padding: var(--space-2) 10px;
    font: inherit;
    text-align: start;
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: pointer;
  }
  .pick:hover {
    background: var(--surface-hover);
  }
  .pick:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring), var(--shadow-block-sm);
  }
  .pick-text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  @media (max-width: 1100px) {
    .layout {
      grid-template-columns: 190px minmax(0, 1fr);
      gap: var(--space-5);
    }
    .grid,
    .picks,
    .picks-two {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
