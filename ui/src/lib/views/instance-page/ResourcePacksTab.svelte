<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon, IconButton, Inspector, InspectorHead, InspectorPath, InspectorSection, InspectorStatus, InstanceIcon, MetaChip } from '../../cubby'
  import { bytes, num, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Instance, Pack } from '../../types'
  import type { InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  const packs = $derived(data.resourcePacks)
  // The backend lists the packs that are on first, highest priority first.
  const on = $derived((packs ?? []).filter((p) => p.enabled))
  const off = $derived((packs ?? []).filter((p) => !p.enabled))
  /** The game rewrites options.txt while it runs: change packs once it's closed. */
  const locked = $derived(!!app.running[id])
  const updating = $derived(!!app.tasks[`update:${id}`])

  let selected = $state<string | null>(null)
  const sel = $derived(packs?.find((p) => p.fileName === selected) ?? on[0] ?? off[0] ?? null)
  const rank = $derived(sel?.enabled ? on.findIndex((p) => p.fileName === sel.fileName) + 1 : 0)
  const selSource = $derived(sel ? data.source(sel.fileName) : undefined)

  let list: HTMLElement | undefined = $state()

  async function persist(next: Pack[], focus?: string) {
    const before = data.resourcePacks
    data.resourcePacks = next
    if (focus) {
      await tick()
      const btn = list?.querySelector<HTMLButtonElement>(`[data-focus="${CSS.escape(focus)}"]:not(:disabled)`)
      btn?.focus()
    }
    try {
      await api.setResourcePacks(
        id,
        next.filter((p) => p.enabled).map((p) => p.fileName),
      )
    } catch (e) {
      data.resourcePacks = before
      app.error(e)
    }
  }

  function move(from: number, to: number) {
    if (to < 0 || to >= on.length || from === to) return
    const next = [...on]
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    // keep the focus on the moved pack, on the same arrow when it can still go that way
    const dir = to < from ? 'up' : 'down'
    const end = to === 0 || to === on.length - 1
    void persist([...next, ...off], `${item.fileName}:${end ? (dir === 'up' ? 'down' : 'up') : dir}`)
  }

  function turnOn(p: Pack) {
    // a pack turned on goes to the top: it wins over the others
    void persist([{ ...p, enabled: true }, ...on, ...off.filter((x) => x.fileName !== p.fileName)])
    selected = p.fileName
  }

  function turnOff(p: Pack) {
    void persist([...on.filter((x) => x.fileName !== p.fileName), { ...p, enabled: false }, ...off])
  }

  async function removePack(p: Pack) {
    try {
      await api.removePack(id, 'resource', p.fileName)
      app.forgetFile(id, p.fileName)
      if (selected === p.fileName) selected = null
      app.toast(t('{name} moved to the instance’s trash folder.', { name: p.name }), 'success')
    } catch (e) {
      app.error(e)
    }
    await data.loadPacks()
  }

  const describe = (p: Pack) => `${p.isDir ? t('Folder') : t('Zip file')} · ${bytes(p.size)}`
</script>

{#snippet updateButton(p: Pack)}
  {@const up = data.update(p.fileName)}
  {#if up}
    <Button size="sm" icon="update" busy={updating} disabled={updating || locked} onclick={() => data.applyUpdates([up.fileName])}>{t('Update to {latestVersion}', { latestVersion: up.latestVersion })}</Button>
  {/if}
{/snippet}

{#snippet art(p: Pack, raised = false, size = 32)}
  {@const src = data.source(p.fileName)}
  {#if src?.iconUrl}
    <InstanceIcon src={src.iconUrl} {size} {raised} />
  {:else}
    <InstanceIcon {...projectArt(src?.slug ?? p.name)} {size} {raised} />
  {/if}
{/snippet}

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Resource packs')} bind:this={list}>
  <div class="note">
    <span class="info icon"><Icon name="info" size={18} /></span>
    <div class="note__text">
      <p class="text-body-strong">{t('Packs higher in the list win when two change the same texture.')}</p>
      <p class="text-small muted">{t('Resource packs change how blocks, items and menus look. They never change how the game plays.')}</p>
    </div>
  </div>

  {#if locked}
    <Banner tone="info" icon="play" title={t('The game is running')}>
      {t('Close it to change packs here, or change them in game under Options › Resource packs.')}
    </Banner>
  {/if}

  {#if packs === null}
    <p class="muted text-body">{t('Reading the resource packs folder…')}</p>
  {:else}
    <div class="cols">
      <section class="stack" aria-labelledby="rp-on">
        <div class="head">
          <h2 id="rp-on" class="text-label">{t('On, in load order')}</h2>
          <span class="text-small muted">{t('Number 1 is on top and wins.')}</span>
        </div>
        <ol class="list">
          {#each on as p, i (p.fileName)}
            <li class={['pack', sel?.fileName === p.fileName && 'is-selected']}>
              <span class="rank text-counter">{num(i + 1)}</span>
              {@render art(p)}
              <div class="pack__body">
                <button type="button" class="pack__name text-body-strong" aria-pressed={sel?.fileName === p.fileName} onclick={() => (selected = p.fileName)}>{p.name}</button>
                <p class="text-small muted">{describe(p)}</p>
                <div class="pack__tools">
                  <IconButton icon="update" size="sm" label={t('Move {name} up', { name: p.name })} data-focus={`${p.fileName}:up`} disabled={i === 0 || locked} onclick={() => move(i, i - 1)} />
                  <IconButton icon="download" size="sm" label={t('Move {name} down', { name: p.name })} data-focus={`${p.fileName}:down`} disabled={i === on.length - 1 || locked} onclick={() => move(i, i + 1)} />
                  <Button size="sm" variant="ghost" icon="minimize" disabled={locked} onclick={() => turnOff(p)}>{t('Turn off')}</Button>
                  {@render updateButton(p)}
                </div>
              </div>
            </li>
          {/each}
          <li class="pack">
            <span class="rank rank--default text-counter">{num(on.length + 1)}</span>
            <InstanceIcon motif="block" ground="moss" size={32} />
            <div class="pack__body">
              <div class="pack__line">
                <span class="text-body-strong">{t('Default')}</span>
                <span class="text-small muted">{t('Minecraft’s own')}</span>
              </div>
              <p class="text-small muted">{t('Always last, so anything the packs above leave out still has a texture.')}</p>
              <span class="text-small muted locked"><Icon name="key" size={18} />{t('Locked')}</span>
            </div>
          </li>
        </ol>
      </section>

      <section class="stack" aria-labelledby="rp-off">
        <div class="head">
          <h2 id="rp-off" class="text-label">{t('Available')}</h2>
          <span class="text-small muted">{t('Installed but off. They don’t load.')}</span>
        </div>
        {#if off.length}
          <ul class="list">
            {#each off as p (p.fileName)}
              <li class={['pack', sel?.fileName === p.fileName && 'is-selected']}>
                {@render art(p)}
                <div class="pack__body">
                  <button type="button" class="pack__name text-body-strong" aria-pressed={sel?.fileName === p.fileName} onclick={() => (selected = p.fileName)}>{p.name}</button>
                  <p class="text-small muted">{describe(p)}</p>
                  <div class="pack__tools">
                    <Button size="sm" icon="plus" disabled={locked} onclick={() => turnOn(p)}>{t('Turn on')}</Button>
                    {@render updateButton(p)}
                  </div>
                </div>
              </li>
            {/each}
          </ul>
          <p class="text-small muted">{t('New packs land here, turned off, until you turn them on.')}</p>
        {:else}
          <p class="empty text-body">{packs.length ? t('Every installed pack is on.') : t('No resource packs yet.')}</p>
        {/if}
        <div class="actions">
          <Button variant="primary" icon="search" onclick={() => data.discover('resourcepack')}>{t('Find resource packs')}</Button>
          <Button icon="folder" onclick={() => data.openFolder('resourcepacks')}>{t('Open folder')}</Button>
        </div>
      </section>
    </div>
  {/if}
</div>

<aside class="ip-aside" aria-label={t('Pack details')}>
  <Inspector class="ip-aside__inner" danger={sel ? { label: t('Remove'), hint: t('Moves the pack to the instance’s trash.'), disabled: locked, onclick: () => sel && void removePack(sel) } : null}>
    {#if sel}
      {@const up = data.update(sel.fileName)}
      {@const i = on.findIndex((p) => p.fileName === sel.fileName)}
      {#snippet packMedia()}<span class="cb-ins-media" style:opacity={sel.enabled ? undefined : 0.5}>{@render art(sel, false, 52)}</span>{/snippet}
      {#snippet packMeta()}
        {#if selSource?.versionNumber}<MetaChip>{selSource.versionNumber}</MetaChip>{/if}
        {#if sel.enabled}<InspectorStatus tone="ok">{t('On · {n} of {n2}', { n: num(rank), n2: num(on.length) })}</InspectorStatus>{:else}<InspectorStatus tone="off">{t('Off')}</InspectorStatus>{/if}
        {#if up}<InspectorStatus tone="update">{t('Update available')}</InspectorStatus>{/if}
      {/snippet}
      <InspectorHead eyebrow={t('Resource pack')} title={selSource?.title ?? sel.name} sub={selSource ? null : describe(sel)} media={packMedia} meta={packMeta} />
      <div class="cb-ins-acts">
        {#if up}
          <Button variant="primary" class="wide" icon="update" busy={updating} disabled={updating || locked} onclick={() => data.applyUpdates([up.fileName])}>{t('Update to {latestVersion}', { latestVersion: up.latestVersion })}</Button>
        {/if}
        {#if sel.enabled}
          <Button icon="minimize" disabled={locked} onclick={() => sel && turnOff(sel)}>{t('Turn off')}</Button>
        {:else}
          <Button variant={up ? 'secondary' : 'primary'} class={up ? undefined : 'wide'} icon="check" disabled={locked} onclick={() => sel && turnOn(sel)}>{t('Turn on')}</Button>
        {/if}
        {#if selSource}<Button icon="globe" onclick={() => selSource && data.project(selSource.slug, 'resourcepack')}>{t('Open page')}</Button>{/if}
        {#if sel.enabled && on.length > 1}
          <Button icon="update" disabled={i === 0 || locked} onclick={() => move(i, i - 1)}>{t('Move up')}</Button>
          <Button icon="download" disabled={i === on.length - 1 || locked} onclick={() => move(i, i + 1)}>{t('Move down')}</Button>
        {/if}
      </div>
      <p class="cb-ins-hint">{locked ? t('Close the game to change resource packs.') : sel.enabled && on.length > 1 ? t('The pack on top wins. Changes apply the next time you start the game.') : t('Changes apply the next time you start the game.')}</p>
      <InspectorSection title={t('Details')}>
        <dl class="cb-ins-facts">
          <dt>{t('Kind')}</dt><dd><span>{sel.isDir ? t('Folder') : t('Zip file')}</span></dd>
          <dt>{t('Source')}</dt><dd><span>{selSource ? 'Modrinth' : t('Not found on Modrinth')}</span></dd>
          {#if sel.modified}<dt>{t('Changed')}</dt><dd><span>{relativeTime(sel.modified)}</span></dd>{/if}
          <dt>{t('Size')}</dt><dd><span>{bytes(sel.size)}</span></dd>
          <InspectorPath path={sel.fileName} openLabel={t('Open folder')} onopen={() => data.openFolder('resourcepacks')} />
        </dl>
      </InspectorSection>
    {:else}
      {#snippet emptyMedia()}<span class="cb-ins-media cb-ins-media--empty"><Icon name="captures" size={27} /></span>{/snippet}
      <InspectorHead eyebrow={t('Resource pack')} title={t('No resource packs')} sub={t('This instance uses the game’s textures')} media={emptyMedia} empty />
      <p class="cb-ins-hint">{t('Resource packs change textures, sounds and fonts, without mods.')}</p>
      <div class="cb-ins-acts">
        <Button variant="primary" icon="search" onclick={() => data.discover('resourcepack')}>{t('Find resource packs')}</Button>
        <Button icon="folder" class="wide" onclick={() => data.openFolder('resourcepacks')}>{t('Open folder')}</Button>
      </div>
    {/if}
  </Inspector>
</aside>

<style>
  p,
  h2,
  dl,
  dd,
  ol,
  ul {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .info {
    color: var(--diamond-text);
  }
  .icon {
    display: inline-flex;
    flex: none;
  }
  .note {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .note__text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: var(--space-5);
    align-items: start;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-3);
  }
  .list {
    padding: 0;
    list-style: none;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .pack {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
  }
  .pack + .pack {
    border-top: var(--border-hair) solid var(--line);
  }
  .pack.is-selected {
    background: var(--surface-hover);
  }
  .pack.is-selected::after {
    content: '';
    position: absolute;
    inset: 0;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
  }
  .rank {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 28px;
    height: 28px;
    margin-top: 2px;
    color: var(--ink);
    background: var(--bg-chrome);
    border: var(--border-block) solid var(--edge);
  }
  .rank--default {
    color: var(--ink-muted);
    border-style: dashed;
    border-color: var(--outline);
  }
  .pack__body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .pack__line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px var(--space-2);
  }
  .pack__name {
    max-width: 100%;
    padding: 0;
    color: var(--ink);
    text-align: start;
    overflow-wrap: anywhere;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .pack__name:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .pack__name:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .pack__tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1);
    margin-top: 6px;
  }
  .locked {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }
  .empty {
    padding: var(--space-4);
    color: var(--ink-muted);
    background: var(--surface);
    border: var(--border-block) dashed var(--outline);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }
</style>
