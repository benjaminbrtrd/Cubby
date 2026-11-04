<script lang="ts">
  import { onMount } from 'svelte'
  import { app, sectionOf } from './lib/state.svelte'
  import { windowAction } from './lib/api'
  import { Button, Hotbar, TitleBar, Tooltips, type HotbarItem } from './lib/cubby'
  import Toasts from './lib/views/shell/Toasts.svelte'
  import Palette from './lib/views/shell/Palette.svelte'
  import UpdateBanner from './lib/views/shell/UpdateBanner.svelte'
  import UpdateDialogs from './lib/views/shell/UpdateDialogs.svelte'
  import Library from './lib/views/library/Library.svelte'
  import DeleteInstanceDialog from './lib/views/library/DeleteInstanceDialog.svelte'
  import ImportDialog from './lib/views/library/ImportDialog.svelte'
  import NewInstance from './lib/views/create/NewInstance.svelte'
  import InstancePage from './lib/views/instance-page/InstancePage.svelte'
  import Running from './lib/views/play/Running.svelte'
  import Crash from './lib/views/play/Crash.svelte'
  import Downloads from './lib/views/downloads/Downloads.svelte'
  import Discover from './lib/views/discover/Discover.svelte'
  import ProjectPage from './lib/views/discover/ProjectPage.svelte'
  import Servers from './lib/views/servers/Servers.svelte'
  import Screenshots from './lib/views/captures/Screenshots.svelte'
  import Accounts, { skinHead } from './lib/views/accounts/Accounts.svelte'
  import Settings from './lib/views/settings/Settings.svelte'
  import Onboarding from './lib/views/onboarding/Onboarding.svelte'

  onMount(() => {
    const ready = app.init()
    // "Match Windows": follow the system theme while the app is open
    const media = [window.matchMedia('(prefers-color-scheme: light)'), window.matchMedia('(prefers-contrast: more)')]
    const follow = () => app.applyAppearance()
    media.forEach((m) => m.addEventListener('change', follow))
    return () => {
      media.forEach((m) => m.removeEventListener('change', follow))
      void ready.then((unlisten) => unlisten())
    }
  })

  $effect(() => {
    void app.settings?.theme
    void app.settings?.hyperlegible
    void app.settings?.reduceMotion
    app.applyAppearance()
  })

  const activeTasks = $derived(Object.keys(app.tasks).length)

  const nav = $derived<HotbarItem[]>([
    { id: 'library', label: 'Library', icon: 'library' },
    { id: 'discover', label: 'Discover', icon: 'discover' },
    { id: 'servers', label: 'Servers', icon: 'servers' },
    { id: 'captures', label: 'Screenshots', icon: 'captures' },
    { id: 'downloads', label: 'Downloads', icon: 'download', badge: activeTasks || undefined },
  ])
  const navFoot: HotbarItem[] = [
    { id: 'accounts', label: 'Accounts', icon: 'accounts' },
    { id: 'settings', label: 'Settings', icon: 'settings' },
  ]
  const SLOTS = ['library', 'discover', 'servers', 'captures', 'downloads', 'accounts', 'settings']

  function go(id: string) {
    if (id === 'settings') app.navigate({ name: 'settings', section: 'general' })
    else if (id === 'library' || id === 'discover' || id === 'servers' || id === 'captures' || id === 'downloads' || id === 'accounts') app.navigate({ name: id })
  }

  const account = $derived(
    app.defaultAccount
      ? { name: app.defaultAccount.username, label: `Account: ${app.defaultAccount.username}`, art: accountHead }
      : undefined,
  )


  const onboarding = $derived(app.loaded && !!app.settings && !app.settings.onboarded)

  /* The update chip in the title bar */
  const upd = $derived(app.update)
  // Only once the strip under the title bar was closed: it says the same thing while it is there.
  const chipShown = $derived(!!upd.info && upd.dismissed && ['available', 'downloading', 'ready', 'failed'].includes(upd.phase))
  const bannerShown = $derived(!onboarding && !!upd.info && !upd.dismissed && ['available', 'downloading', 'ready', 'failed'].includes(upd.phase))
  const waitingForGame = $derived(upd.phase === 'ready' && Object.keys(app.running).length > 0)
  const updPercent = $derived.by(() => {
    const t = app.tasks['self-update']
    return t && t.total ? Math.round((t.done / t.total) * 100) : null
  })


  /** No browser menu (Back, Reload, Print…) in a desktop app; text fields keep theirs for cut, copy and paste. */
  function oncontextmenu(e: MouseEvent) {
    const t = e.target
    const editable = t instanceof HTMLElement && (t.isContentEditable || !!t.closest('input, textarea'))
    if (!editable) e.preventDefault()
  }

  function onkeydown(e: KeyboardEvent) {
    if (onboarding) return
    const ctrl = e.ctrlKey || e.metaKey
    const key = e.key.toLowerCase()
    if (ctrl && !e.shiftKey && !e.altKey && key === 'k') {
      e.preventDefault()
      app.paletteOpen = !app.paletteOpen
    } else if (ctrl && !e.altKey && key === 'n') {
      e.preventDefault()
      app.paletteOpen = false
      app.navigate({ name: 'create' })
    } else if (ctrl && e.key === ',') {
      e.preventDefault()
      app.paletteOpen = false
      app.navigate({ name: 'settings', section: 'general' })
    } else if (ctrl && !e.shiftKey && !e.altKey && /^[1-7]$/.test(e.key)) {
      e.preventDefault()
      app.paletteOpen = false
      go(SLOTS[Number(e.key) - 1])
    } else if (e.altKey && !ctrl && e.key === 'ArrowLeft') {
      e.preventDefault()
      app.back()
    }
  }
</script>

<svelte:window {onkeydown} {oncontextmenu} />

<!-- The skin texture cropped to the head, like the Accounts screen draws it -->
{#snippet accountHead()}
  {@const a = app.defaultAccount}
  {#if a}{@render skinHead(a.skinUrl, a.username, 24, '')}{/if}
{/snippet}

<div class="window">
  <TitleBar
    {account}
    children={chipShown ? updateChip : undefined}
    onAccount={() => app.navigate({ name: 'accounts' })}
    onSearch={() => (app.paletteOpen = true)}
    onMinimize={() => windowAction('minimize')}
    onMaximize={() => windowAction('maximize')}
    onClose={() => windowAction('close')}
  />
  <div class="body">
    <Hotbar items={nav} footer={navFoot} value={sectionOf(app.route)} onChange={go} />
    <div class="stage">
    {#if bannerShown}<div class="update"><UpdateBanner /></div>{/if}
    <main id="content" tabindex="-1">
      {#if !app.loaded}
        <p class="loading text-small">Loading your instances…</p>
      {:else if app.route.name === 'library'}
        <Library />
      {:else if app.route.name === 'instance'}
        {#key app.route.id}
          <InstancePage id={app.route.id} tab={app.route.tab} />
        {/key}
      {:else if app.route.name === 'running'}
        {#key app.route.id}<Running id={app.route.id} />{/key}
      {:else if app.route.name === 'crash'}
        {#key app.route.id}<Crash id={app.route.id} />{/key}
      {:else if app.route.name === 'create'}
        <NewInstance />
      {:else if app.route.name === 'discover'}
        <Discover />
      {:else if app.route.name === 'project'}
        {#key app.route.idOrSlug}
          <ProjectPage idOrSlug={app.route.idOrSlug} kind={app.route.kind} />
        {/key}
      {:else if app.route.name === 'downloads'}
        <Downloads />
      {:else if app.route.name === 'servers'}
        <Servers />
      {:else if app.route.name === 'captures'}
        <Screenshots />
      {:else if app.route.name === 'accounts'}
        <Accounts />
      {:else if app.route.name === 'settings'}
        <Settings section={app.route.section} />
      {/if}
    </main>
    </div>
  </div>
</div>

{#snippet updateChip()}
  {#if upd.phase === 'available'}
    <Button size="sm" icon="download" onclick={() => (app.updateDialog = 'notes')}>Update available</Button>
  {:else if upd.phase === 'downloading'}
    <Button size="sm" icon="download" disabled>Updating{updPercent != null ? ` ${updPercent}%` : ''}</Button>
  {:else if waitingForGame}
    <Button size="sm" icon="clock" onclick={() => (app.updateDialog = 'playing')}>Update after play</Button>
  {:else if upd.phase === 'ready'}
    <Button size="sm" variant="primary" icon="update" onclick={() => (app.updateDialog = 'restart')}>Restart to update</Button>
  {:else if upd.phase === 'failed'}
    <Button size="sm" icon="warning" onclick={() => app.downloadUpdate()}>Update failed</Button>
  {/if}
{/snippet}

{#if app.paletteOpen && !onboarding}<Palette onClose={() => (app.paletteOpen = false)} />{/if}
{#if app.deletingId}<DeleteInstanceDialog id={app.deletingId} />{/if}
{#if app.prismImportOpen}<ImportDialog />{/if}
{#if onboarding}<Onboarding />{:else}<UpdateDialogs />{/if}
<Toasts />
<Tooltips />

<style>
  .window {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-chrome);
    overflow: clip;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: clip;
  }
  .stage {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .update {
    flex: none;
  }
  main {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow: auto;
    background: var(--bg);
    outline: none;
  }
  .loading {
    margin: 0;
    padding: var(--space-6);
    color: var(--ink-muted);
  }
</style>
