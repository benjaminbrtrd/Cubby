<!-- "Into world": picks the world of the target instance that datapacks go into (app.targetWorld). -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { relativeTime } from '../../format'
  import { app } from '../../state.svelte'
  import type { World } from '../../types'
  import Picker from './Picker.svelte'

  let { variant = 'field' }: { variant?: 'field' | 'block' } = $props()

  const target = $derived(app.target)
  const targetId = $derived(target?.id)
  let worlds = $state<World[] | null>(null)

  $effect(() => {
    const id = targetId
    worlds = null
    if (!id) return
    let stale = false
    api
      .listWorlds(id)
      .then((list) => {
        if (stale) return
        worlds = [...list].sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? ''))
        // Keep the chosen world when the instance has it, else the one played last.
        if (!worlds.some((w) => w.folder === app.targetWorld)) app.targetWorld = worlds[0]?.folder ?? null
      })
      .catch(() => {
        if (!stale) worlds = []
      })
    return () => {
      stale = true
    }
  })

  const items = $derived((worlds ?? []).map((w) => ({ id: w.folder, world: w })))
  const chosen = $derived(worlds?.find((w) => w.folder === app.targetWorld) ?? null)
</script>

{#if worlds && !worlds.length}
  <p class="none text-small">{t('{name} has no world yet. Play it once and create one.', { name: target?.name ?? '' })}</p>
{:else if worlds && chosen}
  <Picker {items} value={chosen.folder} onSelect={(id) => (app.targetWorld = id)} label={t('Into world')} buttonLabel={t('Into {world}. Change world', { world: chosen.name })} {variant}>
    {#snippet trigger()}
      {#if chosen.iconUrl}<img class="thumb" src={chosen.iconUrl} alt="" />{/if}
      <span class="stack">
        <span class="text-body-strong name" title={chosen.name}>{chosen.name}</span>
        <span class="text-small muted">{t('World')}</span>
      </span>
    {/snippet}
    {#snippet option(item)}
      {#if item.world.iconUrl}<img class="thumb" src={item.world.iconUrl} alt="" />{/if}
      <span class="stack">
        <span class="text-body-strong name" title={item.world.name}>{item.world.name}</span>
        <span class="text-small muted">{item.world.lastPlayed ? relativeTime(item.world.lastPlayed) : t('Not played yet')}</span>
      </span>
    {/snippet}
  </Picker>
{/if}

<style>
  .stack {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted,
  .none {
    color: var(--ink-muted);
  }
  .none {
    margin: 0;
  }
  .thumb {
    flex: none;
    width: 32px;
    height: 32px;
    image-rendering: pixelated;
    border: var(--border-hair) solid var(--edge);
  }
</style>
