<!-- A modpack from a provider: what it is, and the form that installs it as a new instance. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { untrack } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, InstanceIcon, MetaChip, Switch, type Ground, type Motif } from '../../cubby'
  import { compactNum, date, loaderName, plural, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { OptionalMod, PackDetails, PackProvider, PackSummary, PackVersion } from '../../types'
  import Markdown from '../discover/Markdown.svelte'
  import Picker from '../discover/Picker.svelte'
  import Identity, { iconValue } from './Identity.svelte'
  import { CHANNEL_WORD, mcSpan, paragraphs } from './packs'

  interface Props {
    provider: PackProvider
    summary: PackSummary
    onBack: () => void
  }

  let { provider, summary, onBack }: Props = $props()

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  // data

  let details = $state<PackDetails | null>(null)
  let detailsError = $state<string | null>(null)
  let versions = $state<PackVersion[] | null>(null)
  let versionsError = $state<string | null>(null)
  let tries = $state(0)

  $effect(() => {
    const id = summary.id
    const p = provider.id
    void tries
    let stale = false
    detailsError = null
    versionsError = null
    api
      .packDetails(p, id)
      .then((d) => {
        if (!stale) details = d
      })
      .catch((e) => {
        if (!stale) detailsError = message(e)
      })
    api
      .packVersions(p, id)
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

  // version

  let versionId = $state<string | null>(null)
  const chosen = $derived(versions?.find((v) => v.id === versionId) ?? versions?.find((v) => v.channel === 'release') ?? versions?.[0])
  const newest = $derived(versions?.find((v) => v.channel === 'release') ?? versions?.[0])
  const versionItems = $derived((versions ?? []).map((v) => ({ id: v.id, v })))

  function versionLine(v: PackVersion): string {
    return [v.minecraft, v.loader ? loaderName(v.loader) : null].filter(Boolean).join(' · ')
  }

  // optional mods (ATLauncher)

  let optional = $state<OptionalMod[] | null>(null)
  let optionalError = $state<string | null>(null)
  let ticked = $state<Record<string, boolean>>({})
  let optTries = $state(0)

  $effect(() => {
    const v = chosen?.id
    const p = provider.id
    const id = summary.id
    void optTries
    optional = null
    optionalError = null
    ticked = {}
    if (!v || p !== 'atlauncher') return
    let stale = false
    api
      .packOptionalMods(p, id, v)
      .then((list) => {
        if (stale) return
        const next: Record<string, boolean> = {}
        for (const m of list) if (m.selected) pick(list, next, m, true)
        ticked = next
        optional = list
      })
      .catch((e) => {
        if (stale) return
        optional = []
        optionalError = message(e)
      })
    return () => {
      stale = true
    }
  })

  /** Prism's rules: a group lets one mod through, a mod brings the mods it depends on, and switching one off switches off those that need it. */
  function pick(list: OptionalMod[], state: Record<string, boolean>, m: OptionalMod, on: boolean, seen = new Set<string>()) {
    if (seen.has(m.name)) return
    seen.add(m.name)
    if (on) {
      state[m.name] = true
      if (m.group) for (const o of list) if (o.group === m.group && o.name !== m.name) pick(list, state, o, false, seen)
      for (const dep of m.depends) {
        const d = list.find((o) => o.name === dep)
        if (d) pick(list, state, d, true, seen)
      }
    } else {
      state[m.name] = false
      for (const o of list) if (o.depends.includes(m.name) && state[o.name]) pick(list, state, o, false, seen)
    }
  }

  function toggle(m: OptionalMod, on: boolean) {
    if (!optional) return
    const next = { ...ticked }
    pick(optional, next, m, on)
    ticked = next
  }

  const tickedNames = $derived((optional ?? []).filter((m) => ticked[m.name]).map((m) => m.name))

  /** What a mod's choice does to the others, in words. */
  function notes(m: OptionalMod): string[] {
    const out: string[] = []
    if (m.depends.length) out.push(t('Needs {mods}', { mods: m.depends.join(', ') }))
    if (m.group) {
      const others = (optional ?? []).filter((o) => o.group === m.group && o.name !== m.name).map((o) => o.name)
      if (others.length) out.push(t('Pick one: replaces {mods}', { mods: others.join(', ') }))
    }
    return out
  }

  // name, group, icon

  let name = $state(untrack(() => summary.name))
  let group = $state('Modpacks')
  let motif = $state<Motif | undefined>()
  let ground = $state<Ground | undefined>()
  let src = $state<string | null>(untrack(() => summary.iconUrl))
  let iconTouched = $state(false)
  let submitted = $state(false)
  let busy = $state(false)
  const nameError = $derived(submitted && !name.trim() ? t('Give the instance a name.') : undefined)

  const warning = $derived(details?.warning ?? null)
  const canInstall = $derived(!warning && !!chosen && !busy)

  async function install() {
    submitted = true
    if (!canInstall || !chosen || !name.trim()) return
    busy = true
    try {
      const inst = await api.installPack(provider.id, summary.id, chosen.id, name.trim(), group.trim() || null, iconValue(src, motif, ground), provider.id === 'atlauncher' && optional?.length ? tickedNames : null)
      await app.refreshInstances()
      app.selected = inst.id
      app.openInstance(inst.id)
      app.toast(t('{name} is installing. You can follow it in Downloads.', { name: inst.name }), 'success', { label: t('Downloads'), run: () => app.navigate({ name: 'downloads' }) })
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  // links and gallery

  function open(e: MouseEvent, url: string) {
    e.preventDefault()
    void api.openUrl(url).catch((err) => app.error(err))
  }

  const links = $derived.by(() => {
    const list = details?.links ?? []
    if (list.length || !summary.websiteUrl) return list
    return [{ label: t('Page on {name}', { name: provider.name }), url: summary.websiteUrl }]
  })

  let viewing = $state<number | null>(null)
  const gallery = $derived(details?.gallery ?? [])
  const move = (by: number) => (viewing = viewing === null ? null : (viewing + by + gallery.length) % gallery.length)

  const art = $derived(summary.iconUrl ? { src: summary.iconUrl } : projectArt(summary.name))
  const lower = (s: string) => s.charAt(0).toLowerCase() + s.slice(1)
</script>

<div class="page">
  <div class="main">
    <nav aria-label={t('Breadcrumb')}>
      <ol class="crumbs">
        <li><button type="button" class="link text-body-strong" onclick={onBack}>{provider.name}</button></li>
        <li aria-hidden="true" class="sep"><Icon name="chevron-right" size={18} /></li>
        <li class="text-body" aria-current="page">{summary.name}</li>
      </ol>
    </nav>

    <header class="header">
      <InstanceIcon {...art} size={64} raised label={summary.name} />
      <div class="header-text">
        <div class="title-line">
          <h1 class="text-title">{summary.name}</h1>
          {#if summary.authors.length}<span class="text-body muted">{t('by {authors}', { authors: summary.authors.join(', ') })}</span>{/if}
        </div>
        {#if summary.summary}<p class="text-body">{summary.summary}</p>{/if}
        <div class="chips">
          {#if summary.minecraft.length}<MetaChip title={t('Minecraft versions')}>{mcSpan(summary.minecraft)}</MetaChip>{/if}
          {#if summary.loaders.length}<MetaChip title={t('Loaders')}>{summary.loaders.map((l) => loaderName(l)).join(', ')}</MetaChip>{/if}
        </div>
        <ul class="stats text-small">
          {#if summary.downloads != null}<li><Icon name="download" size={18} />{t('{n} downloads', { n: compactNum(summary.downloads) })}</li>{/if}
          {#if summary.updated}<li><Icon name="clock" size={18} />{t('Updated {never}', { never: lower(relativeTime(summary.updated, 'never')) })}</li>{/if}
          <li><Icon name="globe" size={18} />{t('Source: {name}', { name: provider.name })}</li>
        </ul>
      </div>
    </header>

    {#if warning}
      <Banner tone="warning" title={t('Cubby can\'t install this pack')}>{warning}</Banner>
    {/if}

    {#if detailsError}
      {#snippet retry()}<Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>{/snippet}
      <Banner tone="warning" title={t('Couldn\'t load the pack\'s page')} actions={retry}>{detailsError}</Banner>
    {:else if !details}
      <div class="skeleton" aria-hidden="true"><span class="sk w90"></span><span class="sk w60"></span><span class="sk w90"></span><span class="sk w40"></span></div>
      <span class="cb-visually-hidden" role="status">{t('Loading the pack')}</span>
    {:else}
      <section class="description" aria-label={t('Description')}>
        {#if !details.description.trim()}
          <p class="text-body muted">{t('The author hasn\'t written a description yet.')}</p>
        {:else if details.descriptionFormat === 'markdown'}
          <Markdown source={details.description} />
        {:else}
          <div class="plain text-body selectable">
            {#each paragraphs(details.description) as para, i (i)}<p>{para}</p>{/each}
          </div>
        {/if}
      </section>

      {#if gallery.length}
        <section aria-labelledby="pk-gallery">
          <h2 id="pk-gallery" class="text-heading">{t('Gallery')}</h2>
          <ul class="gallery">
            {#each gallery as url, i (url + i)}
              <li>
                <button type="button" class="thumb" aria-label={t('Open image {i} of {length}', { i: i + 1, length: gallery.length })} onclick={() => (viewing = i)}>
                  <img src={url} alt="" loading="lazy" />
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if links.length}
        <section aria-labelledby="pk-links">
          <h2 id="pk-links" class="text-heading">{t('Links')}</h2>
          <ul class="links">
            {#each links as l (l.url + l.label)}
              <li>
                <a href={l.url} class="text-body" title={l.url} onclick={(e) => open(e, l.url)} onauxclick={(e) => open(e, l.url)}>
                  <Icon name="globe" size={18} />{l.label}
                </a>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/if}
  </div>

  <aside class="aside" aria-label={t('Install {name}', { name: summary.name })}>
    <p class="cb-ins-eyebrow">{t('Install')}</p>
    <p class="text-small muted">{t('It becomes a new instance, with its own Minecraft version, loader and settings. Your other instances stay as they are.')}</p>

    <div class="field">
      <span class="text-body-strong">{t('Version')}</span>
      {#if versions && versions.length && chosen}
        <Picker items={versionItems} value={chosen.id} onSelect={(id) => (versionId = id)} label={t('Version')} buttonLabel={t('Version {name}. Change version', { name: chosen.name })}>
          {#snippet trigger()}
            <span class="stack">
              <span class="vname"><span class="text-mono">{chosen!.name}</span>{#if chosen!.channel !== 'release'}<span class="text-small chan">{CHANNEL_WORD[chosen!.channel]}</span>{/if}</span>
              <span class="text-small muted">{versionLine(chosen!) || t('Version details unknown')}</span>
            </span>
          {/snippet}
          {#snippet option(item)}
            <span class="stack">
              <span class="vname"><span class="text-mono">{item.v.name}</span>{#if item.v.channel !== 'release'}<span class="text-small chan">{CHANNEL_WORD[item.v.channel]}</span>{:else if item.v.id === newest?.id}<span class="text-small good">{t('Latest')}</span>{/if}</span>
              <span class="text-small muted">{[versionLine(item.v), item.v.released ? relativeTime(item.v.released, '') : ''].filter(Boolean).join(' · ')}</span>
            </span>
          {/snippet}
        </Picker>
        {#if chosen.released}<span class="text-small muted">{t('Released {released} · {released2}', { released: relativeTime(chosen.released, ''), released2: date(chosen.released) })}</span>{/if}
        {#if chosen.channel !== 'release'}
          <span class="text-small warn line"><Icon name="warning" size={18} />{t('{channel} versions can have bugs.', { channel: CHANNEL_WORD[chosen.channel] })}</span>
        {/if}
      {:else if versionsError}
        <p class="text-small bad line"><Icon name="error" size={18} />{t('Couldn\'t load the versions. {versionsError}', { versionsError })}</p>
        <Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>
      {:else if versions}
        <p class="text-small muted">{t('This pack has no version to install yet.')}</p>
      {:else}
        <span class="placeholder" aria-hidden="true"></span>
        <span class="cb-visually-hidden" role="status">{t('Loading versions')}</span>
      {/if}
    </div>

    {#if chosen}
      <dl class="cb-ins-facts">
        <dt class="text-small muted">Minecraft</dt>
        <dd class="text-mono">{chosen.minecraft ?? '—'}</dd>
        <dt class="text-small muted">{t('Loader')}</dt>
        <dd class="text-mono">{chosen.loader ? loaderName(chosen.loader) : 'Vanilla'}</dd>
      </dl>
    {/if}

    {#if provider.id === 'atlauncher' && chosen}
      <section class="optional" aria-labelledby="pk-opt">
        <h3 id="pk-opt" class="cb-ins-eyebrow">{t('Optional mods')}</h3>
        {#if optionalError}
          <p class="text-small bad line"><Icon name="error" size={18} />{t('Couldn\'t load the optional mods. {optionalError}', { optionalError })}</p>
          <Button size="sm" icon="update" onclick={() => optTries++}>{t('Try again')}</Button>
        {:else if optional === null}
          <span class="placeholder tall" aria-hidden="true"></span>
          <span class="cb-visually-hidden" role="status">{t('Loading optional mods')}</span>
        {:else if !optional.length}
          <p class="text-small muted">{t('This version has no optional mods.')}</p>
        {:else}
          <p class="text-small muted">{t('{n} of {length} will be installed.', { n: plural(tickedNames.length, 'mod', 'mods'), length: optional.length })}</p>
          <ul class="mods">
            {#each optional as m (m.name)}
              <li>
                <Switch checked={!!ticked[m.name]} onChange={(on) => toggle(m, on)} label={t('Install {name}', { name: m.name })} hideLabel disabled={busy} />
                <div class="mod">
                  <p class="mod__head">
                    <span class="text-body-strong">{m.name}</span>
                    {#if m.recommended}<span class="text-small good">{t('Recommended')}</span>{/if}
                  </p>
                  {#if m.description}<p class="text-small muted">{m.description}</p>{/if}
                  {#each notes(m) as n (n)}<p class="text-small note">{n}</p>{/each}
                  {#if m.warning}<p class="text-small warn line"><Icon name="warning" size={18} />{m.warning}</p>{/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}

    <Identity bind:name bind:group bind:motif bind:ground bind:src bind:iconTouched {nameError} />

    <Button variant="primary" size="lg" icon="download" busy={busy} disabled={!canInstall} onclick={install} class="wide">{t('Install modpack')}</Button>
    {#if warning}
      <p class="text-small muted line"><Icon name="info" size={18} />{warning}</p>
    {:else}
      <p class="text-small muted">{t('You can keep using Cubby while it installs.')}</p>
    {/if}
  </aside>
</div>

{#if viewing !== null && gallery[viewing]}
  {#snippet viewerActions()}
    {#if gallery.length > 1}
      <Button size="sm" onclick={() => move(-1)}>{t('Previous')}</Button>
      <Button size="sm" iconEnd="chevron-right" onclick={() => move(1)}>{t('Next')}</Button>
    {/if}
  {/snippet}
  <Dialog wide title={t('{name}: image {viewing} of {length}', { name: summary.name, viewing: viewing + 1, length: gallery.length })} onClose={() => (viewing = null)} actions={viewerActions}>
    <img class="big" src={gallery[viewing]} alt={t('{name}, image {viewing}', { name: summary.name, viewing: viewing + 1 })} />
  </Dialog>
{/if}

<style>
  .page {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    align-items: stretch;
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
  .aside {
    box-sizing: border-box;
    flex: none;
    width: var(--size-inspector);
    /* fixed like the rail on the left: only .main scrolls, the panel keeps the height of the window */
    align-self: stretch;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
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
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .good {
    color: var(--grass-text);
  }
  .warn {
    color: var(--gold-text);
  }
  .bad {
    color: var(--redstone-text);
  }
  .line {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .link {
    padding: 0;
    font: inherit;
    color: var(--grass-text);
    background: none;
    border: 0;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .link:focus-visible,
  a:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 0;
    list-style: none;
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
  .header :global(.cb-instance-icon) {
    flex: none;
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
  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .plain {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .sk {
    display: block;
    height: 14px;
    background: var(--surface);
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
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: var(--space-3);
    padding: 0;
    list-style: none;
  }
  .thumb {
    display: block;
    width: 100%;
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
  .big {
    display: block;
    max-width: 100%;
    max-height: 60vh;
    margin: 0 auto;
    object-fit: contain;
    image-rendering: pixelated;
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

  /* install */
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .stack {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .vname {
    display: inline-flex;
    align-items: baseline;
    gap: var(--space-2);
  }
  .chan {
    color: var(--gold-text);
  }
  .placeholder {
    display: block;
    height: var(--size-control-lg);
    background: var(--surface);
    border: var(--border-block) dashed var(--line);
  }
  .placeholder.tall {
    height: 96px;
  }
  .optional {
    padding-top: var(--space-3);
    border-top: var(--border-hair) solid var(--line);
  }
  .mods {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .mods li {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .mods li:last-child {
    border-bottom: 0;
  }
  .mod {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .mod__head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--space-2);
  }
  .note {
    color: var(--diamond-text);
  }
  .aside :global(.wide) {
    width: 100%;
  }
</style>
