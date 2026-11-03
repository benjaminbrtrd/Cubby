<script lang="ts">
  import { Banner, Button, Dialog, Icon, MetaChip, ProgressBar, StatusTag, Switch } from '../../cubby'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { loaderFull, plural } from '../../format'
  import type { PrismInstance } from '../../types'

  let found = $state<PrismInstance[] | null>(null)
  let chosen = $state<Record<string, boolean>>({})
  let failed = $state<string | null>(null)
  let busy = $state(false)

  $effect(() => {
    let stale = false
    api
      .listPrismInstances()
      .then((list) => {
        if (stale) return
        found = list
        chosen = Object.fromEntries(list.map((p) => [p.path, true]))
      })
      .catch((e) => {
        if (stale) return
        found = []
        failed = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
      })
    return () => {
      stale = true
    }
  })

  const picked = $derived((found ?? []).filter((p) => chosen[p.path]))
  const allOn = $derived(!!found?.length && picked.length === found.length)

  function close() {
    app.prismImportOpen = false
  }

  function openImport() {
    app.newSource = 'import'
    close()
    app.navigate({ name: 'create' })
  }

  function toggleAll() {
    const on = !allOn
    chosen = Object.fromEntries((found ?? []).map((p) => [p.path, on]))
  }

  async function run() {
    if (!picked.length) return
    busy = true
    try {
      const imported = await api.importPrism(picked.map((p) => p.path))
      await app.refreshInstances()
      if (imported[0]) app.selected = imported[0].id
      app.toast(`Imported ${plural(imported.length, 'instance', 'instances')} from Prism Launcher.`, 'success')
      close()
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }
</script>

{#snippet actions()}
  <Button variant="ghost" onclick={close}>Cancel</Button>
  <Button variant="primary" icon="import" busy={busy} disabled={busy || !picked.length} onclick={run}>
    {picked.length ? `Import ${plural(picked.length, 'instance', 'instances')}` : 'Import'}
  </Button>
{/snippet}

<Dialog wide icon="import" title="Bring your worlds" onClose={close} {actions}>
  <p class="text-body">Cubby found these on your PC. <span class="muted">Pick what to bring over.</span></p>

  {#if found === null}
    <p class="text-small muted">Looking for Prism Launcher instances…</p>
  {:else if failed}
    <Banner tone="danger" title="Couldn't read Prism Launcher's instances">{failed}</Banner>
  {:else if !found.length}
    <div class="source source--none">
      <div class="source__text">
        <p class="text-body-strong">Prism Launcher</p>
        <p class="text-small">Nothing in its usual folder</p>
        <p class="text-mono">%APPDATA%\PrismLauncher\instances</p>
      </div>
      <StatusTag state="offline">Not found</StatusTag>
    </div>
  {:else}
    <div class="source">
      <div class="source__text">
        <p class="source__title">
          <span class="text-body-strong">Prism Launcher</span>
          <span class="text-small muted">{plural(found.length, 'instance', 'instances')}</span>
        </p>
      </div>
      <Button size="sm" variant="ghost" icon={allOn ? 'minimize' : 'check'} onclick={toggleAll}>{allOn ? 'Select none' : 'Select all'}</Button>
    </div>
    <ul class="found" aria-label="Prism Launcher instances">
      {#each found as p (p.path)}
        <li>
          <Switch bind:checked={chosen[p.path]} label="Import {p.name}" hideLabel disabled={busy} />
          <div class="item">
            <p class="item__head">
              <span class="text-body-strong">{p.name}</span>
              <MetaChip>{p.minecraft}</MetaChip>
              <MetaChip>{loaderFull(p.loader)}</MetaChip>
              {#if p.group}<span class="text-small muted">{p.group}</span>{/if}
            </p>
            <p class="text-mono muted path">{p.path}</p>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="text-small muted">Safe: Cubby copies the mods, worlds, screenshots and settings. Prism Launcher stays as it is.</p>
  <p class="later text-small muted">
    <Icon name="import" size={18} />
    <span>
      Have a <span class="text-mono">.mrpack</span> or <span class="text-mono">.zip</span> file? Go to
      <button type="button" class="link" onclick={openImport}>New instance, then Import</button>.
    </span>
  </p>

  {#if busy}
    <div class="progress">
      <ProgressBar tone="diamond" label="Copying {plural(picked.length, 'instance', 'instances')}" detail="Big modpacks can take a few minutes." />
    </div>
  {/if}
</Dialog>

<style>
  .source p,
  .item p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .source {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-3) 0 var(--space-2);
  }
  .source--none {
    padding: var(--space-2) var(--space-3);
    color: var(--ink-muted);
    background: var(--bg);
    border: var(--border-block) solid var(--edge);
  }
  .source__text {
    flex: 1;
    min-width: 0;
  }
  .source__title {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px var(--space-3);
  }
  .found {
    display: flex;
    flex-direction: column;
    max-height: 320px;
    overflow: auto;
    margin: 0 0 var(--space-3);
    padding: 0;
    list-style: none;
    background: var(--bg);
    border: var(--border-block) solid var(--edge);
  }
  .found li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .found li:last-child {
    border-bottom: 0;
  }
  .item {
    flex: 1;
    min-width: 0;
  }
  .item__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }
  .path {
    overflow-wrap: anywhere;
  }
  .later {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .later {
    margin-top: var(--space-2);
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
  .progress {
    margin-top: var(--space-3);
  }
</style>
