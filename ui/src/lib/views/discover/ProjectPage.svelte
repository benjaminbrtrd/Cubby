<!-- A Modrinth project: one page for mods, modpacks, shader packs and resource packs, with the install
     panel on the side. Everything shown comes from Modrinth or from the instance; nothing is estimated. -->
<script lang="ts">
  import { api } from '../../api'
  import { Banner, Button, Icon, IconButton, InstanceIcon, Kbd, MetaChip, Switch, Tabs } from '../../cubby'
  import type { IconName } from '../../cubby'
  import { bytes, compactNum, date, num, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Dependency, DependencyKind, Project, ProjectKind, ProjectVersion } from '../../types'
  import InstallPanel from './InstallPanel.svelte'
  import Markdown from './Markdown.svelte'
  import {
    CHANNEL,
    KINDS,
    bestVersion,
    categoryLabel,
    fits,
    loaderLabel,
    mcRange,
    modLoadersOf,
    modrinthUrl,
    newestMc,
    primarySize,
    resolutionOf,
    serverHint,
    sideText,
  } from './discover'

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

  const kind = $derived(project?.kind ?? routeKind)
  const words = $derived(KINDS[kind])
  const target = $derived(kind === 'modpack' ? undefined : app.target)

  $effect(() => {
    if (target) void app.loadSources(target.id)
  })

  /** The version shown in the install panel (null = the best one for the target). */
  let versionId = $state<string | null>(null)
  const best = $derived(versions ? bestVersion(versions, kind, target) : undefined)
  const chosen = $derived(versions?.find((v) => v.id === versionId) ?? best ?? versions?.[0])
  const newestRelease = $derived(versions?.find((v) => v.channel === 'release'))

  const icon = $derived(project?.iconUrl ? { src: project.iconUrl } : projectArt(project?.slug ?? idOrSlug))
  const featured = $derived(project ? (project.featuredImage ?? project.gallery.find((g) => g.featured)?.url ?? project.gallery[0]?.url ?? null) : null)
  const pageUrl = $derived(project ? modrinthUrl(kind, project.slug) : '')
  const authorUrl = $derived(project ? `https://modrinth.com/user/${encodeURIComponent(project.author)}` : '')

  // tabs
  type Tab = 'description' | 'gallery' | 'versions' | 'changelog' | 'dependencies'
  let tab = $state<Tab>('description')

  const deps = $derived(chosen?.dependencies ?? [])
  const tabs = $derived.by(() => {
    const out: { id: Tab; label: string; count?: string; panel: string }[] = [{ id: 'description', label: 'Description', panel: 'pp-description' }]
    if (project?.gallery.length) out.push({ id: 'gallery', label: 'Gallery', count: num(project.gallery.length), panel: 'pp-gallery' })
    out.push({ id: 'versions', label: 'Versions', count: project ? num(Math.max(project.versionCount, versions?.length ?? 0)) : undefined, panel: 'pp-versions' })
    out.push({ id: 'changelog', label: 'Changelog', panel: 'pp-changelog' })
    if (kind === 'mod') out.push({ id: 'dependencies', label: 'Dependencies', count: num(deps.filter((d) => d.kind !== 'embedded').length), panel: 'pp-dependencies' })
    return out
  })

  // versions and changelog
  let onlyFits = $state(true)
  let versionLimit = $state(20)
  let changelogLimit = $state(8)
  const filtering = $derived(onlyFits && !!target)
  const listed = $derived(versions ? (filtering ? versions.filter((v) => fits(v, kind, target)) : versions) : [])

  function choose(v: ProjectVersion) {
    versionId = v.id
  }

  const loaderColumn = $derived(kind === 'mod' ? 'Loaders' : kind === 'modpack' ? 'Loader' : kind === 'shader' ? 'Runs with' : null)
  function loadersOf(v: ProjectVersion): string {
    const list = kind === 'mod' || kind === 'modpack' ? modLoadersOf(v.loaders) : v.loaders
    return list.map(loaderLabel).join(', ') || '—'
  }
  const columns = $derived(loaderColumn ? 8 : 7)

  /** Why the best version is the best one, in words. */
  const bestWhy = $derived.by(() => {
    if (!best || !target) return ''
    const what = best.channel === 'release' ? 'Newest release' : 'Newest version'
    if (kind === 'mod' && target.loader.kind !== 'vanilla') return `${what} for Minecraft ${target.minecraft} and ${loaderLabel(target.loader.kind)}`
    return `${what} for Minecraft ${target.minecraft}`
  })

  // dependencies
  const DEP_GROUPS: { kind: DependencyKind; title: string; hint: string }[] = [
    { kind: 'required', title: 'Required', hint: 'Needed to run. Cubby installs them for you.' },
    { kind: 'optional', title: 'Optional', hint: 'Adds extras. Install them yourself if you want them.' },
    { kind: 'incompatible', title: 'Incompatible', hint: "Don't use them together." },
    { kind: 'embedded', title: 'Included', hint: 'Already inside the file. Nothing to install.' },
  ]

  function depName(d: Dependency): string {
    return d.title ?? d.slug ?? 'Unknown project'
  }

  function depStatus(d: Dependency): { tone: string; icon: IconName; word: string } {
    const have = d.projectId && target ? app.installedIn(target.id, d.projectId) : undefined
    switch (d.kind) {
      case 'required':
        if (!target) return { tone: 'muted', icon: 'download', word: 'Installed with it' }
        return have ? { tone: 'ok', icon: 'check', word: 'Already installed' } : { tone: 'busy', icon: 'download', word: 'Will be installed' }
      case 'optional':
        if (!target) return { tone: 'muted', icon: 'info', word: 'Not installed with it' }
        return have ? { tone: 'ok', icon: 'check', word: `Installed in ${target.name}` } : { tone: 'muted', icon: 'info', word: 'Not installed' }
      case 'incompatible':
        if (!target) return { tone: 'muted', icon: 'warning', word: 'Check your instance' }
        return have ? { tone: 'bad', icon: 'error', word: `In ${target.name}: remove it first` } : { tone: 'ok', icon: 'check', word: `Not in ${target.name}` }
      default:
        return { tone: 'muted', icon: 'check', word: 'Inside this file' }
    }
  }

  function openDep(d: Dependency) {
    const id = d.slug ?? d.projectId
    if (id) app.openProject(id, 'mod')
  }

  const optionalDeps = $derived(deps.filter((d) => d.kind === 'optional'))

  // gallery viewer
  let viewing = $state<number | null>(null)
  let viewer: HTMLDivElement | undefined = $state()
  let returnFocus: HTMLElement | null = null

  function openImage(i: number, e: MouseEvent) {
    returnFocus = e.currentTarget as HTMLElement
    viewing = i
  }

  function closeViewer() {
    viewing = null
    returnFocus?.focus()
  }

  function move(by: number) {
    if (viewing === null || !project) return
    const n = project.gallery.length
    viewing = (viewing + by + n) % n
  }

  $effect(() => {
    if (viewing !== null && viewer) viewer.querySelector<HTMLElement>('.viewer-close button, button')?.focus()
  })

  // Only while the viewer is open: Esc closes it, the arrows move, Tab stays inside.
  function onWindowKey(e: KeyboardEvent) {
    if (viewing === null || !viewer) return
    if (e.key === 'Escape') {
      e.preventDefault()
      closeViewer()
    } else if (e.key === 'ArrowLeft' && !e.altKey) {
      e.preventDefault()
      move(-1)
    } else if (e.key === 'ArrowRight' && !e.altKey) {
      e.preventDefault()
      move(1)
    } else if (e.key === 'Tab') {
      const items = Array.from(viewer.querySelectorAll<HTMLElement>('button:not(:disabled)'))
      if (!items.length) return
      const at = items.indexOf(document.activeElement as HTMLElement)
      e.preventDefault()
      items[(at + (e.shiftKey ? -1 : 1) + items.length) % items.length].focus()
    }
  }

  // links
  function open(url: string) {
    void api.openUrl(url).catch((e) => app.error(e))
  }

  function follow(e: MouseEvent, url: string) {
    e.preventDefault()
    if (e.type === 'auxclick' && e.button !== 1) return
    open(url)
  }

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(pageUrl)
      app.toast('Link copied', 'success')
    } catch (e) {
      app.error(e)
    }
  }

  const links = $derived.by(() => {
    if (!project) return []
    const out: { icon: IconName; label: string; url: string }[] = [{ icon: 'globe', label: 'Page on Modrinth', url: pageUrl }]
    if (project.links.source) out.push({ icon: 'terminal', label: 'Source code', url: project.links.source })
    if (project.links.issues) out.push({ icon: 'warning', label: 'Report an issue', url: project.links.issues })
    if (project.links.wiki) out.push({ icon: 'library', label: 'Wiki', url: project.links.wiki })
    if (project.links.discord) out.push({ icon: 'accounts', label: 'Discord', url: project.links.discord })
    for (const d of project.links.donation) out.push({ icon: 'globe', label: `Donate on ${d.platform}`, url: d.url })
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

  const lower = (s: string) => s.charAt(0).toLowerCase() + s.slice(1)
  const side = $derived(project ? sideText(project.clientSide, project.serverSide) : null)
  const resolution = $derived(project ? resolutionOf(project.categories) : null)
  const packMc = $derived(chosen ? newestMc(chosen.gameVersions) : null)
  const packLoader = $derived(chosen ? modLoadersOf(chosen.loaders).map(loaderLabel)[0] : undefined)
</script>

<svelte:window onkeydown={onWindowKey} />

{#snippet chip(text: string, iconName?: IconName, title?: string)}
  <MetaChip icon={iconName} {title}>{text}</MetaChip>
{/snippet}

<div class="screen">
  <div class="layout">
    <div class="main">
      <nav aria-label="Breadcrumb">
        <ol class="crumbs">
          <li><button type="button" class="link text-body-strong" onclick={() => backToDiscover(false)}>Discover</button></li>
          <li aria-hidden="true" class="sep"><Icon name="chevron-right" size={18} /></li>
          <li><button type="button" class="link text-body" onclick={() => backToDiscover(true)}>{words.label}</button></li>
          <li aria-hidden="true" class="sep"><Icon name="chevron-right" size={18} /></li>
          <li class="text-body" aria-current="page">{project?.title ?? idOrSlug}</li>
        </ol>
      </nav>

      {#if error && !project}
        {#snippet errorActions()}
          <Button size="sm" icon="update" onclick={() => tries++}>Try again</Button>
          <Button size="sm" variant="ghost" onclick={() => backToDiscover(false)}>Back to Discover</Button>
        {/snippet}
        <Banner tone="warning" icon="offline" title="Couldn't open this {words.one}" actions={errorActions}>
          {error.replace(/\.\s*$/, '')}. Check your internet connection, then try again.
        </Banner>
      {:else if !project}
        <div class="header-skeleton" aria-hidden="true">
          <span class="sk-icon"></span>
          <span class="sk-lines"><span class="sk w40 tall"></span><span class="sk w90"></span><span class="sk w60"></span></span>
        </div>
        <span class="cb-visually-hidden" role="status">Loading the {words.one}</span>
      {:else}
        <header class="header">
          <InstanceIcon {...icon} size={64} raised label={project.title} />
          <div class="header-text">
            <div class="title-line">
              <h1 class="text-title">{project.title}</h1>
              <span class="text-body muted">
                by <a href={authorUrl} class="author" onclick={(e) => follow(e, authorUrl)} onauxclick={(e) => follow(e, authorUrl)}>{project.author}</a>
              </span>
            </div>
            <p class="text-body">{project.summary}</p>
            <div class="chips">
              {#each project.categories as c (c)}{@render chip(categoryLabel(c))}{/each}
              {#if kind === 'mod' && side}{@render chip(side, 'servers', 'Where it runs')}{/if}
              {#if (kind === 'mod' || kind === 'modpack') && modLoadersOf(project.loaders).length}
                {@render chip(modLoadersOf(project.loaders).map(loaderLabel).join(', '), undefined, 'Loaders')}
              {/if}
              {#if kind === 'shader' && project.loaders.length}{@render chip(`Runs with ${project.loaders.map(loaderLabel).join(', ')}`)}{/if}
              {#if project.gameVersions.length}{@render chip(mcRange(project.gameVersions), undefined, 'Minecraft versions')}{/if}
              {#if project.license}{@render chip(project.license, undefined, 'License')}{/if}
            </div>
            <ul class="stats text-small" aria-label="Project stats">
              <li><Icon name="download" size={18} />{compactNum(project.downloads)} downloads</li>
              <li><Icon name="accounts" size={18} />{compactNum(project.follows)} followers</li>
              <li><Icon name="clock" size={18} />Updated {lower(relativeTime(project.updated, 'never'))}</li>
              <li><Icon name="globe" size={18} />Source: Modrinth</li>
            </ul>
          </div>
          <div class="header-actions">
            <IconButton icon="copy" label="Copy link" onclick={copyLink} />
            <IconButton icon="globe" label="Open on Modrinth" onclick={() => open(pageUrl)} />
          </div>
        </header>

        {#if kind === 'modpack'}
          <Banner tone="info" title="What is a modpack?">
            <p>
              A modpack is a set of mods chosen and tested together. Cubby installs it as a new instance with everything set up:
              Minecraft {packMc ?? ''}{packLoader ? `, the ${packLoader} loader` : ''} and every mod with its settings, in one step. Your other instances stay as they are.
            </p>
          </Banner>
        {:else if kind === 'shader' || kind === 'resourcepack'}
          <section class={['what', !featured && 'no-image']} aria-labelledby="pp-what">
            {#if featured}
              <figure>
                <img src={featured} alt="{project.title} in game" loading="lazy" />
                <figcaption class="text-body-strong">With {project.title}</figcaption>
              </figure>
            {/if}
            <div class="what-text">
              {#if kind === 'shader'}
                <h2 id="pp-what" class="text-body-strong">What is a shader pack?</h2>
                <p class="text-small muted">It changes how light, shadows, water and sky are drawn. Blocks, mobs and the way you play stay the same, so it's safe in any world.</p>
                <p class="text-small muted">
                  It runs through a mod: Iris, which needs Sodium, or OptiFine. Both Iris and Sodium are in Discover, under Mods. With Iris, press <Kbd keys="K" /> in game to turn shaders on or off.
                </p>
              {:else}
                <h2 id="pp-what" class="text-body-strong">What is a resource pack?</h2>
                <p class="text-small muted">It changes how blocks, items, mobs and menus look, and sometimes how they sound. It adds nothing new, and the game plays the same.</p>
                {#if resolution}
                  <p class="text-small muted">
                    {resolution}x means each texture is {resolution} × {resolution} pixels. Minecraft's own are 16 × 16.{resolution > 32 ? ' Bigger textures need more video memory.' : ''}
                  </p>
                {/if}
                <p class="text-small muted">
                  The order matters: when two packs change the same thing, the one higher in the list wins. Press <Kbd keys="F3" /> + <Kbd keys="T" /> in game to reload them.
                </p>
              {/if}
            </div>
          </section>
        {/if}

        <Tabs label="{words.label} sections" items={tabs} value={tab} onChange={(id) => (tab = id as Tab)} />

        {#if tab === 'description'}
          <div id="pp-description" role="tabpanel" aria-label="Description" class="description">
            <div class="body">
              {#if project.body.trim()}
                <Markdown source={project.body} />
              {:else}
                <p class="text-body muted">The author hasn't written a description yet.</p>
              {/if}
            </div>
            <div class="side">
              {#if kind === 'mod' && optionalDeps.length}
                <section aria-labelledby="pp-pairs">
                  <h2 id="pp-pairs" class="text-heading">Works well with</h2>
                  <p class="text-small muted">Optional mods that add extras to it.</p>
                  <ul class="pairs">
                    {#each optionalDeps as dep (dep.projectId ?? dep.slug ?? dep.title)}
                      {@const st = depStatus(dep)}
                      <li>
                        <InstanceIcon {...dep.iconUrl ? { src: dep.iconUrl } : projectArt(dep.slug ?? dep.projectId ?? 'x')} size={32} />
                        <span class="pair-text">
                          {#if dep.slug || dep.projectId}
                            <button type="button" class="link text-body-strong" onclick={() => openDep(dep)}>{depName(dep)}</button>
                          {:else}
                            <span class="text-body-strong">{depName(dep)}</span>
                          {/if}
                          <span class={['text-small', 'status', st.tone]}><Icon name={st.icon} size={18} />{st.word}</span>
                        </span>
                      </li>
                    {/each}
                  </ul>
                </section>
              {/if}
              <section aria-labelledby="pp-faq">
                <h2 id="pp-faq" class="text-heading">Good to know</h2>
                <dl class="faq">
                  {#if kind === 'mod'}
                    {@const hint = serverHint(project.clientSide, project.serverSide)}
                    {#if hint}
                      <div><dt class="text-body-strong">Does it work on servers?</dt><dd class="text-body muted">{hint}</dd></div>
                    {/if}
                    <div>
                      <dt class="text-body-strong">Which version do I need?</dt>
                      <dd class="text-body muted">The one for your instance's Minecraft version and loader. Cubby picks it for you and adds the mods it needs.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Can I remove it later?</dt>
                      <dd class="text-body muted">Yes. Turn it off or remove it in the instance's Mods tab. Your worlds stay, but what the mod added disappears from them.</dd>
                    </div>
                  {:else if kind === 'modpack'}
                    <div>
                      <dt class="text-body-strong">Can I add my own mods?</dt>
                      <dd class="text-body muted">Yes. It's a normal instance: add or turn off mods in its Mods tab.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Can I play with friends?</dt>
                      <dd class="text-body muted">Yes, if everyone has the same version of the pack{chosen ? ` (${chosen.versionNumber})` : ''}. A server needs the same mods too.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Does it change my other instances?</dt>
                      <dd class="text-body muted">No. It gets its own instance, with its own worlds and settings.</dd>
                    </div>
                  {:else if kind === 'shader'}
                    <div>
                      <dt class="text-body-strong">Does it change my worlds?</dt>
                      <dd class="text-body muted">No. Shaders only change how the game is drawn. Turn them off and everything looks as before.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Will it run on my PC?</dt>
                      <dd class="text-body muted">Shaders work your graphics card harder than the default look. If the game gets slow, lower the pack's settings in game, or turn it off.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Where do I turn it on?</dt>
                      <dd class="text-body muted">In the instance's Shaders tab, or in game: Options, Video settings, Shader packs.</dd>
                    </div>
                  {:else}
                    <div>
                      <dt class="text-body-strong">Does it change my worlds?</dt>
                      <dd class="text-body muted">No. Only how things look or sound. Turn it off and the game looks as before.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Does it work on servers?</dt>
                      <dd class="text-body muted">Yes. It stays on your side, and only you see it. Some servers send their own pack.</dd>
                    </div>
                    <div>
                      <dt class="text-body-strong">Where do I turn it on?</dt>
                      <dd class="text-body muted">In the instance's Resource packs tab, or in game: Options, Resource packs.</dd>
                    </div>
                  {/if}
                </dl>
              </section>
            </div>
          </div>
        {:else if tab === 'gallery'}
          <div id="pp-gallery" role="tabpanel" aria-label="Gallery" class="panel">
            <p class="text-small muted">{plural(project.gallery.length, 'image', 'images')} from the author. Select one to see it larger.</p>
            <ul class="gallery">
              {#each project.gallery as g, i (g.url + i)}
                <li>
                  <button type="button" class="thumb" aria-label="Open image {i + 1} of {project.gallery.length}{g.title ? `: ${g.title}` : ''}" onclick={(e) => openImage(i, e)}>
                    <img src={g.url} alt="" loading="lazy" />
                  </button>
                  {#if g.title}<span class="text-small caption">{g.title}</span>{/if}
                </li>
              {/each}
            </ul>
          </div>
        {:else if tab === 'versions'}
          <div id="pp-versions" role="tabpanel" aria-label="Versions" class="panel">
            <div class="panel-head">
              <p class="text-small muted">
                {target ? `Cubby picks the best one for ${target.name}.` : 'Newest first.'} Betas can have bugs.
              </p>
              <span class="grow"></span>
              {#if target}
                <Switch checked={onlyFits} onChange={(v) => ((onlyFits = v), (versionLimit = 20))} label="Only for {target.name}" />
              {/if}
            </div>
            {#if versionsError}
              {#snippet retryVersions()}<Button size="sm" icon="update" onclick={() => tries++}>Try again</Button>{/snippet}
              <Banner tone="warning" title="Couldn't load the versions" actions={retryVersions}>{versionsError}</Banner>
            {:else if !versions}
              <div class="table-skeleton" aria-hidden="true">{#each { length: 5 }, i (i)}<span class="sk"></span>{/each}</div>
            {:else if !listed.length}
              <div class="empty">
                <p class="text-body">{filtering && target ? `No version for ${target.name} (${target.minecraft}) yet.` : 'No versions yet.'}</p>
                {#if filtering}<Button size="sm" onclick={() => (onlyFits = false)}>Show all versions</Button>{/if}
              </div>
            {:else}
              <div class="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th scope="col" class="text-label">Version</th>
                      <th scope="col" class="text-label">Minecraft</th>
                      {#if loaderColumn}<th scope="col" class="text-label">{loaderColumn}</th>{/if}
                      <th scope="col" class="text-label">Channel</th>
                      <th scope="col" class="text-label">Date</th>
                      <th scope="col" class="text-label end">Downloads</th>
                      <th scope="col" class="text-label end">Size</th>
                      <th scope="col" class="text-label end"><span class="cb-visually-hidden">Action</span></th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each listed.slice(0, versionLimit) as v (v.id)}
                      {@const isBest = !!best && v.id === best.id && !!target}
                      {@const isChosen = !!chosen && v.id === chosen.id}
                      {#if isBest}
                        <tr class="best-note">
                          <td colspan={columns}>
                            <span class="text-body-strong best-word"><Icon name="check" size={18} />Best for {target?.name}</span>
                            <span class="text-small muted">{bestWhy}</span>
                          </td>
                        </tr>
                      {/if}
                      <tr class={[isBest && 'best', isChosen && 'chosen']}>
                        <td class="text-mono">{v.versionNumber}</td>
                        <td class="text-mono">{mcRange(v.gameVersions)}</td>
                        {#if loaderColumn}<td class="text-mono">{loadersOf(v)}</td>{/if}
                        <td><MetaChip icon={CHANNEL[v.channel].icon} tone={CHANNEL[v.channel].tone} title={CHANNEL[v.channel].hint}>{CHANNEL[v.channel].word}</MetaChip></td>
                        <td class="text-body" title={date(v.published)}>{date(v.published)}</td>
                        <td class="text-counter end">{compactNum(v.downloads)}</td>
                        <td class="text-mono end">{bytes(primarySize(v))}</td>
                        <td class="end">
                          {#if isChosen}
                            <span class="text-small chosen-word"><Icon name="check" size={18} />Chosen</span>
                          {:else}
                            <Button size="sm" variant="ghost" onclick={() => choose(v)} aria-label="Choose version {v.versionNumber}">Choose</Button>
                          {/if}
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
              {#if listed.length > versionLimit}
                <Button size="sm" onclick={() => (versionLimit += 20)}>Show more ({num(listed.length - versionLimit)} left)</Button>
              {/if}
            {/if}
          </div>
        {:else if tab === 'changelog'}
          <div id="pp-changelog" role="tabpanel" aria-label="Changelog" class="panel">
            <div class="panel-head">
              <p class="text-small muted">What changed in each version, newest first.</p>
              <span class="grow"></span>
              {#if target}
                <Switch checked={onlyFits} onChange={(v) => ((onlyFits = v), (changelogLimit = 8))} label="Only for {target.name}" />
              {/if}
            </div>
            {#if versionsError}
              <Banner tone="warning" title="Couldn't load the changelog">{versionsError}</Banner>
            {:else if !versions}
              <div class="table-skeleton" aria-hidden="true">{#each { length: 3 }, i (i)}<span class="sk tall"></span>{/each}</div>
            {:else if !listed.length}
              <div class="empty">
                <p class="text-body">{filtering && target ? `No version for ${target.name} yet.` : 'No versions yet.'}</p>
                {#if filtering}<Button size="sm" onclick={() => (onlyFits = false)}>Show all versions</Button>{/if}
              </div>
            {:else}
              <ol class="changes">
                {#each listed.slice(0, changelogLimit) as v (v.id)}
                  <li>
                    <div class="change-head">
                      <h3 class="text-body-strong"><span class="text-mono">{v.versionNumber}</span></h3>
                      <MetaChip icon={CHANNEL[v.channel].icon} tone={CHANNEL[v.channel].tone}>{CHANNEL[v.channel].word}</MetaChip>
                      <span class="text-small muted">{date(v.published)} · Minecraft <span class="text-mono">{mcRange(v.gameVersions)}</span></span>
                      {#if v.id === newestRelease?.id}<span class="text-small good">Latest release</span>{/if}
                    </div>
                    {#if v.changelog.trim()}
                      <Markdown source={v.changelog} level={4} />
                    {:else}
                      <p class="text-small muted">No notes for this version.</p>
                    {/if}
                  </li>
                {/each}
              </ol>
              {#if listed.length > changelogLimit}
                <Button size="sm" onclick={() => (changelogLimit += 8)}>Show older versions</Button>
              {/if}
            {/if}
          </div>
        {:else if tab === 'dependencies'}
          <div id="pp-dependencies" role="tabpanel" aria-label="Dependencies" class="panel">
            <p class="text-small muted">
              {#if chosen}For version <span class="text-mono">{chosen.versionNumber}</span>. {/if}Cubby adds required mods for you and warns you about conflicts.
            </p>
            {#if !versions && !versionsError}
              <div class="table-skeleton" aria-hidden="true">{#each { length: 3 }, i (i)}<span class="sk tall"></span>{/each}</div>
            {:else if !deps.length}
              <div class="empty"><p class="text-body">Nothing else needed. It works on its own.</p></div>
            {:else}
              <div class="dep-groups">
                {#each DEP_GROUPS as g (g.kind)}
                  {@const list = deps.filter((d) => d.kind === g.kind)}
                  {#if list.length || g.kind !== 'embedded'}
                    <section class="dep-group" aria-labelledby="pp-dep-{g.kind}">
                      <div>
                        <h3 id="pp-dep-{g.kind}" class={['text-label', 'dep-title', g.kind === 'incompatible' && 'bad']}>
                          {#if g.kind === 'incompatible'}<Icon name="error" size={18} />{/if}{g.title}
                        </h3>
                        <p class="text-small muted">{g.hint}</p>
                      </div>
                      {#if !list.length}
                        <p class="text-small muted">None</p>
                      {/if}
                      {#each list as dep, i (i)}
                        {@const st = depStatus(dep)}
                        <div class="dep">
                          {#if dep.slug || dep.projectId}
                            <button type="button" class="link text-body-strong" onclick={() => openDep(dep)}>{depName(dep)}</button>
                          {:else}
                            <p class="text-body-strong">{depName(dep)}</p>
                          {/if}
                          <p class={['text-small', 'status', st.tone]}><Icon name={st.icon} size={18} />{st.word}</p>
                        </div>
                      {/each}
                    </section>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/if}
    </div>

    <aside aria-label="Install {project?.title ?? ''}">
      {#if project}
        <InstallPanel {project} {versions} {versionsError} bind:versionId />

        {#if links.length}
          <section class="aside-section" aria-labelledby="pp-links">
            <h3 id="pp-links" class="text-label muted">Links</h3>
            <ul class="links">
              {#each links as l (l.url + l.label)}
                <li>
                  <a href={l.url} class="text-body" title={l.url} onclick={(e) => follow(e, l.url)} onauxclick={(e) => follow(e, l.url)}>
                    <Icon name={l.icon} size={18} />{l.label}
                  </a>
                </li>
              {/each}
            </ul>
          </section>
        {/if}

        <section class="aside-section" aria-labelledby="pp-details">
          <h3 id="pp-details" class="text-label muted">Details</h3>
          <dl class="details">
            {#if project.license}<dt class="text-small muted">License</dt><dd class="text-mono">{project.license}</dd>{/if}
            {#if kind === 'mod' && side}<dt class="text-small muted">Runs on</dt><dd class="text-body">{side}</dd>{/if}
            {#if project.gameVersions.length}<dt class="text-small muted">Minecraft</dt><dd class="text-mono">{mcRange(project.gameVersions)}</dd>{/if}
            {#if project.loaders.length && kind !== 'resourcepack'}
              <dt class="text-small muted">{kind === 'shader' ? 'Runs with' : 'Loaders'}</dt>
              <dd class="text-mono">{(kind === 'shader' ? project.loaders : modLoadersOf(project.loaders)).map(loaderLabel).join(', ') || '—'}</dd>
            {/if}
            <dt class="text-small muted">Versions</dt><dd class="text-body">{num(Math.max(project.versionCount, versions?.length ?? 0))}</dd>
            <dt class="text-small muted">Source</dt><dd class="text-body">Modrinth</dd>
            <dt class="text-small muted">Since</dt><dd class="text-body">{date(project.published)}</dd>
          </dl>
        </section>
      {:else}
        <span class="aside-skeleton" aria-hidden="true"></span>
      {/if}
    </aside>
  </div>
</div>

{#if viewing !== null && project && project.gallery[viewing]}
  {@const img = project.gallery[viewing]}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="cb-dialog-layer viewer-layer" onclick={(e) => e.target === e.currentTarget && closeViewer()}>
    <div bind:this={viewer} class="viewer" role="dialog" aria-modal="true" aria-label="Image {viewing + 1} of {project.gallery.length}">
      <div class="viewer-head">
        <div class="viewer-text">
          <p class="text-body-strong">{img.title ?? project.title}</p>
          {#if img.description}<p class="text-small muted">{img.description}</p>{/if}
        </div>
        <span class="text-counter muted">{viewing + 1} / {project.gallery.length}</span>
        <span class="viewer-close"><IconButton icon="close" label="Close" onclick={closeViewer} /></span>
      </div>
      <div class="viewer-stage">
        <img src={img.url} alt={img.title ?? `${project.title}, image ${viewing + 1}`} />
      </div>
      {#if project.gallery.length > 1}
        <div class="viewer-nav">
          <Button size="sm" onclick={() => move(-1)}><span class="flip"><Icon name="chevron-right" size={18} /></span>Previous</Button>
          <span class="text-small muted">Use the arrow keys to move, Esc to close.</span>
          <Button size="sm" iconEnd="chevron-right" onclick={() => move(1)}>Next</Button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .layout {
    flex: 1;
    display: flex;
    align-items: stretch;
    min-width: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  aside {
    flex: none;
    box-sizing: border-box;
    width: var(--size-inspector);
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  h1,
  h2,
  h3,
  p,
  ol,
  ul,
  dl,
  dd,
  figure {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .good,
  .ok {
    color: var(--grass-text);
  }
  .bad {
    color: var(--redstone-text);
  }
  .busy {
    color: var(--diamond-text);
  }
  .grow {
    flex: 1;
  }
  .link {
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
  .link:hover {
    color: var(--ink);
  }
  .link:focus-visible,
  a:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }

  /* breadcrumb and header */
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
  .header {
    display: flex;
    align-items: flex-start;
    gap: var(--space-5);
  }
  .header-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .title-line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-3);
  }
  .author {
    color: var(--grass-text);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 6px var(--space-5);
    padding: 0;
    list-style: none;
    color: var(--ink-muted);
  }
  .stats li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .header-actions {
    display: flex;
    gap: var(--space-1);
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
  .sk {
    display: block;
    height: 14px;
    background: var(--surface);
  }
  .sk.tall {
    height: 28px;
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

  /* what is it */
  .what {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--space-4);
    align-items: stretch;
  }
  .what.no-image {
    grid-template-columns: minmax(0, 1fr);
  }
  figure {
    position: relative;
  }
  figure img {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 160px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
    box-sizing: border-box;
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  figcaption {
    position: absolute;
    top: var(--space-2);
    inset-inline-start: var(--space-2);
    padding: 2px var(--space-2);
    background: var(--bg-chrome);
    border: var(--border-block) solid var(--edge);
  }
  .what-text {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) 14px;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }

  /* description */
  .description {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    gap: 28px;
    align-items: start;
    margin-top: var(--space-1);
  }
  .body {
    min-width: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    min-width: 0;
  }
  .side h2 {
    margin-bottom: var(--space-2);
  }
  .side h2 + p {
    margin: calc(-1 * var(--space-1)) 0 10px;
  }
  .pairs {
    padding: 0;
    list-style: none;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .pairs li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 10px var(--space-3);
  }
  .pairs li + li {
    border-top: var(--border-hair) solid var(--line);
  }
  .pair-text {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
  }
  .status {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .status.muted {
    color: var(--ink-muted);
  }
  .faq {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .faq > div + div {
    padding-top: 14px;
    border-top: var(--border-hair) solid var(--line);
  }
  .faq dd {
    margin-top: var(--space-1);
  }

  /* panels */
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
    margin-top: var(--space-1);
  }
  .panel > :global(.cb-btn) {
    align-self: flex-start;
  }
  .panel-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
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
  .table-skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  /* gallery */
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--space-3);
    padding: 0;
    list-style: none;
  }
  .gallery li {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }
  .thumb {
    display: block;
    padding: 0;
    line-height: 0;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: zoom-in;
  }
  .thumb:hover {
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block);
  }
  .thumb:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring), var(--shadow-block-sm);
  }
  .thumb img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .caption {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-muted);
  }

  /* versions */
  .table-wrap {
    overflow-x: auto;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    padding: 10px var(--space-2);
    text-align: start;
    white-space: nowrap;
    color: var(--ink-muted);
    background: var(--bg-chrome);
    border-bottom: var(--border-block) solid var(--edge);
  }
  td {
    padding: var(--space-2);
    vertical-align: middle;
    border-bottom: var(--border-hair) solid var(--line);
  }
  th:first-child,
  td:first-child {
    padding-inline-start: var(--space-3);
  }
  th:last-child,
  td:last-child {
    padding-inline-end: var(--space-3);
  }
  .end {
    text-align: end;
  }
  tr.best,
  tr.best-note {
    background: var(--surface-hover);
  }
  tr.best td:first-child,
  tr.best-note td {
    border-inline-start: 4px solid var(--grass);
  }
  tr.best-note td {
    padding: 10px var(--space-3) 0;
    border-bottom: 0;
  }
  tr.best-note td > span {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  tr.best-note td > span + span {
    margin-inline-start: var(--space-2);
  }
  .best-word {
    color: var(--grass-text);
  }
  tr.chosen td {
    box-shadow: inset 0 2px 0 var(--ink), inset 0 -2px 0 var(--ink);
  }
  tr.chosen td:first-child {
    box-shadow: inset 2px 2px 0 var(--ink), inset 0 -2px 0 var(--ink);
  }
  tr.chosen td:last-child {
    box-shadow: inset -2px 2px 0 var(--ink), inset 0 -2px 0 var(--ink);
  }
  .chosen-word {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--grass-text);
  }

  /* changelog */
  .changes {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: 0;
    list-style: none;
  }
  .changes > li {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .change-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-3);
  }

  /* dependencies */
  .dep-groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--space-3);
    align-items: stretch;
  }
  .dep-group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .dep-title {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dep {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .dep + .dep {
    padding-top: 10px;
    border-top: var(--border-hair) solid var(--line);
  }

  /* aside */
  .aside-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .links {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0;
    list-style: none;
  }
  .links a {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--grass-text);
  }
  .links a:hover {
    color: var(--ink);
  }
  .details {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
  }
  .aside-skeleton {
    display: block;
    height: 320px;
    background: var(--surface);
  }

  /* gallery viewer */
  .viewer {
    display: flex;
    flex-direction: column;
    width: min(1100px, 100%);
    max-height: calc(100vh - 48px);
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-float);
  }
  .viewer-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    background: var(--bg-chrome);
    border-bottom: var(--border-block) solid var(--edge);
  }
  .viewer-text {
    flex: 1;
    min-width: 0;
  }
  .viewer-stage {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-3);
    background: var(--field);
  }
  .viewer-stage img {
    display: block;
    max-width: 100%;
    max-height: calc(100vh - 200px);
    object-fit: contain;
    image-rendering: pixelated;
  }
  .viewer-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .flip {
    display: inline-flex;
    transform: scaleX(-1);
  }

  @media (max-width: 1100px) {
    aside {
      width: 300px;
      padding-inline: var(--space-4);
    }
    .description,
    .what {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
