<!-- Files of a modpack that its authors don't let launchers download: the player gets each one from its page,
     then Cubby looks for it in the Downloads folder and puts it in place. -->
<script lang="ts">
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon } from '../../cubby'
  import { plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { BlockedFile, Instance } from '../../types'

  interface Props {
    instance: Instance
    onClose: () => void
  }

  let { instance, onClose }: Props = $props()

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  let files = $state<BlockedFile[] | null>(null)
  let error = $state<string | null>(null)
  let checking = $state(false)
  let checked = $state(false)
  let tries = $state(0)

  $effect(() => {
    const id = instance.id
    void tries
    let stale = false
    error = null
    api
      .blockedFiles(id)
      .then((list) => {
        if (!stale) files = list
      })
      .catch((e) => {
        if (!stale) error = message(e)
      })
    return () => {
      stale = true
    }
  })

  const missing = $derived((files ?? []).filter((f) => !f.found))
  const allIn = $derived(!!files?.length && !missing.length)

  async function check() {
    if (checking) return
    checking = true
    try {
      files = await api.findBlockedFiles(instance.id)
      checked = true
      await app.refreshInstances()
    } catch (e) {
      app.error(e)
    } finally {
      checking = false
    }
  }

  function openPage(f: BlockedFile) {
    void api.openUrl(f.pageUrl).catch((e) => app.error(e))
  }

  const fileName = (f: BlockedFile) => f.fileName || f.path.split('/').pop() || f.path
</script>

{#snippet actions()}
  <Button variant="ghost" onclick={onClose}>Close</Button>
  <Button variant="primary" icon="folder" busy={checking} disabled={checking || !files?.length || allIn} onclick={check}>Check my Downloads folder</Button>
{/snippet}

<Dialog wide icon="download" title="Files to download by hand" {onClose} {actions}>
  <p class="text-body intro">
    The authors of {instance.modpack?.title ?? instance.name} don't let launchers download these files. Open each page, download the file, and save it in your Downloads folder.
    <span class="muted">Then Cubby moves it into place. You can play meanwhile: the pack runs without them, but it may be missing some content.</span>
  </p>

  {#if error}
    {#snippet retry()}<Button size="sm" icon="update" onclick={() => tries++}>Try again</Button>{/snippet}
    <Banner tone="danger" title="Couldn't load the list" actions={retry}>{error}</Banner>
  {:else if files === null}
    <p class="text-small muted" role="status">Loading the list…</p>
  {:else if !files.length}
    <Banner tone="success" title="Nothing to download">Every file of this pack is in place.</Banner>
  {:else}
    {#if allIn}
      <Banner tone="success" title="All files are in place">Nothing more to do for {instance.name}.</Banner>
    {:else if checked}
      <Banner tone="info" title="{plural(missing.length, 'file is', 'files are')} still missing">
        Cubby didn't find {missing.length === 1 ? 'it' : 'them'} in your Downloads folder yet. Save {missing.length === 1 ? 'it' : 'them'} there, then check again.
      </Banner>
    {/if}
    <ul class="files" aria-label="Files to download by hand">
      {#each files as f (f.path)}
        <li class={[f.found && 'is-found']}>
          <div class="file">
            <p class="text-mono name">{fileName(f)}</p>
            <p class="text-small muted">Goes in <span class="text-mono">{f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : 'the game folder'}</span></p>
          </div>
          {#if f.found}
            <span class="text-small found"><Icon name="check" size={18} />In place</span>
          {:else}
            <span class="text-small miss">Missing</span>
            <Button size="sm" icon="globe" onclick={() => openPage(f)} aria-label="Open the page of {fileName(f)}">Open page</Button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</Dialog>

<style>
  p {
    margin: 0;
  }
  .intro {
    margin-bottom: var(--space-3);
  }
  .muted {
    color: var(--ink-muted);
  }
  .files {
    display: flex;
    flex-direction: column;
    max-height: 320px;
    overflow: auto;
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
    background: var(--bg);
    border: var(--border-block) solid var(--edge);
  }
  .files li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .files li:last-child {
    border-bottom: 0;
  }
  .file {
    flex: 1;
    min-width: 0;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .found {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--grass-text);
  }
  .miss {
    color: var(--gold-text);
  }
  .is-found .name {
    color: var(--ink-muted);
  }
  :global(.cb-dialog__body) > .files:first-child,
  :global(.cb-dialog__body) > :global(.cb-banner) + .files {
    margin-top: var(--space-3);
  }
</style>
