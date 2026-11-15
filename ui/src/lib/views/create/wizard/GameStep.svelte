<!-- Wizard › Game: the loader first, then a Minecraft version it runs on (releases, snapshots, old versions), and the
     loader's version. -->
<script lang="ts" module>
  import type { LoaderKind as Kind } from '../../../types'
  /** Minecraft versions each loader runs on, read once per session. */
  const supportedCache = new Map<Kind, Promise<string[]>>()
</script>

<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { api } from '../../../api'
  import { Banner, Button, Icon, SegmentedControl, TextField, rovingKeys } from '../../../cubby'
  import { fold, locale } from '../../../format'
  import type { GameVersion, LoaderKind, LoaderVersion } from '../../../types'
  import Picker from '../../discover/Picker.svelte'
  import { wizard as w } from './state.svelte'

  type Channel = 'release' | 'snapshot' | 'old'
  /** Snapshots and old versions shown before "Show every version". */
  const FEW = 12

  const CHANNEL_LABEL: Record<Channel, string> = { release: t('Releases'), snapshot: t('Snapshots'), old: t('Old versions') }
  const CHANNEL_HINT: Record<Channel, string> = {
    release: t('Not sure? Take the latest release. Pick 1.20.1 for the biggest choice of mods.'),
    snapshot: t('Snapshots are test builds of the next update. Back up worlds before you open them.'),
    old: t('Alpha, beta and classic versions, as they were. Most mods and servers need a newer version.'),
  }
  const LOADERS: { kind: LoaderKind; name: string; text: string; badge?: string }[] = [
    { kind: 'fabric', name: 'Fabric', text: t('Light and quick to update. Most performance and shader mods are made for it.'), badge: t('Recommended') },
    { kind: 'neoforge', name: 'NeoForge', text: t('Where most big content mods live on recent versions.') },
    { kind: 'quilt', name: 'Quilt', text: t('A cousin of Fabric. Runs most Fabric mods too.') },
    { kind: 'forge', name: 'Forge', text: t('The classic loader, best for older versions and older modpacks.') },
    { kind: 'vanilla', name: 'Vanilla', text: t('The game as Mojang makes it. No mods, only resource packs and datapacks.') },
  ]
  const monthYear = $derived(new Intl.DateTimeFormat(locale(), { month: 'short', year: 'numeric' }))
  const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))
  const kindOf = (v: GameVersion): Channel => (v.kind === 'release' ? 'release' : v.kind === 'snapshot' ? 'snapshot' : 'old')

  let versions = $state<GameVersion[] | null>(null)
  let versionsError = $state<string | null>(null)
  let channel = $state<Channel>('release')
  let query = $state('')

  function loadVersions() {
    versionsError = null
    versions = null
    api
      .listVersions()
      .then((list) => (versions = list))
      .catch((e) => (versionsError = message(e)))
  }
  loadVersions()

  // the Minecraft versions the loader runs on
  let supported = $state<Set<string> | null>(null)
  let supportedError = $state<string | null>(null)
  let supportedTry = $state(0)
  $effect(() => {
    const kind = w.loader
    void supportedTry
    supportedError = null
    if (kind === 'vanilla') {
      supported = null
      return
    }
    supported = null
    let stale = false
    let load = supportedCache.get(kind)
    if (!load) {
      load = api.loaderGameVersions(kind)
      supportedCache.set(kind, load)
      load.catch(() => supportedCache.delete(kind))
    }
    load
      .then((list) => {
        if (!stale) supported = new Set(list)
      })
      .catch((e) => {
        if (!stale) supportedError = message(e)
      })
    return () => {
      stale = true
    }
  })
  /** Still finding out which versions the loader runs on. */
  const checking = $derived(w.loader !== 'vanilla' && supported === null && !supportedError)
  /** Versions the picked loader runs on (all of them for Vanilla, or when the list couldn't be read). */
  const pool = $derived(versions && !checking ? (supported ? versions.filter((v) => supported!.has(v.id)) : versions) : null)
  const channels = $derived((['release', 'snapshot', 'old'] as Channel[]).filter((c) => pool?.some((v) => kindOf(v) === c)).map((c) => ({ value: c, label: CHANNEL_LABEL[c] })))

  // A version the loader can't run gives way to the newest one it can.
  $effect(() => {
    if (!pool?.length) return
    const picked = pool.find((v) => v.id === w.minecraft)
    const next = picked ?? pool.find((v) => v.kind === 'release') ?? pool[0]
    if (next.id !== w.minecraft) w.minecraft = next.id
    const c = kindOf(next)
    if (!picked || !channels.some((x) => x.value === channel)) channel = c
  })

  const latest = $derived(pool?.find((v) => v.kind === 'release') ?? null)
  const latestRelease = $derived(latest?.id)
  /** The loader isn't ready for the newest Minecraft yet: its own newest one is featured. */
  const behind = $derived(!!latest && !!versions && versions.find((v) => v.kind === 'release')?.id !== latest.id)
  /** Every version: a filter or "Show every version". */
  let everything = $state(false)
  const inChannel = $derived.by(() => {
    const q = fold(query.trim())
    return (pool ?? []).filter((v) => kindOf(v) === channel && (!q || fold(v.id).includes(q)))
  })
  /** "1.21.11" → "1.21", "26.1.2" → "26.1": the update a release belongs to. */
  const lineOf = (id: string) => id.split('.').slice(0, 2).join('.')
  /** By default: the newest release of each update (26.2, 1.21.11, 1.20.6…), the newest snapshots and old versions. */
  const condensed = $derived.by(() => {
    if (channel !== 'release') return inChannel.slice(0, FEW)
    const seen = new Set<string>()
    return inChannel.filter((v) => v.id !== latestRelease && !seen.has(lineOf(v.id)) && seen.add(lineOf(v.id)))
  })
  const full = $derived(everything || !!query.trim())
  const listed = $derived(full ? inChannel.filter((v) => !(channel === 'release' && !query.trim() && v.id === latestRelease)) : condensed)
  /** The picked version stays visible even when it isn't in the short list. */
  const shown = $derived(!full && w.minecraft && !listed.some((v) => v.id === w.minecraft) && inChannel.some((v) => v.id === w.minecraft && v.id !== latestRelease) ? [inChannel.find((v) => v.id === w.minecraft)!, ...listed] : listed)
  const hidden = $derived(inChannel.length - (channel === 'release' && !query.trim() && latest ? 1 : 0) - shown.length)
  /** The latest release, on its own above the others. */
  const featured = $derived(channel === 'release' && !query.trim() && !!latest)
  const shownIndex = $derived(Math.max(0, shown.findIndex((v) => v.id === w.minecraft)))

  // loader versions follow the loader and the game version
  let loaderVersions = $state<LoaderVersion[] | null>(null)
  let loaderError = $state<string | null>(null)
  let loaderTry = $state(0)
  $effect(() => {
    const kind = w.loader
    const game = w.minecraft
    void loaderTry
    loaderVersions = null
    loaderError = null
    if (kind === 'vanilla' || !game) {
      w.loaderVersion = null
      return
    }
    let stale = false
    api
      .listLoaderVersions(kind, game)
      .then((list) => {
        if (stale) return
        loaderVersions = list
        if (!list.some((l) => l.version === w.loaderVersion)) w.loaderVersion = list.find((l) => l.stable)?.version ?? list[0]?.version ?? null
      })
      .catch((e) => {
        if (stale) return
        loaderVersions = []
        w.loaderVersion = null
        loaderError = message(e)
      })
    return () => {
      stale = true
    }
  })
  const recommended = $derived(loaderVersions?.find((l) => l.stable)?.version ?? loaderVersions?.[0]?.version)
  const loaderInfo = $derived(LOADERS.find((l) => l.kind === w.loader)!)
  const loaderMissing = $derived(w.loader !== 'vanilla' && loaderVersions !== null && !w.loaderVersion)

  function loaderKeys(e: KeyboardEvent) {
    const at = LOADERS.findIndex((l) => l.kind === w.loader)
    rovingKeys(e, LOADERS.length, Math.max(0, at), (to) => (w.loader = LOADERS[to].kind))
  }
</script>

{#snippet loaderOption(item: { id: string; lv: LoaderVersion })}
  <span class="text-mono">{item.lv.version}</span>
  {#if item.lv.version === recommended}<span class="text-small muted">{t('Recommended')}</span>{:else if !item.lv.stable}<span class="text-small muted">{t('Beta')}</span>{/if}
{/snippet}

{#snippet retryVersions()}
  <Button size="sm" icon="update" onclick={loadVersions}>{t('Try again')}</Button>
{/snippet}

<section class="block" aria-labelledby="wz-loader">
  <div>
    <h2 id="wz-loader" class="text-heading">{t('Loader')}</h2>
    <p class="text-small muted">{t('What runs your mods. Each mod is made for one loader. Pick it first: the versions below are the ones it runs on.')}</p>
  </div>
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div class="loaders" role="radiogroup" aria-labelledby="wz-loader" onkeydown={loaderKeys}>
    {#each LOADERS as l (l.kind)}
      {@const on = l.kind === w.loader}
      <button type="button" role="radio" aria-checked={on} tabindex={on ? 0 : -1} class={['loader', on && 'is-on']} onclick={() => (w.loader = l.kind)}>
        <span class={['radio', on && 'on']} aria-hidden="true">{#if on}<Icon name="check" size={18} />{/if}</span>
        <span class="loader__txt">
          <span class="loader__head"><span class="text-body-strong">{l.name}</span>{#if l.badge}<span class="badge">{l.badge}</span>{/if}</span>
          <span class="text-small muted">{l.text}</span>
        </span>
      </button>
    {/each}
  </div>
</section>

<section class="block" aria-labelledby="wz-version">
  <div class="row">
    <h2 id="wz-version" class="text-heading grow">{t('Minecraft version')}</h2>
    {#if channels.length > 1}
      <SegmentedControl label={t('Version type')} options={channels} value={channel} onChange={(v) => (channel = v as Channel)} />
    {/if}
  </div>
  {#if supportedError}
    <p class="warn text-small">
      <Icon name="warning" size={18} />
      <span>{t('Couldn’t check which versions {name} runs on: every version is shown. {error}', { name: loaderInfo.name, error: supportedError })}</span>
      <Button size="sm" variant="ghost" icon="update" onclick={() => supportedTry++}>{t('Try again')}</Button>
    </p>
  {/if}
  {#if versionsError}
    <Banner tone="danger" title={t('Couldn\'t load the version list')} actions={retryVersions}>{versionsError}</Banner>
  {:else if pool === null}
    <p class="note text-small">{checking ? t('Finding the versions {name} runs on…', { name: loaderInfo.name }) : t('Loading versions…')}</p>
  {:else}
    {#if featured && latest}
      {@const on = latest.id === w.minecraft}
      <button type="button" class={['hero', on && 'is-on']} aria-pressed={on} onclick={() => (w.minecraft = latest.id)}>
        <span class={['radio', on && 'on']} aria-hidden="true">{#if on}<Icon name="check" size={18} />{/if}</span>
        <span class="hero__txt">
          <span class="hero__head">
            <span class="text-label muted">{behind ? t('Newest for {name}', { name: loaderInfo.name }) : t('Latest release')}</span>
            <span class="badge">{t('Recommended')}</span>
          </span>
          <span class="hero__ver text-mono">{latest.id}</span>
          <span class="text-small muted">
            {behind
              ? t('{name} isn’t ready for the newest Minecraft yet. This is the newest version it runs on.', { name: loaderInfo.name })
              : t('Released {date}. The newest game, with the newest blocks and mobs.', { date: monthYear.format(new Date(latest.releaseTime)) })}
          </span>
        </span>
      </button>
    {/if}
    <div class="row">
      <h3 class="text-label muted grow">{featured ? t('Other versions') : t('Versions')}</h3>
      <div class="filter"><TextField label={t('Filter versions')} hideLabel size="sm" icon="search" placeholder={t('Filter versions')} bind:value={query} /></div>
    </div>
    {#if !shown.length && !featured}
      <p class="note text-small">{query.trim() ? t('No version matches “{versionQuery}”.', { versionQuery: query.trim() }) : t('{name} runs on no version of this kind.', { name: loaderInfo.name })}</p>
    {:else if shown.length}
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div class="grid" role="radiogroup" aria-labelledby="wz-version" onkeydown={(e) => rovingKeys(e, shown.length, shownIndex, (to) => (w.minecraft = shown[to].id))}>
        {#each shown as v, i (v.id)}
          {@const on = v.id === w.minecraft}
          <button type="button" role="radio" aria-checked={on} tabindex={i === shownIndex ? 0 : -1} class={['tile', on && 'is-on']} onclick={() => (w.minecraft = v.id)}>
            <span class="text-mono">{v.id}</span>
            <span class="tile__sub">{v.id === latestRelease ? t('Latest') : monthYear.format(new Date(v.releaseTime))}</span>
          </button>
        {/each}
      </div>
    {/if}
    {#if !query.trim() && (hidden > 0 || everything)}
      <div><Button size="sm" variant="ghost" icon={everything ? 'minimize' : 'chevron-down'} onclick={() => (everything = !everything)}>{everything ? t('Show fewer versions') : t('Show every version ({n})', { n: hidden })}</Button></div>
    {/if}
    <p class="text-small muted">{CHANNEL_HINT[channel]}</p>
  {/if}

  {#if w.loader !== 'vanilla' && pool}
    <div class="row lv">
      <span class="text-small muted">{t('{name} version', { name: loaderInfo.name })}</span>
      <Picker
        items={loaderVersions?.map((lv) => ({ id: lv.version, lv })) ?? []}
        value={w.loaderVersion}
        onSelect={(v) => (w.loaderVersion = v)}
        label={t('{name} version', { name: loaderInfo.name })}
        buttonLabel={t('Loader version')}
        variant="block"
        disabled={!loaderVersions?.length}
        listWidth={240}
        option={loaderOption}
      >
        {#snippet trigger()}
          {#if loaderVersions === null}
            {t('Loading…')}
          {:else if w.loaderVersion}
            <span class="text-mono">{w.loaderVersion}</span>{w.loaderVersion === recommended ? t(' (recommended)') : ''}
          {:else}
            {t('None available')}
          {/if}
        {/snippet}
      </Picker>
      <span class="text-small muted">{t('The recommended one is right for almost everyone.')}</span>
    </div>
  {/if}
  {#if loaderMissing}
    <p class="warn text-small">
      <Icon name="warning" size={18} />
      {#if loaderError}
        <span>{t('Couldn\'t load {name} versions: {loaderError}', { name: loaderInfo.name, loaderError })}</span>
        <Button size="sm" variant="ghost" icon="update" onclick={() => loaderTry++}>{t('Try again')}</Button>
      {:else}
        <span>{t('{name} has no version for Minecraft', { name: loaderInfo.name })} <span class="text-mono">{w.minecraft}</span>{t('. Pick another version or loader.')}</span>
      {/if}
    </p>
  {/if}
</section>

<style>
  h2,
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }
  .note {
    padding: var(--space-3);
    color: var(--ink-muted);
    background: var(--field);
    border: var(--border-block) solid var(--outline);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(116px, 1fr));
    gap: 10px;
    padding: 4px;
  }
  h3 {
    margin: 0;
  }
  .filter {
    flex: 0 1 260px;
  }
  /* the loader's version: its own line under the versions, apart from their hint */
  .lv {
    margin-top: var(--space-2);
    padding-top: var(--space-4);
    column-gap: var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .hero {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: var(--space-3);
    align-items: start;
    max-width: 560px;
    padding: var(--space-4) var(--space-5);
    font: inherit;
    color: var(--ink);
    text-align: start;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
    cursor: pointer;
  }
  .hero:hover {
    background: var(--surface-hover);
  }
  .hero:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring), var(--shadow-block);
  }
  .hero.is-on {
    outline: var(--border-block) solid var(--ink);
    outline-offset: 3px;
  }
  .hero__txt {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }
  .hero__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .hero__ver {
    font-size: 28px;
    line-height: 34px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-height: 56px;
    padding: 8px 12px;
    font: inherit;
    color: var(--ink);
    text-align: start;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .tile:hover {
    background: var(--surface-hover);
  }
  .tile:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: -3px;
  }
  .tile.is-on {
    background: var(--surface-hover);
    outline: var(--border-block) solid var(--ink);
    outline-offset: -6px;
  }
  .tile__sub {
    font-size: 12px;
    color: var(--ink-muted);
  }
  .loaders {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 10px;
  }
  .loader {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: 4px var(--space-3);
    align-items: start;
    padding: 12px 14px;
    font: inherit;
    color: var(--ink);
    text-align: start;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .loader:hover:not(:disabled) {
    background: var(--surface-hover);
  }
  .loader:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: -3px;
  }
  .loader.is-on {
    background: var(--surface-hover);
    outline: var(--border-block) solid var(--ink);
    outline-offset: -6px;
  }
  .loader:disabled {
    color: var(--ink-disabled);
    border-style: dashed;
    cursor: default;
  }
  .radio {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    margin-top: 1px;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
  }
  .radio.on {
    color: var(--on-grass);
    background: var(--grass);
    border-color: var(--edge);
  }
  .loader__txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .loader__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .badge {
    padding: 0 6px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--grass-text);
    border: 1px solid currentColor;
  }
  .warn {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    color: var(--gold-text);
  }
</style>
