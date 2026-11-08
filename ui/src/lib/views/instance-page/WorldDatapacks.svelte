<!-- The datapacks of one world, in the Worlds tab's side panel: turn them on and off, add .zip files, or
     open Discover › Datapacks set to install into this world. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, IconButton, Switch } from '../../cubby'
  import { bytes, plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { Datapack, Instance, World } from '../../types'

  let { instance, world }: { instance: Instance; world: World } = $props()

  const id = $derived(instance.id)
  const running = $derived(!!app.running[id])

  let packs = $state<Datapack[] | null>(null)
  let adding = $state(false)

  async function load() {
    const w = world.folder
    try {
      const list = await api.listDatapacks(id, w)
      if (w === world.folder) packs = list
    } catch (e) {
      packs = []
      app.error(e)
    }
  }

  $effect(() => {
    void world.folder
    packs = null
    void load()
  })

  async function toggle(p: Datapack, on: boolean) {
    packs = (packs ?? []).map((x) => (x.fileName === p.fileName ? { ...x, enabled: on } : x))
    try {
      await api.setDatapackEnabled(id, world.folder, p.fileName, on)
    } catch (e) {
      app.error(e)
    }
    await load()
  }

  async function remove(p: Datapack) {
    try {
      await api.removeDatapack(id, world.folder, p.fileName)
      app.toast(t('{name} moved to the instance’s trash folder.', { name: p.name }), 'success')
    } catch (e) {
      app.error(e)
    }
    await load()
  }

  async function addFiles() {
    adding = true
    try {
      const added = await api.addDatapackFiles(id, world.folder)
      if (added.length) app.toast(t('Added {n} to {world}.', { n: plural(added.length, 'datapack', 'datapacks'), world: world.name }), 'success')
      await load()
    } catch (e) {
      app.error(e)
    } finally {
      adding = false
    }
  }
</script>

<section class="part" aria-labelledby="w-datapacks">
  <h3 id="w-datapacks" class="text-label muted">{t('Datapacks · {length}', { length: packs?.length ?? 0 })}</h3>
  {#if packs === null}
    <p class="text-small muted">{t('Reading datapacks…')}</p>
  {:else if !packs.length}
    <p class="text-small muted">{t('No datapacks in this world. They add structures, biomes or rules to one world, without mods.')}</p>
  {:else}
    <ul class="packs">
      {#each packs as p (p.fileName)}
        <li class="pack">
          <div class="pack__text">
            <span class="text-body clip" title={p.fileName}>{p.name}</span>
            <span class="text-small muted clip" title={p.description ?? ''}>{p.description ?? (p.isDir ? t('Folder') : bytes(p.size))}</span>
          </div>
          <Switch checked={p.enabled} label={t('Load {name} in this world', { name: p.name })} hideLabel disabled={running} onChange={(on) => toggle(p, on)} />
          <IconButton icon="trash" size="sm" variant="ghost" label={t('Remove {name}', { name: p.name })} disabled={running} onclick={() => remove(p)} />
        </li>
      {/each}
    </ul>
  {/if}
  <div class="actions">
    <Button size="sm" icon="discover" disabled={running} onclick={() => app.openDatapacks(id, world.folder)}>{t('Find on Modrinth')}</Button>
    <Button size="sm" icon="plus" busy={adding} disabled={adding || running} onclick={addFiles}>{t('Add from file')}</Button>
  </div>
  {#if running}
    <p class="text-small muted">{t('Close the game to change datapacks. They load when the world opens.')}</p>
  {/if}
</section>

<style>
  p,
  h3,
  ul {
    margin: 0;
  }
  ul {
    padding: 0;
    list-style: none;
  }
  .muted {
    color: var(--ink-muted);
  }
  .clip {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .part {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .packs {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .pack {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .pack__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  /* Full width, so the two buttons wrap inside the panel instead of sticking out of it */
  .actions {
    align-self: stretch;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
