<!--
  First-launch assistant, over the app (below the title bar) while settings.onboarded is false.
  Page 1: sign in with Microsoft (device code), the only way to play.
  Page 2: bring Prism Launcher instances over, and make a first instance. Escape does not close it; Skip does.
-->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { onMount, tick } from 'svelte'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { loaderFull, loaderName, plural } from '../../format'
  import type { Instance, NewInstance, PrismInstance } from '../../types'
  import { addressProblem, discoverPerformanceMods, findNewest, hostOf, presetSpec, type Preset } from '../create/presets'
  import {
    Banner,
    Button,
    Icon,
    InstanceIcon,
    Logo,
    MetaChip,
    ProgressBar,
    StatusTag,
    Switch,
    TextField,
    type Ground,
    type Motif } from '../../cubby'
  import { MicrosoftLogin, deviceCode, skinHead } from '../accounts/Accounts.svelte'

  const uid = $props.id()
  let page = $state<1 | 2>(1)
  let root: HTMLDivElement | undefined = $state()

  async function toPage(p: 1 | 2) {
    page = p
    await tick()
    root?.querySelector<HTMLElement>('[data-heading]')?.focus()
  }

  /* Page 1: sign in */
  const login = new MicrosoftLogin()
  const msaReady = $derived(!!app.info?.msaConfigured)
  const account = $derived(app.defaultAccount)

  $effect(() => () => login.cancel())

  async function signIn() {
    const signed = await login.start()
    if (signed) app.toast(t('Signed in as {username}.', { username: signed.username }), 'success')
  }

  function next() {
    if (!account) return
    login.cancel()
    void toPage(2)
  }

  function skipSignIn() {
    login.cancel()
    void toPage(2)
  }

  const s = $derived(app.settings)

  const STEPS = [t('Sign in'), t('Bring your worlds'), t('First instance')]
  function stepState(i: number): 'now' | 'done' | 'skipped' | 'later' {
    if (page === 1) return i === 0 ? 'now' : 'later'
    if (i === 0) return account ? 'done' : 'skipped'
    return 'now'
  }
  const STEP_TAG = { now: t('Now'), done: t('Done'), skipped: t('Skipped'), later: '' } as const

  /* Page 2: worlds and a first instance */
  let prism = $state<PrismInstance[] | null>(null)
  let picked = $state<Record<string, boolean>>({})
  const toImport = $derived(prism?.filter((p) => picked[p.path]) ?? [])

  async function loadPrism() {
    try {
      const found = await api.listPrismInstances()
      picked = Object.fromEntries(found.map((p) => [p.path, true]))
      prism = found
    } catch (e) {
      prism = []
      app.error(e)
    }
  }

  let release = $state<string | null>(null)
  let fabric = $state<string | null>(null)
  let versions = $state<'loading' | 'ready' | 'failed'>('loading')
  let fabricState = $state<'loading' | 'ready' | 'none' | 'failed'>('loading')

  async function loadVersions() {
    versions = 'loading'
    fabricState = 'loading'
    const newest = await findNewest()
    release = newest.release
    fabric = newest.fabric
    versions = release ? 'ready' : 'failed'
    fabricState = !release || newest.fabricFailed ? 'failed' : fabric ? 'ready' : 'none'
  }

  onMount(() => {
    void loadPrism()
    void loadVersions()
    void tick().then(() => root?.querySelector<HTMLElement>('[data-heading]')?.focus())
  })

  type Choice = 'latest' | 'fast' | 'server' | 'none'
  const CHOICES: { id: Choice; title: string; art?: { motif: Motif; ground: Ground } }[] = [
    { id: 'latest', title: t('Latest release'), art: { motif: 'block', ground: 'moss' } },
    { id: 'fast', title: t('Smooth and fast'), art: { motif: 'rocket', ground: 'sky' } },
    { id: 'server', title: t('Join a server'), art: { motif: 'castle', ground: 'amethyst' } },
    { id: 'none', title: t('Not now') },
  ]
  let choice = $state<Choice>('latest')

  function available(c: Choice): boolean {
    if (c === 'none') return true
    if (versions !== 'ready') return false
    return c !== 'fast' || fabricState === 'ready'
  }
  /** What Finish does: the choice, or no instance when its version can't be found. */
  const effective = $derived<Choice>(available(choice) ? choice : versions === 'loading' ? choice : 'none')
  /** Finish waits until the newest release is known when it has to make an instance. */
  const waitingForVersion = $derived(effective !== 'none' && versions === 'loading')

  function describe(c: Choice): string {
    const v = release ?? 'the latest release'
    switch (c) {
      case 'latest':
        return versions === 'loading' ? t('Looking for the latest version…') : t('The game as Mojang made it, nothing added.')
      case 'fast':
        if (fabricState === 'loading') return t('Looking for the newest Fabric loader…')
        if (fabricState === 'none') return t('Fabric isn’t ready for {v} yet.', { v })
        if (fabricState === 'failed') return t('Cubby couldn’t reach Fabric’s servers.')
        return t('The Fabric loader, ready for performance mods like Sodium. After setup, Cubby shows them in Discover.')
      case 'server':
        return t('Paste an address. Cubby makes a vanilla instance on {v} and adds the server to it.', { v })
      default:
        return t('Keep what you imported, or start with an empty library.')
    }
  }

  async function choose(c: Choice) {
    choice = c
    if (c === 'server') {
      await tick()
      document.getElementById(`${uid}-address`)?.focus()
    }
  }

  let address = $state('')
  let addressLeft = $state(false)
  const addressError = $derived(addressLeft ? addressProblem(address.trim()) : '')

  let playAfter = $state(false)

  /* Finish */
  let finishing = $state(false)
  let progress = $state<{ label: string; done?: number; total?: number } | null>(null)
  // What already happened, so Finish can be pressed again after an error without doing it twice
  const imported = new Set<string>()
  let created: Instance | null = null
  let serverAdded = false

  function spec(c: Preset, addr: string): NewInstance {
    if (!release) throw new Error(t('Cubby couldn’t find the latest version. Check your connection and try again.'))
    return presetSpec(c, release, fabric, addr)
  }

  async function finish() {
    if (finishing) return
    const c = effective
    const addr = address.trim()
    if (c === 'server' && addressProblem(addr)) {
      addressLeft = true
      await tick()
      document.getElementById(`${uid}-address`)?.focus()
      return
    }
    finishing = true
    try {
      const paths = toImport.map((p) => p.path).filter((p) => !imported.has(p))
      for (let i = 0; i < paths.length; i++) {
        progress = { label: t('Importing from Prism Launcher · {i} of {length}', { i: i + 1, length: paths.length }), done: i, total: paths.length }
        await api.importPrism([paths[i]])
        imported.add(paths[i])
      }
      if (c !== 'none' && !created) {
        progress = { label: t('Creating your first instance') }
        created = await api.createInstance(spec(c, addr))
      }
      if (c === 'server' && created && !serverAdded) {
        await api.saveServers(created.id, [{ name: hostOf(addr), address: addr, iconUrl: null, acceptTextures: null }])
        serverAdded = true
      }
      await app.refreshInstances()
      close(c, addr)
    } catch (e) {
      app.error(e)
    } finally {
      finishing = false
      progress = null
    }
  }

  const openDiscover = discoverPerformanceMods

  function close(c: Choice, addr: string) {
    const inst = created
    app.paletteOpen = false
    if (inst) app.selected = inst.id
    if (inst && playAfter && !app.demoOnly) {
      app.openInstance(inst.id)
      void app.launch(inst.id, false, c === 'server' ? addr : null)
      if (c === 'fast') app.toast(t('Add performance mods like Sodium from Discover.'), 'info', { label: t('Open Discover'), run: () => openDiscover(inst.id) })
    } else if (inst && c === 'fast') {
      openDiscover(inst.id)
    } else {
      app.navigate({ name: 'library' })
    }
    void app.saveSettings({ onboarded: true })
  }

  function skipSetup() {
    login.cancel()
    app.paletteOpen = false
    app.navigate({ name: 'library' })
    void app.saveSettings({ onboarded: true })
  }

  const summary = $derived(
    `${toImport.length ? t('{n} to import', { n: plural(toImport.length, 'instance', 'instances') }) : t('Nothing to import')} · ${
      effective === 'none' ? t('no new instance') : t('first instance: {title}', { title: CHOICES.find((c) => c.id === effective)?.title ?? '' })
    }`,
  )

  /* Focus stays in the assistant (and the title bar, for the window buttons) */
  const FOCUSABLE = 'a[href], button, input:not([type="hidden"]), select, textarea, [tabindex]'
  function tabbables(): HTMLElement[] {
    if (!root) return []
    return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
      (n) => n.tabIndex >= 0 && !n.matches(':disabled') && n.getClientRects().length > 0,
    )
  }

  function trap(e: KeyboardEvent) {
    if (e.key !== 'Tab') return
    const items = tabbables()
    if (!items.length) return
    const first = items[0]
    const last = items[items.length - 1]
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first.focus()
    }
  }

  $effect(() => {
    const back = (e: FocusEvent) => {
      const t = e.target
      if (!root || !(t instanceof Element) || root.contains(t) || t.closest('.cb-titlebar, [aria-live], .cb-dialog-layer')) return
      tabbables()[0]?.focus()
    }
    document.addEventListener('focusin', back)
    return () => document.removeEventListener('focusin', back)
  })
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="ob" role="dialog" aria-modal="true" aria-labelledby="{uid}-title" tabindex="-1" bind:this={root} onkeydown={trap}>
  <h1 id="{uid}-title" class="cb-visually-hidden">{t('Welcome to Cubby')}</h1>

  <div class="ob__body">
    <div class="ob__inner">
      <nav class="steps" aria-label={t('Setup steps')}>
        <ol class="steps__list">
          {#each STEPS as label, i (label)}
            {@const st = stepState(i)}
            <li class={['step', 'step--' + st]} aria-current={st === 'now' ? 'step' : undefined}>
              {#if i > 0}<span class="step__line" aria-hidden="true"></span>{/if}
              <span class="step__box text-heading" aria-hidden="true">
                {#if st === 'done'}<Icon name="check" size={18} />{:else}{i + 1}{/if}
              </span>
              <span class={st === 'now' ? 'text-body-strong' : 'text-body'}><span class="cb-visually-hidden">{t('Step {i}:', { i: i + 1 })} </span>{label}</span>
              {#if st !== 'later'}<span class="step__tag text-label">{STEP_TAG[st]}</span>{/if}
            </li>
          {/each}
        </ol>
      </nav>

      {#if page === 1}
        <div class="welcome">
          <section class="intro" aria-label={t('About Cubby')}>
            <Logo size={128} />
            <p class="tagline text-heading">{t('The lightweight Minecraft launcher.')}</p>
            <ul class="features">
              <li class="feature">
                <span class="feature__icon"><Icon name="cube" size={18} /></span>
                <span class="feature__text">
                  <span class="text-body-strong">{t('Every version in one click')}</span>
                  <span class="text-small muted">{t('Releases, snapshots and old versions. Cubby gets the game and Java for you.')}</span>
                </span>
              </li>
              <li class="feature">
                <span class="feature__icon"><Icon name="mods" size={18} /></span>
                <span class="feature__text">
                  <span class="text-body-strong">{t('Mods that fit your game')}</span>
                  <span class="text-small muted">{t('Cubby checks each mod against your game version and loader, and adds what it needs.')}</span>
                </span>
              </li>
              <li class="feature">
                <span class="feature__icon"><Icon name="search" size={18} /></span>
                <span class="feature__text">
                  <span class="text-body-strong">{t('Find anything from the top bar')}</span>
                  <span class="text-small muted">{t('Search your instances, servers and settings, and launch from there.')}</span>
                </span>
              </li>
            </ul>
          </section>

          <section class="card signin" aria-labelledby="{uid}-signin">
            <div>
              <h2 id="{uid}-signin" class="text-title" tabindex="-1" data-heading>{t('Sign in')}</h2>
              <p class="text-small muted">{t('You need a Microsoft account that owns Minecraft: Java Edition.')}</p>
            </div>

            {#if account}
              <div class="signed">
                {@render skinHead(account.skinUrl, account.username, 48, '')}
                <div class="signed__text">
                  <p class="text-heading name">{account.username}</p>
                  <p class="kind text-body">
                    <Icon name="globe" size={18} />
                    {t('Microsoft account')}
                  </p>
                </div>
                <StatusTag state="ready">{t('Ready to play')}</StatusTag>
              </div>
              <p class="text-small muted">{t('You can add more accounts later in Accounts.')}</p>
            {:else}
              {#if msaReady}
                {#if login.phase === 'idle'}
                  <div class="wide"><Button variant="primary" size="lg" icon="accounts" onclick={signIn}>{t('Sign in with Microsoft')}</Button></div>
                {:else}
                  {@render deviceCode(login, signIn)}
                  {#if login.busy}<div><Button variant="ghost" onclick={() => login.cancel()}>{t('Cancel sign-in')}</Button></div>{/if}
                {/if}
                <ul class="facts">
                  <li class="text-small muted"><Icon name="globe" size={18} /><span>{t('Sign-in happens on Microsoft’s page, in your browser.')}</span></li>
                  <li class="text-small muted"><Icon name="key" size={18} /><span>{t('Cubby never sees your password.')}</span></li>
                </ul>
              {:else}
                <Banner tone="warning" title={t('Microsoft sign-in isn’t set up in this copy of Cubby')}>
                  {t('Without it, Cubby only starts the Minecraft demo. Official builds include it. You can also add an application id in Settings › Accounts.')}
                </Banner>
              {/if}
            {/if}
          </section>
        </div>
      {:else}
        <div class="cards">
          <section class="card import" aria-labelledby="{uid}-import">
            <div>
              <div class="card__title">
                <span class="text-label muted">{t('Step 2')}</span>
                <h2 id="{uid}-import" class="text-title" tabindex="-1" data-heading>{t('Bring your worlds')}</h2>
              </div>
              <p class="text-body">
                {#if prism?.length}{t('Cubby found these Prism Launcher instances on your PC.')} <span class="muted">{t('Pick what to bring over.')}</span>
                {:else}{t('Cubby can bring over instances from Prism Launcher, with their worlds and mods.')}{/if}
              </p>
            </div>

            {#if prism === null}
              <ProgressBar label={t('Looking for Prism Launcher')} />
            {:else if prism.length === 0}
              <ul class="sources">
                <li class="source is-missing">
                  <div class="source__text">
                    <p class="source__name"><span class="text-body-strong">Prism Launcher</span><span class="text-small">{t('Nothing in its usual folder')}</span></p>
                    <p class="text-mono path">%APPDATA%\PrismLauncher</p>
                  </div>
                  <StatusTag state="offline">{t('Not found')}</StatusTag>
                </li>
              </ul>
            {:else}
              <ul class="sources" aria-label={t('Prism Launcher instances')}>
                {#each prism as p (p.path)}
                  {@const unsupported = p.loader.kind === 'forge' || p.loader.kind === 'neoforge'}
                  <li class="source">
                    <Switch label={t('Import {name}', { name: p.name })} hideLabel checked={!!picked[p.path]} disabled={finishing} onChange={(v) => (picked[p.path] = v)} />
                    <div class="source__text">
                      <p class="source__name">
                        <span class="text-body-strong">{p.name}</span>
                        <MetaChip>{p.minecraft}</MetaChip>
                        <MetaChip>{loaderFull(p.loader)}</MetaChip>
                        {#if p.group}<span class="text-small muted">{p.group}</span>{/if}
                      </p>
                      <p class="text-mono muted path">{p.path}</p>
                      {#if unsupported}
                        <p class="warn text-small">
                          <Icon name="warning" size={18} />
                          <span>{t('{loader} isn’t supported yet: its worlds come over, but Cubby can’t start this instance.', { loader: loaderName(p.loader) })}</span>
                        </p>
                      {/if}
                    </div>
                  </li>
                {/each}
              </ul>
            {/if}
            <p class="text-small muted">{t('Cubby copies what you pick: Prism Launcher keeps its own. Import from the official launcher is coming later.')}</p>
          </section>

          <section class="card first" aria-labelledby="{uid}-first">
            <div>
              <div class="card__title">
                <span class="text-label muted">{t('Step 3')}</span>
                <h2 id="{uid}-first" class="text-title">{t('Your first instance')}</h2>
              </div>
              <p class="text-small muted">{t('An instance is a copy of the game with its own version, mods and worlds.')}</p>
            </div>

            {#if versions === 'failed'}
              {#snippet retryVersions()}<Button size="sm" icon="update" onclick={loadVersions}>{t('Try again')}</Button>{/snippet}
              <Banner tone="warning" title={t('Can’t find the latest version')} actions={retryVersions}>
                {t('Cubby couldn’t reach Mojang’s servers. Check your connection, or make an instance later.')}
              </Banner>
            {/if}

            <div class="choices">
              {#each CHOICES as c (c.id)}
                {@const on = effective === c.id}
                {@const ok = available(c.id)}
                <div class={['choice', on && 'is-on']}>
                  {#if c.art}
                    <InstanceIcon motif={c.art.motif} ground={c.art.ground} size={32} raised />
                  {:else}
                    <span class="choice__blank" aria-hidden="true"><Icon name="library" size={18} /></span>
                  {/if}
                  <div class="choice__main">
                    <div class="choice__head">
                      <h3 class="text-body-strong grow">{c.title}</h3>
                      {#if on}
                        <span class="picked text-body-strong"><Icon name="check" size={18} />{t('Selected')}</span>
                      {:else}
                        <Button size="sm" aria-label={t('Choose {title}', { title: c.title })} disabled={!ok || finishing} onclick={() => choose(c.id)}>{t('Choose')}</Button>
                      {/if}
                    </div>
                    {#if (c.id === 'latest' || (c.id === 'fast' && fabric)) && release}
                      <div class="chips">
                        <MetaChip>{release}</MetaChip>
                        <MetaChip>{c.id === 'fast' ? t('Fabric {fabric}', { fabric: fabric ?? '' }) : 'Vanilla'}</MetaChip>
                      </div>
                    {/if}
                    <p class="text-small muted">{describe(c.id)}</p>
                    {#if c.id === 'server' && on}
                      <TextField
                        id="{uid}-address"
                        label={t('Server address')}
                        placeholder="play.example.net"
                        icon="servers"
                        mono
                        spellcheck={false}
                        autocomplete="off"
                        error={addressError}
                        bind:value={address}
                        onblur={() => (addressLeft = address.trim().length > 0)}
                      />
                    {/if}
                  </div>
                </div>
              {/each}
            </div>

            <Switch
              label={t('Play when setup is done')}
              hint={account || !app.demoOnly ? t('Cubby downloads the game, then starts it.') : t('Without a Microsoft account, Cubby starts the Minecraft demo.')}
              checked={playAfter}
              disabled={effective === 'none' || finishing}
              onChange={(v) => (playAfter = v)}
            />
            <p class="plus text-small muted"><Icon name="plus" size={18} />{t('You can add more instances any time.')}</p>
          </section>
        </div>
      {/if}
    </div>
  </div>

  <footer class="ob__foot">
    {#if page === 1}
      <div class="foot__field">
        <span class="cb-label text-body-strong">{t('Language')}</span>
        <span class="lang text-body">English</span>
      </div>
      <span class="grow"></span>
      <div class="foot__actions">
        <Button variant="ghost" onclick={skipSignIn}>{t('Skip for now')}</Button>
        <Button variant="primary" iconEnd="chevron-right" disabled={!account} onclick={next}>{t('Continue')}</Button>
      </div>
    {:else}
      <div class="foot__actions">
        <Button variant="ghost" disabled={finishing} onclick={() => toPage(1)}>{t('Back')}</Button>
        <Button variant="ghost" disabled={finishing} onclick={skipSetup}>{t('Skip setup')}</Button>
      </div>
      <span class="grow"></span>
      {#if progress}
        <div class="foot__progress">
          <ProgressBar
            tone="diamond"
            label={progress.label}
            value={progress.total ? ((progress.done ?? 0) / progress.total) * 100 : undefined}
          />
        </div>
      {:else}
        <p class="text-small muted">{summary}</p>
      {/if}
      <Button variant="primary" icon="check" busy={finishing} disabled={finishing || waitingForVersion} onclick={finish}>{t('Finish setup')}</Button>
    {/if}
  </footer>
</div>

<style>
  .ob {
    position: fixed;
    inset: var(--size-titlebar) 0 0 0;
    z-index: calc(var(--layer-dialog) - 1);
    display: flex;
    flex-direction: column;
    color: var(--ink);
    background: var(--bg);
    outline: none;
  }
  .ob :where(h2, h3, p, ul, ol) {
    margin: 0;
  }
  [data-heading]:focus {
    outline: none;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
  }
  .ob__body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .ob__inner {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    max-width: 1136px;
    min-height: 100%;
    margin: 0 auto;
    padding: var(--space-5) var(--space-8);
  }

  /* Steps */
  .steps {
    display: flex;
    justify-content: center;
  }
  .steps__list {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: 0;
    list-style: none;
  }
  .step {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--ink-muted);
  }
  .step--now {
    color: var(--ink);
  }
  .step__line {
    display: block;
    width: 40px;
    height: 2px;
    background: var(--line);
  }
  .step__box {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: var(--border-block) dashed var(--outline);
  }
  .step--now .step__box {
    color: var(--ink);
    background: var(--surface-hover);
    border: var(--border-block) solid var(--ink);
    box-shadow: var(--shadow-block-sm);
  }
  .step--done .step__box {
    color: var(--grass-text);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .step__tag {
    padding: 4px 6px;
    border: var(--border-block) solid var(--line);
  }
  .step--now .step__tag {
    color: var(--ink);
    border-color: var(--ink);
  }
  .step--skipped .step__tag {
    border-style: dashed;
    border-color: var(--outline);
  }

  /* Cards */
  .card {
    box-sizing: border-box;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: var(--space-6);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .card__title {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-3);
  }
  .card h2 {
    outline: none;
  }

  /* Page 1 */
  .welcome {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: var(--space-6) var(--space-12);
  }
  .intro {
    flex: 1 1 380px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-5);
  }
  .tagline {
    max-width: 440px;
  }
  .features {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: 0;
    list-style: none;
  }
  .feature {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .feature__icon {
    box-sizing: border-box;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    color: var(--grass-text);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .feature__text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .feature__kbd {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .signin {
    flex: 1 1 460px;
    max-width: 540px;
  }
  .wide {
    display: flex;
    flex-direction: column;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0;
    list-style: none;
  }
  .facts li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .signed {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: 14px var(--space-4);
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .signed__text {
    flex: 1;
    min-width: 0;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .kind {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  /* Page 2 */
  .cards {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: stretch;
    gap: var(--space-6);
  }
  .import {
    flex: 7 1 520px;
    gap: var(--space-3);
    padding: 18px 20px;
  }
  .first {
    flex: 5 1 380px;
    gap: var(--space-3);
    padding: 18px 20px;
  }
  .sources {
    padding: 0;
    list-style: none;
    background: var(--bg);
    border: var(--border-block) solid var(--edge);
  }
  .source {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: var(--space-2) 14px;
    border-top: var(--border-hair) solid var(--line);
  }
  .source:first-child {
    border-top: 0;
  }
  .source > :global(.cb-switch) {
    margin-top: 2px;
  }
  .source.is-missing {
    align-items: center;
    color: var(--ink-muted);
  }
  .source__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .source__name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }
  .path {
    overflow-wrap: anywhere;
  }
  .warn {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--gold-text);
  }
  .warn :global(.cb-icon) {
    margin-top: 1px;
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) 14px;
    background: var(--bg);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  /* The chosen one: an ink frame and the word Selected */
  .choice.is-on {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .choice__blank {
    box-sizing: border-box;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    color: var(--ink-muted);
    border: var(--border-block) dashed var(--outline);
  }
  .choice__main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .choice__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
  }
  .picked {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--grass-text);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px var(--space-2);
  }
  .plus {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  /* Footer */
  .ob__foot {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--space-4) var(--space-6);
    padding: 14px var(--space-8);
    background: var(--bg-chrome);
    border-top: var(--border-chrome) solid var(--edge);
  }
  .foot__field {
    display: flex;
    flex-direction: column;
  }
  .lang {
    display: flex;
    align-items: center;
    min-height: var(--size-control);
  }
  .foot__switch {
    max-width: 270px;
    padding-bottom: 2px;
  }
  .foot__actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .foot__progress {
    width: min(360px, 100%);
  }
</style>
