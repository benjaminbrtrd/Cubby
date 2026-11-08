<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog, Icon, IconButton, Inspector, InspectorHead, InspectorNotice, InspectorPath, InspectorSection, InspectorStatus, MetaChip, PlayButton } from '../../cubby'
  import { bytes, plural, relativeTime } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, World, WorldBackup } from '../../types'
  import { when, type InstanceData } from './InstancePage.svelte'
  import { opensWorlds } from '../library/Stage.svelte'
  import WorldDatapacks from './WorldDatapacks.svelte'
  import WorldMap from './WorldMap.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  const running = $derived(!!app.running[id])
  const preparing = $derived(!!app.instanceTask(id))

  untrack(() => {
    if (!data.backups) void data.loadBackups()
  })

  const worlds = $derived(
    [...(data.worlds ?? [])].sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? '') || a.name.localeCompare(b.name)),
  )
  const total = $derived(worlds.reduce((s, w) => s + w.sizeBytes, 0))

  let selected = $state<string | null>(null)
  const sel = $derived(worlds.find((w) => w.folder === selected) ?? worlds[0] ?? null)
  const selBackups = $derived(sel ? backupsOf(sel) : [])
  let asideTitle: HTMLElement | undefined = $state()

  let backingUp = $state<string | null>(null)
  let restoring = $state<WorldBackup | null>(null)
  let deleting = $state<World | null>(null)
  let busy = $state(false)

  function backupsOf(w: World): WorldBackup[] {
    return (data.backups ?? []).filter((b) => b.world === w.folder)
  }

  async function select(w: World) {
    selected = w.folder
    await tick()
    asideTitle?.focus()
  }

  /** Minecraft 1.20 and newer open a world straight from the launcher; older versions start on the title screen. */
  const direct = $derived(opensWorlds(instance.minecraft))

  function play(world: string | null = null) {
    if (running) void app.kill(id)
    else if (!preparing) void app.launch(id, false, null, direct ? world : null)
  }

  async function backUp(w: World) {
    if (backingUp) return
    backingUp = w.folder
    try {
      await api.backupWorld(id, w.folder)
      await data.loadBackups()
      app.toast(t('{name} is backed up.', { name: w.name }), 'success')
    } catch (e) {
      app.error(e)
    } finally {
      backingUp = null
    }
  }

  async function restore() {
    const b = restoring
    if (!b) return
    busy = true
    try {
      await api.restoreBackup(id, b.fileName)
      restoring = null
      app.toast(t('Restored the backup from {true}.', { true: when(b.created, true) }), 'success')
      await Promise.all([data.loadWorlds(), data.loadBackups()])
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  async function remove() {
    const w = deleting
    if (!w) return
    busy = true
    try {
      await api.deleteWorld(id, w.folder)
      deleting = null
      data.worlds = (data.worlds ?? []).filter((x) => x.folder !== w.folder)
      if (selected === w.folder) selected = null
      app.toast(t('{name} moved to the trash.', { name: w.name }), 'success')
      void data.loadWorlds()
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  const playState = $derived(running ? 'running' : preparing ? 'installing' : 'ready')
</script>

{#snippet thumb(w: World, big = false)}
  {#if w.iconUrl}
    <img class={['thumb', big && 'thumb--big']} src={w.iconUrl} alt="" />
  {:else}
    <span class={['thumb thumb--empty', big && 'thumb--big']} aria-hidden="true"><Icon name="cube" size={27} /></span>
  {/if}
{/snippet}

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Worlds')}>
  <div class="toolbar">
    <h2 class="text-heading">{data.worlds ? plural(worlds.length, 'world', 'worlds') : t('Worlds')}</h2>
    {#if worlds.length}<span class="text-small muted">{t('{n} in all', { n: bytes(total) })}</span>{/if}
    <span class="grow"></span>
    <Button icon="folder" onclick={() => data.openFolder('saves')}>{t('Open saves folder')}</Button>
  </div>

  {#if data.worlds === null}
    <p class="muted text-body">{t('Looking for worlds…')}</p>
  {:else if !worlds.length}
    <div class="empty">
      <p class="text-heading">{t('No worlds yet')}</p>
      <p class="muted text-body">{t('Play, then create a world in Singleplayer. It shows up here.')}</p>
      <PlayButton size="sm" state={playState} onclick={() => play()} />
    </div>
  {:else}
    <div class="grid">
      {#each worlds as w (w.folder)}
        {@const n = backupsOf(w).length}
        <article class={['world', sel?.folder === w.folder && 'is-selected']} aria-label={w.name}>
          <div class="world__top">
            {@render thumb(w)}
            <div class="world__text">
              <!-- The title is a button stretched over the whole card: a click anywhere on it shows the world in the panel. -->
              <h3 class="text-heading clip" title={w.name}>
                <button type="button" class="world__hit" aria-pressed={sel?.folder === w.folder} onclick={() => select(w)}>{w.name}</button>
              </h3>
              <p class="text-small muted">{w.lastPlayed ? relativeTime(w.lastPlayed) : t('Not played yet')} · {bytes(w.sizeBytes)}</p>
              <p class="text-small muted">{n ? plural(n, 'backup', 'backups') : t('No backups')}</p>
            </div>
          </div>
          <div class="world__actions">
            <PlayButton size="sm" state={playState} onclick={() => play(w.folder)} />
            <Button icon="copy" busy={backingUp === w.folder} disabled={!!backingUp} onclick={() => backUp(w)}>{t('Back up')}</Button>
          </div>
        </article>
      {/each}
    </div>
    {#if !direct}<p class="text-small muted">{t('Minecraft {minecraft} can’t open a world from a launcher: Play starts on the title screen, then pick the world in Singleplayer.', { minecraft: instance.minecraft })}</p>{/if}
  {/if}
</div>

<aside class="ip-aside" aria-label={t('World details')}>
  <Inspector class="ip-aside__inner" danger={sel ? { label: t('Delete world'), hint: t('Moves to the trash. Its backups stay.'), disabled: running, onclick: () => (deleting = sel) } : null}>
    {#if sel}
      {#snippet worldMedia()}
        {#if sel.iconUrl}
          <span class="cb-ins-media cb-ins-media--wide"><img src={sel.iconUrl} alt="" /></span>
        {:else}
          <span class="cb-ins-media cb-ins-media--wide cb-ins-media--empty"><Icon name="cube" size={36} /></span>
        {/if}
      {/snippet}
      {#snippet worldMeta()}
        <MetaChip>{bytes(sel.sizeBytes)}</MetaChip>
        {#if running}<InspectorStatus tone="ok">{t('Game running')}</InspectorStatus>{/if}
        <InspectorStatus tone={selBackups.length ? 'ok' : 'off'}>{selBackups.length ? plural(selBackups.length, 'backup', 'backups') : t('No backups')}</InspectorStatus>
      {/snippet}
      <InspectorHead eyebrow={t('World')} title={sel.name} sub={sel.lastPlayed ? t('Last played {when}', { when: when(sel.lastPlayed) }) : t('Not played yet')} media={worldMedia} wide meta={worldMeta} bind:titleEl={asideTitle} />
      <div class="cb-ins-acts">
        {#if running}
          <Button class="wide" icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>{t('Show the running game')}</Button>
        {:else}
          <Button variant="primary" class="wide" icon="play" disabled={preparing} onclick={() => sel && play(sel.folder)}>{direct ? t('Play this world') : t('Play')}</Button>
        {/if}
      </div>
      {#if running}
        <InspectorNotice tone="info">{t('Close the game to restore a backup, change datapacks or delete this world.')}</InspectorNotice>
      {:else if !direct}
        <p class="cb-ins-hint">{t('Minecraft {minecraft} can’t open a world from a launcher: Play starts on the title screen.', { minecraft: instance.minecraft })}</p>
      {/if}
      {#snippet backupAction()}
        <Button size="sm" variant="ghost" icon="copy" busy={backingUp === sel.folder} disabled={!!backingUp} onclick={() => sel && backUp(sel)}>{t('Back up')}</Button>
      {/snippet}
      <InspectorSection title={t('Backups')} count={data.backups ? selBackups.length : null} action={backupAction}>
        {#if data.backups === null}
          <p class="cb-ins-hint">{t('Loading backups…')}</p>
        {:else if !selBackups.length}
          <p class="cb-ins-hint">{t('No backups yet. A backup is a zip of the world you can go back to.')}</p>
        {:else}
          <ul class="cb-ins-rows">
            {#each selBackups as b (b.fileName)}
              <li class="cb-ins-row">
                <div class="cb-ins-row__txt">
                  <span class="cb-ins-row__t">{when(b.created)}</span>
                  <span class="cb-ins-row__s">{bytes(b.sizeBytes)}</span>
                </div>
                <Button size="sm" disabled={running} onclick={() => (restoring = b)}>{t('Restore')}</Button>
              </li>
            {/each}
          </ul>
        {/if}
      </InspectorSection>
      <WorldMap {instance} world={sel} />
      <WorldDatapacks {instance} world={sel} />
      <InspectorSection title={t('Details')}>
        <dl class="cb-ins-facts">
          <dt>{t('Size')}</dt><dd><span>{bytes(sel.sizeBytes)}</span></dd>
          <InspectorPath path={`saves/${sel.folder}`} openLabel={t('Open folder')} onopen={() => sel && data.openFolder(`saves/${sel.folder}`)} />
        </dl>
      </InspectorSection>
    {:else}
      {#snippet noWorldMedia()}<span class="cb-ins-media cb-ins-media--wide cb-ins-media--empty"><Icon name="globe" size={36} /></span>{/snippet}
      <InspectorHead eyebrow={t('World')} title={t('No worlds yet')} sub={instance.name} media={noWorldMedia} wide empty />
      <p class="cb-ins-hint">{t('Play, then create a world in Singleplayer. It shows up here with its backups, map and datapacks.')}</p>
      <div class="cb-ins-acts">
        <Button variant="primary" icon="play" disabled={preparing || running} onclick={() => play()}>{t('Play')}</Button>
        <Button icon="folder" class="wide" onclick={() => data.openFolder('saves')}>{t('Open saves folder')}</Button>
      </div>
    {/if}
  </Inspector>
</aside>

{#if restoring}
  {@const b = restoring}
  {#snippet restoreActions()}
    <Button variant="ghost" onclick={() => (restoring = null)}>{t('Cancel')}</Button>
    <Button variant="primary" icon="update" busy={busy} disabled={busy} onclick={restore}>{t('Restore')}</Button>
  {/snippet}
  <Dialog tone="warning" icon="warning" title={t('Restore {world} from {true}?', { world: b.world, true: when(b.created, true) })} onClose={() => (restoring = null)} actions={restoreActions}>
    <p>{t('The world goes back to how it was then. Cubby backs up the world as it is now first, so you can undo this.')}</p>
  </Dialog>
{/if}

{#if deleting}
  {@const w = deleting}
  {#snippet deleteActions()}
    <Button variant="ghost" onclick={() => (deleting = null)}>{t('Cancel')}</Button>
    <Button variant="danger" icon="trash" busy={busy} disabled={busy} onclick={remove}>{t('Move to trash')}</Button>
  {/snippet}
  <Dialog tone="danger" icon="trash" title={t('Move {name} to the trash?', { name: w.name })} onClose={() => (deleting = null)} actions={deleteActions}>
    <p>{t('Moves the world folder')} <span class="text-mono">{w.folder}</span> {t('to Cubby’s trash. Its backups stay in this instance.')}</p>
  </Dialog>
{/if}

<style>
  p,
  h2,
  h3,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
  }
  .clip {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--space-4);
  }
  .world {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
    padding: var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .world:hover {
    background: var(--surface-hover);
  }
  .world.is-selected {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .world__top {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .world__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .world__actions {
    position: relative;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  /* Play and Back up: the same height, so the two buttons read as a pair */
  .world__actions :global(.cb-btn),
  .world__actions :global(.cb-play) {
    box-sizing: border-box;
    height: var(--size-control);
    min-height: var(--size-control);
  }
  /* ...and the same shadow at rest (Play's is deeper than a Button's by default) */
  .world__actions :global(.cb-btn:not(:hover):not(:active):not(:focus-visible)),
  .world__actions :global(.cb-play:not(:hover):not(:active):not(:focus-visible)) {
    box-shadow: var(--shadow-block-sm);
  }
  .world__hit {
    all: unset;
    cursor: pointer;
  }
  .world__hit::after {
    content: '';
    position: absolute;
    inset: 0;
  }
  .world__hit:focus-visible::after {
    box-shadow: var(--focus-ring);
  }
  .thumb {
    display: block;
    flex: none;
    box-sizing: border-box;
    width: 72px;
    height: 72px;
    object-fit: cover;
    image-rendering: pixelated;
    border: var(--border-block) solid var(--edge);
  }
  .thumb--empty {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ink-muted);
    background: var(--field);
  }
  .thumb--big {
    width: 128px;
    height: 128px;
    box-shadow: var(--shadow-block-sm);
  }
</style>
