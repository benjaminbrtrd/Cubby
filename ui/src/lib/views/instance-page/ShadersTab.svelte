<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, Inspector, InspectorHead, InspectorNotice, InspectorPath, InspectorSection, InspectorStatus, InstanceIcon, MetaChip, StatusTag } from '../../cubby'
  import { bytes, relativeTime } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Instance, ModFile, Pack } from '../../types'
  import type { InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  const packs = $derived(data.shaderPacks)
  const current = $derived(packs?.find((p) => p.enabled) ?? null)
  const currentSource = $derived(current ? data.source(current.fileName) : undefined)
  /** Iris and OptiFine rewrite their settings while the game runs. */
  const locked = $derived(!!app.running[id])
  const updating = $derived(!!app.tasks[`update:${id}`])
  let saving = $state(false)

  // Forge uses the Oculus and Embeddium ports of Iris and Sodium.
  const forge = $derived(instance.loader.kind === 'forge')
  const iris = $derived({ name: forge ? 'Oculus' : 'Iris', slug: forge ? 'oculus' : 'iris' })
  const sodium = $derived({ name: forge ? 'Embeddium' : 'Sodium', slug: forge ? 'embeddium' : 'sodium' })

  type Need = { name: string; mod: ModFile | undefined }
  type Requirement =
    | { ok: true; title: string; text: string; needs: Need[] }
    | { ok: false; title: string; text: string; needs: Need[]; fix: { label: string; icon: 'search' | 'wrench' | 'mods'; run: () => void } }

  const requirement = $derived.by<Requirement | null>(() => {
    if (data.mods === null) return null
    const m = data.shaderMods
    const needs: Need[] = [
      { name: iris.name, mod: m.iris },
      { name: sodium.name, mod: m.sodium },
    ]
    const find = (p: { slug: string }) => () => data.project(p.slug, 'mod')
    if (m.optifine?.enabled)
      return { ok: true, title: t('Shaders run through OptiFine'), text: t('OptiFine loads the shader pack. It is installed and on.'), needs: [{ name: 'OptiFine', mod: m.optifine }] }
    if (instance.loader.kind === 'vanilla')
      return {
        ok: false,
        title: t('Shaders need a mod loader'),
        text: t('Shader packs run through Iris, a mod. This instance is Vanilla, which doesn’t load mods. Pick Fabric or Quilt in Settings, then add Iris and Sodium.'),
        needs,
        fix: { label: t('Change loader'), icon: 'wrench', run: () => app.openInstance(id, 'settings') },
      }
    if (!m.iris)
      return { ok: false, title: t('Shaders need {name}', { name: iris.name }), text: t('{name} is the mod that loads shader packs. It isn’t installed in this instance.', { name: iris.name }), needs, fix: { label: t('Find {name}', { name: iris.name }), icon: 'search', run: find(iris) } }
    if (!m.iris.enabled)
      return { ok: false, title: `${iris.name} is turned off`, text: t('Turn {name} on in Mods to use shader packs.', { name: iris.name }), needs, fix: { label: t('Open Mods'), icon: 'mods', run: () => app.openInstance(id, 'mods') } }
    if (!m.sodium)
      return { ok: false, title: `${iris.name} needs ${sodium.name}`, text: t('{name} uses {name2} to draw the world. Add {name2} to use shader packs.', { name: iris.name, name2: sodium.name }), needs, fix: { label: t('Find {name}', { name: sodium.name }), icon: 'search', run: find(sodium) } }
    if (!m.sodium.enabled)
      return { ok: false, title: `${sodium.name} is turned off`, text: t('{name} needs it. Turn {name2} on in Mods.', { name: iris.name, name2: sodium.name }), needs, fix: { label: t('Open Mods'), icon: 'mods', run: () => app.openInstance(id, 'mods') } }
    return { ok: true, title: t('Shaders run through {name}', { name: iris.name }), text: t('{name} loads the shader pack, and {name2} keeps the game fast while it runs. Both are installed.', { name: iris.name, name2: sodium.name }), needs }
  })

  const irisUpdate = $derived(data.shaderMods.iris ? data.update(data.shaderMods.iris.fileName) : undefined)

  async function use(fileName: string | null) {
    if (locked || saving) return
    const before = data.shaderPacks
    data.shaderPacks = (data.shaderPacks ?? []).map((p) => ({ ...p, enabled: p.fileName === fileName }))
    saving = true
    try {
      await api.setShaderPack(id, fileName)
    } catch (e) {
      data.shaderPacks = before
      app.error(e)
    } finally {
      saving = false
    }
  }

  // Which pack the side panel shows: the one clicked, else the one in use. NONE is the "no shaders" card.
  const NONE = '\u0000none'
  let selected = $state<string | null>(null)
  const shown = $derived(selected === NONE ? null : (packs?.find((p) => p.fileName === selected) ?? current))
  const shownIsNone = $derived(selected === NONE || (!packs?.some((p) => p.fileName === selected) && !current))
  const shownSource = $derived(shown ? data.source(shown.fileName) : undefined)
  const shownUpdate = $derived(shown ? data.update(shown.fileName) : undefined)

  let deleting = $state<Pack | null>(null)
  let removing = $state(false)

  async function remove() {
    const p = deleting
    if (!p || removing) return
    removing = true
    try {
      await api.removePack(id, 'shader', p.fileName)
      deleting = null
      if (selected === p.fileName) selected = null
      await data.loadPacks()
      app.toast(t('{name} moved to the trash.', { name: data.source(p.fileName)?.title ?? p.name }), 'success')
    } catch (e) {
      app.error(e)
    } finally {
      removing = false
    }
  }

  const describe = (p: Pack) => `${p.isDir ? t('Folder') : t('Zip file')} · ${bytes(p.size)}`
</script>

{#snippet art(p: Pack)}
  {@const src = data.source(p.fileName)}
  {#if src?.iconUrl}
    <InstanceIcon src={src.iconUrl} size={32} />
  {:else}
    <InstanceIcon {...projectArt(src?.slug ?? p.name)} size={32} />
  {/if}
{/snippet}

{#snippet fixAction()}
  {#if requirement && !requirement.ok}
    <Button size="sm" icon={requirement.fix.icon} onclick={requirement.fix.run}>{requirement.fix.label}</Button>
  {/if}
{/snippet}

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Shader packs')}>
  {#if requirement}
    <Banner tone={requirement.ok ? 'success' : 'warning'} title={requirement.title} actions={requirement.ok ? undefined : fixAction}>
      <div class="req">
        <p>{requirement.text}</p>
        <div class="req__items">
          {#each requirement.needs as n (n.name)}
            {#if n.mod?.enabled}
              <span class="req__item"><span class="ok icon"><Icon name="check" size={18} /></span>{t('{name} installed', { name: n.mod.version ? `${n.name} ${n.mod.version}` : n.name })}</span>
            {:else if n.mod}
              <span class="req__item"><span class="warn icon"><Icon name="minimize" size={18} /></span>{t('{name} installed, turned off', { name: n.name })}</span>
            {:else}
              <span class="req__item"><span class="warn icon"><Icon name="warning" size={18} /></span>{t('{name} missing', { name: n.name })}</span>
            {/if}
          {/each}
          {#if irisUpdate}
            <span class="req__item info"><Icon name="update" size={18} />{t('{name} {latestVersion} available', { name: iris.name, latestVersion: irisUpdate.latestVersion })}</span>
            <button type="button" class="link text-body-strong" onclick={() => app.openInstance(id, 'mods')}>{t('Update in Mods')}</button>
          {/if}
        </div>
      </div>
    </Banner>
  {/if}

  {#if locked}
    <Banner tone="info" icon="play" title={t('The game is running')}>
      {t('Close it to pick a shader pack here, or pick one in game under Options › Video settings › Shader packs.')}
    </Banner>
  {/if}

  <section class="stack" aria-labelledby="sh-title">
    <div class="toolbar">
      <div class="toolbar__title">
        <h2 id="sh-title" class="text-label">{t('Shader packs')}</h2>
        <span class="text-small muted">{t('Only one can be on at a time.')}</span>
      </div>
      <Button icon="folder" onclick={() => data.openFolder('shaderpacks')}>{t('Open folder')}</Button>
      <Button variant="primary" icon="search" onclick={() => data.discover('shader')}>{t('Find shader packs')}</Button>
    </div>

    {#if packs === null}
      <p class="muted text-body">{t('Reading the shader packs folder…')}</p>
    {:else}
      <div class="grid">
        <div class={['card', !current && 'is-on', shownIsNone && 'is-shown']}>
          <div class="card__head">
            <InstanceIcon motif="block" ground="sky" size={32} />
            <div class="card__title">
              <!-- A button stretched over the whole card: a click anywhere on it shows it in the panel. -->
              <p class="text-body-strong"><button type="button" class="card__hit" aria-pressed={shownIsNone} onclick={() => (selected = NONE)}>{t('None')}</button></p>
              <p class="text-small muted">{t('Minecraft’s own look')}</p>
            </div>
          </div>
          <p class="text-small muted">{t('No shader pack. The game looks as Mojang made it and runs at its fastest.')}</p>
          <div class="card__foot">
            {#if !current}
              <StatusTag state="ready">{t('In use')}</StatusTag>
            {:else}
              <Button size="sm" disabled={locked || saving} onclick={() => use(null)}>{t('Use no shaders')}</Button>
            {/if}
          </div>
        </div>
        {#each packs as p (p.fileName)}
          {@const src = data.source(p.fileName)}
          <div class={['card', p.enabled && 'is-on', shown?.fileName === p.fileName && 'is-shown']}>
            <div class="card__head">
              {@render art(p)}
              <div class="card__title">
                <p class="text-body-strong wrap"><button type="button" class="card__hit" aria-pressed={shown?.fileName === p.fileName} onclick={() => (selected = p.fileName)}>{src?.title ?? p.name}</button></p>
                <p class="text-small muted">{src ? src.versionNumber : describe(p)}</p>
              </div>
            </div>
            <p class="text-mono muted wrap">{p.fileName}</p>
            {#if data.update(p.fileName)}
              {@const up = data.update(p.fileName)!}
              <div class="card__update">
                <StatusTag state="update" detail={up.latestVersion}>{t('Update')}</StatusTag>
                <Button size="sm" icon="update" busy={updating} disabled={updating || locked} onclick={() => data.applyUpdates([up.fileName])}>{t('Update to {latestVersion}', { latestVersion: up.latestVersion })}</Button>
              </div>
            {/if}
            <div class="card__foot">
              {#if p.enabled}
                <StatusTag state="ready">{t('In use')}</StatusTag>
              {:else}
                <Button size="sm" disabled={locked || saving} onclick={() => use(p.fileName)}>{t('Use this pack')}</Button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
      {#if !packs.length}
        <p class="text-body muted">{t('No shader packs yet.')}</p>
      {/if}
    {/if}
  </section>
</div>

<aside class="ip-aside" aria-label={t('Shader pack details')}>
  <Inspector class="ip-aside__inner" danger={shown ? { label: t('Delete'), hint: t('Moves the pack to the instance’s trash.'), disabled: locked, onclick: () => (deleting = shown) } : null}>
    {#if shown}
      {#snippet shaderMedia()}
        <span class="cb-ins-media" style:opacity={shown.enabled ? undefined : 0.6}>
          {#if shownSource?.iconUrl}<InstanceIcon src={shownSource.iconUrl} size={52} />{:else}<InstanceIcon {...projectArt(shownSource?.slug ?? shown.name)} size={52} />{/if}
        </span>
      {/snippet}
      {#snippet shaderMeta()}
        {#if shownSource?.versionNumber}<MetaChip>{shownSource.versionNumber}</MetaChip>{/if}
        {#if shown.enabled}<InspectorStatus tone="ok">{t('In use')}</InspectorStatus>{:else}<InspectorStatus tone="off">{t('Not in use')}</InspectorStatus>{/if}
        {#if shownUpdate}<InspectorStatus tone="update">{t('Update available')}</InspectorStatus>{/if}
      {/snippet}
      <InspectorHead eyebrow={t('Shader pack')} title={shownSource?.title ?? shown.name} sub={shownSource ? null : bytes(shown.size)} media={shaderMedia} meta={shaderMeta} />
      {#if requirement && !requirement.ok}
        {@const fix = requirement.fix}
        {#snippet fixAction()}<Button size="sm" icon={fix.icon} onclick={fix.run}>{fix.label}</Button>{/snippet}
        <InspectorNotice action={fixAction}><strong>{requirement.title}.</strong> {requirement.text}</InspectorNotice>
      {/if}
      <div class="cb-ins-acts">
        {#if shownUpdate}
          <Button variant="primary" class="wide" icon="update" busy={updating} disabled={updating || locked} onclick={() => data.applyUpdates([shownUpdate.fileName])}>{t('Update to {latestVersion}', { latestVersion: shownUpdate.latestVersion })}</Button>
        {/if}
        {#if shown.enabled}
          <Button icon="close" disabled={locked || saving} onclick={() => use(null)}>{t('Use none')}</Button>
        {:else}
          <Button variant={shownUpdate ? 'secondary' : 'primary'} class={shownUpdate ? undefined : 'wide'} icon="check" disabled={locked || saving} onclick={() => use(shown.fileName)}>{t('Use this pack')}</Button>
        {/if}
        {#if shownSource}<Button icon="globe" onclick={() => shownSource && data.project(shownSource.slug, 'shader')}>{t('Open page')}</Button>{/if}
      </div>
      <InspectorSection title={t('Settings')}>
        <p class="cb-ins-hint">{t('Each pack has its own settings, like shadows and fog. Change them in game under Options › Video settings › Shader packs › Shader pack settings.')}</p>
      </InspectorSection>
      <InspectorSection title={t('Details')}>
        <dl class="cb-ins-facts">
          <dt>{t('Source')}</dt><dd><span>{shownSource ? 'Modrinth' : t('Not found on Modrinth')}</span></dd>
          {#if shown.modified}<dt>{t('Changed')}</dt><dd><span>{relativeTime(shown.modified)}</span></dd>{/if}
          <dt>{t('Size')}</dt><dd><span>{bytes(shown.size)}</span></dd>
          <InspectorPath path={shown.fileName} openLabel={t('Open folder')} onopen={() => data.openFolder('shaderpacks')} />
        </dl>
      </InspectorSection>
    {:else}
      {#snippet noneMedia()}<span class="cb-ins-media cb-ins-media--empty"><Icon name="cube" size={27} /></span>{/snippet}
      {#snippet noneMeta()}{#if !current}<InspectorStatus tone="ok">{t('In use')}</InspectorStatus>{:else}<InspectorStatus tone="off">{t('Not in use')}</InspectorStatus>{/if}{/snippet}
      <InspectorHead eyebrow={t('Shader pack')} title={t('No shaders')} sub={t('The game’s normal look')} media={noneMedia} meta={noneMeta} />
      <p class="cb-ins-hint">{t('The game uses its normal look and runs at its fastest. Pick a pack to use it the next time you play.')}</p>
      <div class="cb-ins-acts">
        {#if current}
          <Button variant="primary" icon="close" disabled={locked || saving} onclick={() => use(null)}>{t('Use no shaders')}</Button>
        {:else}
          <Button variant="primary" icon="search" onclick={() => data.discover('shader')}>{t('Find shader packs')}</Button>
        {/if}
      </div>
    {/if}
  </Inspector>
</aside>

{#snippet deleteActions()}
  <Button variant="ghost" onclick={() => (deleting = null)}>{t('Cancel')}</Button>
  <Button variant="danger" icon="trash" busy={removing} disabled={removing} onclick={remove}>{t('Move to trash')}</Button>
{/snippet}

{#if deleting}
  <Dialog title={t('Move {name} to the trash?', { name: data.source(deleting.fileName)?.title ?? deleting.name })} icon="trash" tone="warning" onClose={() => (deleting = null)} actions={deleteActions}>
    <p>{deleting.enabled ? t('It is the shader pack in use: the game goes back to its normal look.') : t('The file moves to the trash folder of this instance.')}</p>
  </Dialog>
{/if}

<style>
  p,
  h2,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .ok {
    color: var(--grass-text);
  }
  .warn {
    color: var(--gold-text);
  }
  .info {
    color: var(--diamond-text);
  }
  .icon {
    display: inline-flex;
    flex: none;
  }
  .wrap {
    overflow-wrap: anywhere;
  }
  .link {
    padding: 0;
    color: var(--grass-text);
    background: none;
    border: 0;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }
  .link:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .req {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .req__items {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px var(--space-5);
  }
  .req__item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--ink);
  }
  .req__item.info {
    color: var(--diamond-text);
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .toolbar__title {
    flex: 1 1 220px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--space-4);
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .card.is-on::after {
    content: '';
    position: absolute;
    inset: -2px;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
  }
  .card__head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .card__title {
    min-width: 0;
  }
  .card__hit {
    all: unset;
    cursor: pointer;
  }
  .card__hit::after {
    content: '';
    position: absolute;
    inset: 0;
  }
  .card__hit:focus-visible::after {
    box-shadow: var(--focus-ring);
  }
  .card:hover {
    background: var(--surface-hover);
  }
  .card.is-shown:not(.is-on) {
    border-color: var(--ink);
  }
  .card__foot,
  .card__update {
    position: relative;
    z-index: 1;
  }
  .card__foot {
    display: flex;
    margin-top: auto;
  }
  .card__update {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
</style>
