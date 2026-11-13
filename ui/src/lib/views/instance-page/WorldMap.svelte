<!-- A top-down map of a world around its spawn, drawn from the region files on demand. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, Dialog, InspectorSection, ProgressBar } from '../../cubby'
  import { num } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, World, WorldMapImage } from '../../types'

  let { instance, world }: { instance: Instance; world: World } = $props()

  /** Maps drawn while Cubby runs, by instance and world. */
  let maps = $state<Record<string, WorldMapImage>>({})
  let failed = $state<Record<string, string>>({})
  let big = $state(false)

  const key = $derived(`${instance.id}/${world.folder}`)
  const map = $derived(maps[key])
  const task = $derived(app.tasks[`map:${instance.id}`])

  async function draw() {
    const k = key
    delete failed[k]
    try {
      maps[k] = await api.renderWorldMap(instance.id, world.folder)
    } catch (e) {
      failed[k] = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
    }
  }
</script>

{#snippet drawAction()}
  <Button size="sm" variant="ghost" icon={map ? 'update' : 'globe'} busy={!!task} disabled={!!task} onclick={draw}>{map ? t('Draw again') : t('Draw the map')}</Button>
{/snippet}
<InspectorSection title={t('Map')} action={drawAction}>
  {#if map}
    <button type="button" class="map" onclick={() => (big = true)} aria-label={t('Show the map of {name} bigger', { name: world.name })}>
      <img src={map.url} alt={t('Map of {name} around its spawn', { name: world.name })} />
      <span class="spawn" aria-hidden="true"></span>
    </button>
    <p class="cb-ins-hint">{t('{n} blocks around the spawn · {chunks} chunks explored', { n: num(map.size / 2), chunks: num(map.chunks) })}</p>
  {:else if task}
    <ProgressBar tone="grass" label={t('Drawing the map')} value={task.total ? (task.done / task.total) * 100 : undefined} />
  {:else if failed[key]}
    <p class="cb-ins-hint">{failed[key]}</p>
  {:else}
    <p class="cb-ins-hint">{t('See the land around the spawn from above.')}</p>
  {/if}
</InspectorSection>

{#if big && map}
  <Dialog wide icon="globe" title={world.name} onClose={() => (big = false)}>
    <div class="map map--big">
      <img src={map.url} alt={t('Map of {name} around its spawn', { name: world.name })} />
      <span class="spawn" aria-hidden="true"></span>
    </div>
    <p class="text-small muted">{t('From {x}, {z} to {x2}, {z2}. The red dot is the spawn.', { x: num(map.minX), z: num(map.minZ), x2: num(map.minX + map.size), z2: num(map.minZ + map.size) })}</p>
  </Dialog>
{/if}

<style>
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .map {
    position: relative;
    display: block;
    width: 100%;
    padding: 0;
    aspect-ratio: 1;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    cursor: zoom-in;
  }
  .map:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .map img {
    display: block;
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
  }
  .map--big {
    /* the whole map stays inside the dialog, whatever the window's size */
    width: min(100%, calc(100vh - 260px));
    margin-inline: auto;
    cursor: default;
  }
  .spawn {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 8px;
    height: 8px;
    margin: -4px 0 0 -4px;
    background: #e5383b;
    border: 2px solid #fff;
    border-radius: 50%;
  }
</style>
