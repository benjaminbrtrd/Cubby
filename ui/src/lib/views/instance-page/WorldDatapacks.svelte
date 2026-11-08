<!-- The datapacks of one world, in the Worlds tab's side panel: turn them on and off, add .zip files or
     find them on Modrinth. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, Dialog, IconButton, InstanceIcon, Switch, TextField } from '../../cubby'
  import { bytes, compactNum, plural } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Datapack, Instance, ProjectHit, World } from '../../types'

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

  // Finding datapacks on Modrinth
  let finding = $state(false)
  let query = $state('')
  let hits = $state<ProjectHit[] | null>(null)
  let searchError = $state<string | null>(null)
  let installing = $state<string | null>(null)
  let installed = $state<string[]>([])
  let searchTimer: ReturnType<typeof setTimeout> | undefined
  let asked = 0

  async function search() {
    const n = ++asked
    searchError = null
    try {
      const results = await api.searchDatapacks(id, query, 0)
      if (n === asked) hits = results.hits
    } catch (e) {
      if (n === asked) {
        hits = []
        searchError = String(e)
      }
    }
  }

  function openFinder() {
    finding = true
    installed = []
    if (!hits) void search()
  }

  function typed() {
    clearTimeout(searchTimer)
    searchTimer = setTimeout(search, 300)
  }

  async function install(h: ProjectHit) {
    installing = h.id
    try {
      const file = await api.installDatapack(id, world.folder, h.id)
      installed = [...installed, h.id]
      app.toast(t('{title} is in {world}.', { title: h.title, world: world.name }), 'success')
      void file
      await load()
    } catch (e) {
      app.error(e)
    } finally {
      installing = null
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
    <Button size="sm" icon="discover" disabled={running} onclick={openFinder}>{t('Find on Modrinth')}</Button>
    <Button size="sm" icon="plus" busy={adding} disabled={adding || running} onclick={addFiles}>{t('Add from file')}</Button>
  </div>
  {#if running}
    <p class="text-small muted">{t('Close the game to change datapacks. They load when the world opens.')}</p>
  {/if}
</section>

{#if finding}
  {#snippet findActions()}
    <Button variant="primary" onclick={() => (finding = false)}>{t('Done')}</Button>
  {/snippet}
  <Dialog wide icon="discover" title={t('Datapacks for {world}', { world: world.name })} onClose={() => (finding = false)} actions={findActions}>
    <div class="finder">
      <TextField label={t('Search datapacks')} hideLabel icon="search" placeholder={t('Search datapacks for {minecraft}', { minecraft: instance.minecraft })} bind:value={query} oninput={typed} />
      {#if hits === null}
        <p class="text-small muted">{t('Searching Modrinth…')}</p>
      {:else if searchError}
        <p class="text-small warn">{searchError}</p>
      {:else if !hits.length}
        <p class="text-small muted">{t('No datapacks for Minecraft {minecraft} match.', { minecraft: instance.minecraft })}</p>
      {:else}
        <ul class="hits">
          {#each hits as h (h.id)}
            <li class="hit">
              {#if h.iconUrl}
                <InstanceIcon src={h.iconUrl} size={40} />
              {:else}
                <InstanceIcon {...projectArt(h.slug)} size={40} />
              {/if}
              <div class="hit__text">
                <span class="text-body-strong clip">{h.title}</span>
                <span class="text-small muted clip" title={h.summary}>{h.summary}</span>
                <span class="text-small muted">{t('{author} · {downloads} downloads', { author: h.author, downloads: compactNum(h.downloads) })}</span>
              </div>
              {#if installed.includes(h.id)}
                <Button size="sm" variant="ghost" icon="check" disabled>{t('Added')}</Button>
              {:else}
                <Button size="sm" icon="download" busy={installing === h.id} disabled={!!installing} onclick={() => install(h)}>{t('Add')}</Button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </Dialog>
{/if}

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
  .warn {
    color: var(--danger, var(--ink));
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
  .pack,
  .hit {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .pack__text,
  .hit__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .finder {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: min(560px, 80vw);
  }
  .hits {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-height: 50vh;
    overflow-y: auto;
  }
</style>
