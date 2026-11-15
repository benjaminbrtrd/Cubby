<!-- The datapacks of one world, in the Worlds tab's side panel: turn them on and off, add .zip files, or
     open Discover › Datapacks set to install into this world. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, IconButton, InspectorSection, SourceMark, Switch } from '../../cubby'
  import VersionDialog, { type VersionTarget } from './VersionDialog.svelte'
  import { bytes, plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { Datapack, DatapackSource, Instance, World } from '../../types'

  let { instance, world }: { instance: Instance; world: World } = $props()

  const id = $derived(instance.id)
  const running = $derived(!!app.running[id])

  let packs = $state<Datapack[] | null>(null)
  let adding = $state(false)
  /** The packs Modrinth or CurseForge knows, by file name. */
  let sources = $state<Map<string, DatapackSource>>(new Map())
  /** The pack whose versions are shown. */
  let versionsOf = $state<VersionTarget | null>(null)

  async function load() {
    const w = world.folder
    try {
      const list = await api.listDatapacks(id, w)
      if (w === world.folder) packs = list
    } catch (e) {
      packs = []
      app.error(e)
    }
    api
      .datapackSources(id, w)
      .then((found) => {
        if (w === world.folder) sources = new Map(found.map((s) => [s.fileName, s]))
      })
      .catch(() => {})
  }

  function versions(p: Datapack, s: DatapackSource) {
    versionsOf = { source: s.source, projectId: s.projectId, kind: 'datapack', title: s.title, datapack: { world: world.folder, fileName: p.fileName } }
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

{#snippet packActions()}
  <Button size="sm" variant="ghost" icon="search" disabled={running} onclick={() => app.openDatapacks(id, world.folder)}>{t('Find')}</Button>
  <IconButton size="sm" icon="plus" label={t('Add from file')} disabled={adding || running} onclick={addFiles} />
{/snippet}
<InspectorSection title={t('Datapacks')} count={packs?.length ?? null} action={packActions}>
  {#if packs === null}
    <p class="cb-ins-hint">{t('Reading datapacks…')}</p>
  {:else if !packs.length}
    <p class="cb-ins-hint">{t('Datapacks add structures, biomes or rules to one world, without mods.')}</p>
  {:else}
    <ul class="cb-ins-rows">
      {#each packs as p (p.fileName)}
        {@const src = sources.get(p.fileName)}
        <li class={['cb-ins-row', !p.enabled && 'cb-ins-row--off']}>
          <SourceMark tone={src?.source ?? 'local'} label={src ? (src.source === 'modrinth' ? 'Modrinth' : 'CurseForge') : t('Local file')} size={16} />
          <div class="cb-ins-row__txt">
            <span class="cb-ins-row__t" title={p.fileName}>{src?.title ?? p.name}</span>
            <span class="cb-ins-row__s" title={p.description ?? ''}>{p.description ?? (p.isDir ? t('Folder') : bytes(p.size))}</span>
          </div>
          <Switch checked={p.enabled} label={t('Load {name} in this world', { name: p.name })} hideLabel disabled={running} onChange={(on) => toggle(p, on)} />
          {#if src}<IconButton icon="clock" size="sm" variant="ghost" label={t('Versions of {title}', { title: src.title })} disabled={running} onclick={() => versions(p, src)} />{/if}
          <IconButton icon="trash" size="sm" variant="ghost" label={t('Remove {name}', { name: p.name })} disabled={running} onclick={() => remove(p)} />
        </li>
      {/each}
    </ul>
  {/if}
</InspectorSection>

{#if versionsOf}
  <VersionDialog instanceId={id} item={versionsOf} onClose={() => ((versionsOf = null), void load())} />
{/if}

<style>
  p,
  ul {
    margin: 0;
  }
  ul {
    padding: 0;
    list-style: none;
  }
</style>
