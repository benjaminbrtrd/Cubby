<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { Banner, Button, Icon, IconPicker, SegmentedControl, TextField, pickIcon, rovingKeys, type Ground, type Motif } from '../../cubby'
  import { api } from '../../api'
  import Picker from '../discover/Picker.svelte'
  import { app } from '../../state.svelte'
  import { encodeIcon } from '../../icon'
  import { locale, fold, loaderName } from '../../format'
  import type { GameVersion, LoaderKind, LoaderVersion } from '../../types'

  type Channel = 'release' | 'snapshot' | 'old'

  const CHANNELS = [
    { value: 'release', label: t('Releases') },
    { value: 'snapshot', label: t('Snapshots') },
    { value: 'old', label: t('Old versions') },
  ]
  const CHANNEL_HINT: Record<Channel, string> = {
    release: t('Not sure? Take the latest release. Pick 1.20.1 for the biggest choice of mods.'),
    snapshot: t('Snapshots are test builds of the next update. Back up worlds before you open them.'),
    old: t('Alpha, beta and classic versions, as they were. Most mods and servers need a newer version.'),
  }
  const LOADERS: { kind: LoaderKind; name: string; text: string; ready: boolean }[] = [
    { kind: 'vanilla', name: t('None'), text: t('The game as Mojang made it'), ready: true },
    { kind: 'fabric', name: 'Fabric', text: t('Light and quick to update, home of most performance mods'), ready: true },
    { kind: 'quilt', name: 'Quilt', text: t('Runs most Fabric mods, with extra tools for mod makers'), ready: true },
    { kind: 'neoforge', name: 'NeoForge', text: t('Big content mods like Create and Mekanism'), ready: true },
    { kind: 'forge', name: 'Forge', text: t('The classic loader, best for older versions'), ready: true },
  ]
  const READY = LOADERS.filter((l) => l.ready)
  const monthYear = $derived(new Intl.DateTimeFormat(locale(), { month: 'short', year: 'numeric' }))
  const listId = $props.id()

  // form state

  let name = $state('')
  let nameTouched = $state(false)
  let group = $state('')
  let motif = $state<Motif | undefined>()
  let ground = $state<Ground | undefined>()
  let src = $state<string | null>(null)
  let iconTouched = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()

  let versions = $state<GameVersion[] | null>(null)
  let versionsError = $state<string | null>(null)
  let channel = $state<Channel>('release')
  let versionQuery = $state('')
  let minecraft = $state<string | null>(null)

  let loader = $state<LoaderKind>('vanilla')
  let loaderVersions = $state<LoaderVersion[] | null>(null)
  let loaderError = $state<string | null>(null)
  let loaderVersion = $state<string | null>(null)
  let loaderTry = $state(0)

  let busy = $state<'create' | 'play' | null>(null)
  let submitted = $state(false)

  const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e))

  // Minecraft versions

  function loadVersions() {
    versionsError = null
    versions = null
    api
      .listVersions()
      .then((list) => {
        versions = list
        minecraft ??= list.find((v) => v.kind === 'release')?.id ?? list[0]?.id ?? null
      })
      .catch((e) => (versionsError = message(e)))
  }
  loadVersions()

  const latestRelease = $derived(versions?.find((v) => v.kind === 'release')?.id)
  const latestSnapshot = $derived(versions?.find((v) => v.kind === 'snapshot')?.id)
  const shown = $derived.by(() => {
    const q = fold(versionQuery.trim())
    return (versions ?? []).filter(
      (v) => (channel === 'old' ? v.kind === 'old_beta' || v.kind === 'old_alpha' : v.kind === channel) && (!q || fold(v.id).includes(q)),
    )
  })
  const shownIndex = $derived(Math.max(0, shown.findIndex((v) => v.id === minecraft)))

  function tagOf(v: GameVersion): string {
    if (v.id === latestRelease) return t('Latest release')
    if (v.id === latestSnapshot) return t('Newest')
    return ''
  }

  // loader versions follow the loader and the game version

  $effect(() => {
    const kind = loader
    const game = minecraft
    void loaderTry
    loaderVersions = null
    loaderVersion = null
    loaderError = null
    if (kind === 'vanilla' || !game) return
    let stale = false
    api
      .listLoaderVersions(kind, game)
      .then((list) => {
        if (stale) return
        loaderVersions = list
        loaderVersion = list.find((l) => l.stable)?.version ?? list[0]?.version ?? null
      })
      .catch((e) => {
        if (stale) return
        loaderVersions = []
        loaderError = message(e)
      })
    return () => {
      stale = true
    }
  })

  const recommended = $derived(loaderVersions?.find((l) => l.stable)?.version ?? loaderVersions?.[0]?.version)
  const loaderInfo = $derived(LOADERS.find((l) => l.kind === loader)!)

  // name and icon follow the choices until the player changes them

  const autoName = $derived(minecraft ? `${loader === 'vanilla' ? 'Minecraft' : loaderName(loader)} ${minecraft}` : '')
  $effect(() => {
    if (!nameTouched) name = autoName
  })
  $effect(() => {
    if (iconTouched) return
    const pair = pickIcon(name.trim() || 'Minecraft')
    motif = pair.motif
    ground = pair.ground
  })

  function importImage() {
    fileInput?.click()
  }

  function readImage(e: Event & { currentTarget: HTMLInputElement }) {
    const file = e.currentTarget.files?.[0]
    e.currentTarget.value = ''
    if (!file) return
    if (!/^image\/(png|jpeg)$/.test(file.type)) return app.toast(t('Pick a PNG or JPG image.'), 'error')
    if (file.size > 512_000) return app.toast(t('Pick an image under 512 KB.'), 'error')
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        src = reader.result
        iconTouched = true
      }
    }
    reader.onerror = () => app.toast(t('Couldn\'t read this image.'), 'error')
    reader.readAsDataURL(file)
  }

  // create

  const nameError = $derived(submitted && !name.trim() ? t('Give the instance a name.') : undefined)
  const loaderMissing = $derived(loader !== 'vanilla' && loaderVersions !== null && !loaderVersion)
  const ready = $derived(!!minecraft && (loader === 'vanilla' || !!loaderVersion))
  const canCreate = $derived(ready && !!name.trim() && !busy)

  async function create(andPlay: boolean) {
    submitted = true
    if (!canCreate || !minecraft) return
    busy = andPlay ? 'play' : 'create'
    try {
      const instance = await api.createInstance({
        name: name.trim(),
        group: group.trim() || null,
        icon: src ?? (motif && ground ? encodeIcon(motif, ground) : null),
        minecraft,
        loader: loader === 'vanilla' || !loaderVersion ? { kind: 'vanilla' } : { kind: loader, version: loaderVersion },
      })
      await app.refreshInstances()
      app.selected = instance.id
      app.openInstance(instance.id)
      if (andPlay) void app.launch(instance.id)
      else app.toast(t('{name} is ready. Game files are downloaded the first time you play.', { name: instance.name }), 'success')
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  function onWindowKey(e: KeyboardEvent) {
    if (e.defaultPrevented || app.paletteOpen || app.deletingId || app.prismImportOpen) return
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
      e.preventDefault()
      void create(true)
    }
  }

  // loader cards: one tab stop, the arrows move between the loaders Cubby can run

  function loaderKeys(e: KeyboardEvent) {
    const at = READY.findIndex((l) => l.kind === loader)
    rovingKeys(e, READY.length, Math.max(0, at), (to) => (loader = READY[to].kind))
  }
</script>

{#snippet loaderOption(item: { id: string; lv: LoaderVersion }, selected: boolean)}
  <span class="text-mono">{item.lv.version}</span>
  {#if item.lv.version === recommended}<span class="text-small muted">{t('Recommended')}</span>{:else if !item.lv.stable}<span class="text-small muted">{t('Beta')}</span>{/if}
{/snippet}

<svelte:window onkeydown={onWindowKey} />

<div class="screen">
  <div class="split">
    <div class="content">
      <h1 class="text-title">{t('Custom instance')}</h1>

      <div class="top">
        <section class="identity" aria-label={t('Name and icon')}>
          <div class="fields">
            <div class="name">
              <TextField label={t('Name')} bind:value={name} error={nameError} oninput={() => (nameTouched = true)} />
            </div>
            <div class="group">
              <TextField label={t('Group')} icon="folder" placeholder={t('No group')} list={listId} bind:value={group} />
              <datalist id={listId}>
                {#each app.groups as g (g.name)}<option value={g.name}></option>{/each}
              </datalist>
            </div>
          </div>
          <IconPicker
            bind:motif
            bind:ground
            bind:src
            name={name.trim() || autoName}
            onChange={() => (iconTouched = true)}
            onImport={importImage}
          />
          <input bind:this={fileInput} class="cb-visually-hidden" type="file" accept="image/png,image/jpeg" tabindex="-1" aria-hidden="true" onchange={readImage} />
        </section>

        <section class="versions" aria-labelledby="ni-version">
          <h2 id="ni-version" class="text-heading">{t('Minecraft version')}</h2>
          <SegmentedControl label={t('Version type')} options={CHANNELS} value={channel} onChange={(v) => (channel = v as Channel)} />
          <TextField label={t('Filter versions')} hideLabel size="sm" icon="search" placeholder={t('Filter versions')} bind:value={versionQuery} />
          {#if versionsError}
            <Banner tone="danger" title={t('Couldn\'t load the version list')} actions={retryVersions}>{versionsError}</Banner>
          {:else if versions === null}
            <p class="well-note text-small">{t('Loading versions…')}</p>
          {:else if !shown.length}
            <p class="well-note text-small">{t('No version matches “{versionQuery}”.', { versionQuery: versionQuery.trim() })}</p>
          {:else}
            <!-- svelte-ignore a11y_interactive_supports_focus -->
            <div class="vlist" role="radiogroup" aria-labelledby="ni-version" onkeydown={(e) => rovingKeys(e, shown.length, shownIndex, (to) => (minecraft = shown[to].id))}>
              {#each shown as v, i (v.id)}
                {@const on = v.id === minecraft}
                <button type="button" role="radio" aria-checked={on} tabindex={i === shownIndex ? 0 : -1} class={['ver', on && 'is-on']} onclick={() => (minecraft = v.id)}>
                  <span class="text-mono">{v.id}</span>
                  {#if tagOf(v)}<span class="tag text-small">{tagOf(v)}</span>{/if}
                  <span class="grow"></span>
                  <span class="text-small muted">{monthYear.format(new Date(v.releaseTime))}</span>
                </button>
              {/each}
            </div>
          {/if}
          <p class="text-small muted">{CHANNEL_HINT[channel]}</p>
        </section>
      </div>

      <section class="loaders" aria-labelledby="ni-loader">
        <div class="loaders__head">
          <h2 id="ni-loader" class="text-heading">{t('Mod loader')}</h2>
          <span class="text-small muted">{t('A loader lets the game run mods. You can change it later.')}</span>
          <span class="grow"></span>
          {#if loader !== 'vanilla'}
            <span class="text-small muted" id="ni-loader-version">{t('Loader version')}</span>
            <Picker
              items={loaderVersions?.map((lv) => ({ id: lv.version, lv })) ?? []}
              value={loaderVersion}
              onSelect={(v) => (loaderVersion = v)}
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
                {:else if loaderVersion}
                  <span class="text-mono">{loaderVersion}</span>{loaderVersion === recommended ? t(' (recommended)') : ''}
                {:else}
                  {t('None available')}
                {/if}
              {/snippet}
            </Picker>
          {/if}
        </div>
        <!-- svelte-ignore a11y_interactive_supports_focus -->
        <div class="cards" role="radiogroup" aria-labelledby="ni-loader" onkeydown={loaderKeys}>
          {#each LOADERS as l (l.kind)}
            {@const on = l.kind === loader}
            <button
              type="button"
              role="radio"
              aria-checked={on}
              tabindex={on ? 0 : -1}
              disabled={!l.ready}
              class={['card', on && 'is-on']}
              onclick={() => (loader = l.kind)}
            >
              <span class="card__head">
                <span class="text-body-strong">{l.name}</span>
                {#if on}<span class="card__check"><Icon name="check" size={18} label={t('Selected')} /></span>{/if}
              </span>
              <span class="text-small muted">{l.text}</span>
            </button>
          {/each}
        </div>
        {#if loaderMissing}
          <p class="warn text-small">
            <Icon name="warning" size={18} />
            {#if loaderError}
              <span>{t('Couldn\'t load {name} versions: {loaderError}', { name: loaderInfo.name, loaderError })}</span>
              <Button size="sm" variant="ghost" icon="update" onclick={() => loaderTry++}>{t('Try again')}</Button>
            {:else}
              <span>{t('{name} has no version for Minecraft', { name: loaderInfo.name })} <span class="text-mono">{minecraft}</span>{t('. Pick another version or loader.')}</span>
            {/if}
          </p>
        {/if}
      </section>
    </div>

    <aside class="aside" aria-labelledby="ni-download">
      <h2 id="ni-download" class="cb-ins-eyebrow">{t('What Cubby will download')}</h2>
      <ul class="cb-ins-rows">
        <li class="cb-ins-row">
          <span class="cb-ins-row__ic"><Icon name="cube" size={18} /></span>
          <div class="cb-ins-row__txt">
            <div class="cb-ins-row__t">Minecraft <span class="text-mono">{minecraft ?? '…'}</span></div>
            <div class="cb-ins-row__s">{t('Game and libraries, from Mojang')}</div>
          </div>
        </li>
        <li class="cb-ins-row">
          <span class="cb-ins-row__ic"><Icon name="folder" size={18} /></span>
          <div class="cb-ins-row__txt">
            <div class="cb-ins-row__t">{t('Game assets')}</div>
            <div class="cb-ins-row__s">{t('Sounds and textures, shared by your instances')}</div>
          </div>
        </li>
        {#if loader !== 'vanilla'}
          <li class="cb-ins-row">
            <span class="cb-ins-row__ic"><Icon name="mods" size={18} /></span>
            <div class="cb-ins-row__txt">
              <div class="cb-ins-row__t">{t('{name} loader', { name: loaderInfo.name })} <span class="text-mono">{loaderVersion ?? '…'}</span></div>
              <div class="cb-ins-row__s">{t('Runs your mods')}</div>
            </div>
          </li>
        {/if}
        <li class="cb-ins-row">
          <span class="cb-ins-row__ic"><Icon name="check" size={18} /></span>
          <div class="cb-ins-row__txt">
            <div class="cb-ins-row__t">Java</div>
            <div class="cb-ins-row__s">
              {app.settings?.java.auto === false ? t('Uses the Java from your settings') : t('Cubby picks the version this game needs')}
            </div>
          </div>
        </li>
      </ul>
      <p class="cb-ins-hint">{t('Game files are downloaded the first time you play.')}</p>

      <div class="cb-ins-acts">
        <Button variant="primary" icon="play" busy={busy === 'play'} disabled={!!busy || !ready} onclick={() => create(true)}>
          {t('Create and play')}
        </Button>
        <Button icon="plus" busy={busy === 'create'} disabled={!!busy || !ready} onclick={() => create(false)}>{t('Create')}</Button>
      </div>
      <p class="cb-ins-hint">{t('You can change all of this later.')}</p>
    </aside>
  </div>
</div>

{#snippet retryVersions()}
  <Button size="sm" icon="update" onclick={loadVersions}>{t('Try again')}</Button>
{/snippet}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    height: 100%;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  h1,
  h2 {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .muted {
    color: var(--ink-muted);
  }
  .top {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-4);
  }
  .identity {
    flex: 1 1 520px;
    /* wide enough for five emblems per row (5 × 60px and four gaps, next to the preview) */
    max-width: 590px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .name {
    flex: 1 1 220px;
  }
  .group {
    flex: 0 1 200px;
    min-width: 160px;
  }
  .versions {
    flex: 1 1 260px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .versions p {
    margin: 0;
  }
  .vlist {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 272px;
    overflow: auto;
    padding: 2px;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .ver {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 var(--space-2);
    min-height: 28px;
    padding: 0 var(--space-2);
    color: var(--ink);
    text-align: start;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .ver:hover {
    background: var(--surface-hover);
  }
  .ver.is-on {
    background: var(--surface-hover);
    outline: var(--border-block) solid var(--ink);
    outline-offset: calc(-1 * var(--border-block));
  }
  .ver:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: -3px;
  }
  .tag {
    color: var(--diamond-text);
  }
  .well-note {
    padding: var(--space-3);
    color: var(--ink-muted);
    background: var(--field);
    border: var(--border-block) solid var(--outline);
  }

  .loaders {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .loaders__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-1);
    padding: 10px;
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
  .card:hover:not(:disabled) {
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block);
  }
  .card.is-on {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .card:focus-visible {
    box-shadow: var(--focus-ring), var(--shadow-block-sm);
    outline: 2px solid transparent;
  }
  .card:disabled {
    color: var(--ink-disabled);
    border-style: dashed;
    box-shadow: none;
    cursor: default;
  }
  .card:disabled .muted {
    color: var(--ink-disabled);
  }
  .card__head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .card__head > :first-child {
    flex: 1;
  }
  .card__check {
    display: inline-flex;
    color: var(--grass-text);
  }
  .warn {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    color: var(--gold-text);
  }
  .aside {
    box-sizing: border-box;
    flex: 0 0 var(--size-inspector);
    width: var(--size-inspector);
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
</style>
