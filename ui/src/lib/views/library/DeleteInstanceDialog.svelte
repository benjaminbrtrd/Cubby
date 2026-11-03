<script lang="ts">
  import { Banner, Button, Dialog } from '../../cubby'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { plural } from '../../format'
  import type { Instance } from '../../types'

  let {
    id,
    ids,
    onClose,
  }: {
    id: string
    /** Several instances at once (the library's multi-select); `id` is then the first one. */
    ids?: string[]
    /** Without it, the dialog closes by clearing `app.deletingId`. */
    onClose?: (deleted: boolean) => void
  } = $props()

  const targets = $derived((ids?.length ? ids : [id]).map((x) => app.instance(x)).filter((i): i is Instance => !!i))
  const single = $derived(targets.length === 1 ? targets[0] : null)
  const running = $derived(targets.filter((i) => app.running[i.id]))

  let busy = $state(false)
  let worlds = $state<number | null>(null)

  // For one instance, say how many worlds go with it.
  $effect(() => {
    const target = single?.id
    worlds = null
    if (!target) return
    let stale = false
    api
      .listWorlds(target)
      .then((list) => {
        if (!stale) worlds = list.length
      })
      .catch(() => {})
    return () => {
      stale = true
    }
  })

  // The instance is gone already (deleted elsewhere): nothing to confirm.
  $effect(() => {
    if (!targets.length && !busy) close(false)
  })

  function close(deleted: boolean) {
    if (onClose) onClose(deleted)
    else app.deletingId = null
  }

  async function remove() {
    if (running.length) return
    busy = true
    const names: string[] = []
    for (const instance of [...targets]) {
      try {
        await api.deleteInstance(instance.id)
        names.push(instance.name)
        if ('id' in app.route && app.route.id === instance.id) app.navigate({ name: 'library' })
        if (app.selected === instance.id) app.selected = null
      } catch (e) {
        app.error(e)
      }
    }
    await app.refreshInstances()
    if (!app.selected) app.selected = app.recent[0]?.id ?? null
    busy = false
    if (names.length) {
      app.toast(names.length === 1 ? `${names[0]} moved to the trash.` : `${plural(names.length, 'instance', 'instances')} moved to the trash.`, 'success', {
        label: 'Open trash',
        run: () => api.openDataFolder('trash').catch((e) => app.error(e)),
      })
    }
    close(names.length > 0)
  }
</script>

{#snippet actions()}
  <Button variant="ghost" onclick={() => close(false)}>Cancel</Button>
  <Button variant="danger" icon="trash" busy={busy} disabled={busy || running.length > 0} onclick={remove}>
    {busy ? 'Moving to trash…' : 'Move to trash'}
  </Button>
{/snippet}

{#snippet stopAction()}
  <Button size="sm" variant="danger" icon="stop" onclick={() => running.forEach((i) => app.kill(i.id))}>Stop the game</Button>
{/snippet}

{#if targets.length}
  <Dialog
    tone="danger"
    icon="trash"
    title={single ? `Move “${single.name}” to the trash?` : `Move ${plural(targets.length, 'instance', 'instances')} to the trash?`}
    onClose={() => close(false)}
    {actions}
  >
    {#if single}
      <p>
        The instance goes to Cubby's trash folder with its mods, {worlds ? plural(worlds, 'world', 'worlds') : 'worlds'}, screenshots and settings. You can restore
        it from there.
      </p>
    {:else}
      <p>They go to Cubby's trash folder with their mods, worlds, screenshots and settings. You can restore them from there.</p>
      <ul class="names">
        {#each targets.slice(0, 6) as instance (instance.id)}
          <li class="text-body-strong">{instance.name}</li>
        {/each}
        {#if targets.length > 6}
          <li class="text-small muted">and {plural(targets.length - 6, 'other', 'others')}</li>
        {/if}
      </ul>
    {/if}
    {#if running.length}
      <Banner tone="warning" title={running.length === 1 ? `${running[0].name} is running` : 'Some of them are running'} actions={stopAction}>
        Stop the game before you move it to the trash.
      </Banner>
    {/if}
  </Dialog>
{/if}

<style>
  .names {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0 0 var(--space-3);
    padding-inline-start: var(--space-5);
  }
  .muted {
    color: var(--ink-muted);
  }
</style>
