<script lang="ts">
  import { api } from '../../api'
  import { Banner, Button, Icon, InstanceIcon, Kbd, StatusTag } from '../../cubby'
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
      return { ok: true, title: 'Shaders run through OptiFine', text: 'OptiFine loads the shader pack. It is installed and on.', needs: [{ name: 'OptiFine', mod: m.optifine }] }
    if (instance.loader.kind === 'vanilla')
      return {
        ok: false,
        title: 'Shaders need a mod loader',
        text: 'Shader packs run through Iris, a mod. This instance is Vanilla, which doesn’t load mods. Pick Fabric or Quilt in Settings, then add Iris and Sodium.',
        needs,
        fix: { label: 'Change loader', icon: 'wrench', run: () => app.openInstance(id, 'settings') },
      }
    if (!m.iris)
      return { ok: false, title: `Shaders need ${iris.name}`, text: `${iris.name} is the mod that loads shader packs. It isn’t installed in this instance.`, needs, fix: { label: `Find ${iris.name}`, icon: 'search', run: find(iris) } }
    if (!m.iris.enabled)
      return { ok: false, title: `${iris.name} is turned off`, text: `Turn ${iris.name} on in Mods to use shader packs.`, needs, fix: { label: 'Open Mods', icon: 'mods', run: () => app.openInstance(id, 'mods') } }
    if (!m.sodium)
      return { ok: false, title: `${iris.name} needs ${sodium.name}`, text: `${iris.name} uses ${sodium.name} to draw the world. Add ${sodium.name} to use shader packs.`, needs, fix: { label: `Find ${sodium.name}`, icon: 'search', run: find(sodium) } }
    if (!m.sodium.enabled)
      return { ok: false, title: `${sodium.name} is turned off`, text: `${iris.name} needs it. Turn ${sodium.name} on in Mods.`, needs, fix: { label: 'Open Mods', icon: 'mods', run: () => app.openInstance(id, 'mods') } }
    return { ok: true, title: `Shaders run through ${iris.name}`, text: `${iris.name} loads the shader pack, and ${sodium.name} keeps the game fast while it runs. Both are installed.`, needs }
  })

  const irisUpdate = $derived(data.shaderMods.iris ? data.update(data.shaderMods.iris.fileName) : undefined)
  const hasIris = $derived(!!data.shaderMods.iris?.enabled)

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

  const describe = (p: Pack) => `${p.isDir ? 'Folder' : 'Zip file'} · ${bytes(p.size)}`
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

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label="Shader packs">
  {#if requirement}
    <Banner tone={requirement.ok ? 'success' : 'warning'} title={requirement.title} actions={requirement.ok ? undefined : fixAction}>
      <div class="req">
        <p>{requirement.text}</p>
        <div class="req__items">
          {#each requirement.needs as n (n.name)}
            {#if n.mod?.enabled}
              <span class="req__item"><span class="ok icon"><Icon name="check" size={18} /></span>{n.name}{n.mod.version ? ` ${n.mod.version}` : ''} installed</span>
            {:else if n.mod}
              <span class="req__item"><span class="warn icon"><Icon name="minimize" size={18} /></span>{n.name} installed, turned off</span>
            {:else}
              <span class="req__item"><span class="warn icon"><Icon name="warning" size={18} /></span>{n.name} missing</span>
            {/if}
          {/each}
          {#if irisUpdate}
            <span class="req__item info"><Icon name="update" size={18} />{iris.name} {irisUpdate.latestVersion} available</span>
            <button type="button" class="link text-body-strong" onclick={() => app.openInstance(id, 'mods')}>Update in Mods</button>
          {/if}
        </div>
      </div>
    </Banner>
  {/if}

  {#if locked}
    <Banner tone="info" icon="play" title="The game is running">
      Close it to pick a shader pack here, or pick one in game under Options › Video settings › Shader packs.
    </Banner>
  {/if}

  <section class="stack" aria-labelledby="sh-title">
    <div class="toolbar">
      <div class="toolbar__title">
        <h2 id="sh-title" class="text-label">Shader packs</h2>
        <span class="text-small muted">Only one can be on at a time.</span>
      </div>
      <Button icon="folder" onclick={() => data.openFolder('shaderpacks')}>Open folder</Button>
      <Button variant="primary" icon="search" onclick={() => data.discover('shader')}>Find shader packs</Button>
    </div>

    {#if packs === null}
      <p class="muted text-body">Reading the shader packs folder…</p>
    {:else}
      <div class="grid">
        <div class={['card', !current && 'is-on']}>
          <div class="card__head">
            <InstanceIcon motif="block" ground="sky" size={32} />
            <div class="card__title">
              <p class="text-body-strong">None</p>
              <p class="text-small muted">Minecraft’s own look</p>
            </div>
          </div>
          <p class="text-small muted">No shader pack. The game looks as Mojang made it and runs at its fastest.</p>
          <div class="card__foot">
            {#if !current}
              <StatusTag state="ready">In use</StatusTag>
            {:else}
              <Button size="sm" disabled={locked || saving} onclick={() => use(null)}>Use no shaders</Button>
            {/if}
          </div>
        </div>
        {#each packs as p (p.fileName)}
          {@const src = data.source(p.fileName)}
          <div class={['card', p.enabled && 'is-on']}>
            <div class="card__head">
              {@render art(p)}
              <div class="card__title">
                <p class="text-body-strong wrap">{src?.title ?? p.name}</p>
                <p class="text-small muted">{src ? `From Modrinth · ${src.versionNumber}` : describe(p)}</p>
              </div>
            </div>
            <p class="text-mono muted wrap">{p.fileName}</p>
            {#if data.update(p.fileName)}
              {@const up = data.update(p.fileName)!}
              <div class="card__update">
                <StatusTag state="update" detail={up.latestVersion}>Update</StatusTag>
                <Button size="sm" icon="update" busy={updating} disabled={updating || locked} onclick={() => data.applyUpdates([up.fileName])}>Update to {up.latestVersion}</Button>
              </div>
            {/if}
            <div class="card__foot">
              {#if p.enabled}
                <StatusTag state="ready">In use</StatusTag>
              {:else}
                <Button size="sm" disabled={locked || saving} onclick={() => use(p.fileName)}>Use this pack</Button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
      {#if !packs.length}
        <p class="text-body muted">No shader packs yet. Find some on Modrinth, or drop .zip files in the folder.</p>
      {/if}
    {/if}
  </section>

  <div class="note">
    <span class="info icon"><Icon name="info" size={18} /></span>
    <p class="text-body">
      Shaders change how the game looks, never how it plays.{#if hasIris}&#32;Press <Kbd keys="K" /> in game to turn them on or off.{/if}
    </p>
  </div>
</div>

<aside class="ip-aside" aria-label="Shader pack in use">
  <div class="ip-aside__inner">
    <div class="aside__title">
      <p class="text-label muted">In use</p>
      <h2 class="text-heading wrap">{current ? (currentSource?.title ?? current.name) : 'No shader pack'}</h2>
    </div>
    {#if current}
      <dl class="facts">
        <dt class="text-small muted">Size</dt>
        <dd class="text-body">{bytes(current.size)}</dd>
        {#if current.modified}
          <dt class="text-small muted">Changed</dt>
          <dd class="text-body">{relativeTime(current.modified)}</dd>
        {/if}
        <dt class="text-small muted">Source</dt>
        <dd class="text-body">{currentSource ? 'Modrinth' : 'Not found on Modrinth'}</dd>
        {#if currentSource}
          <dt class="text-small muted">Version</dt>
          <dd class="text-mono">{currentSource.versionNumber}</dd>
        {/if}
        <dt class="text-small muted full">File</dt>
        <dd class="text-mono full wrap">{current.fileName}</dd>
      </dl>
      {#if currentSource}
        <div><Button size="sm" icon="globe" onclick={() => currentSource && data.project(currentSource.slug, 'shader')}>Open page</Button></div>
      {/if}
      <p class="text-small muted">Each pack has its own settings, like shadows and fog. Change them in game under Options › Video settings › Shader packs › Shader pack settings.</p>
    {:else}
      <p class="text-body">The game uses its normal look and runs at its fastest. Pick a pack to use it the next time you play.</p>
    {/if}
  </div>
</aside>

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
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
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
  .note {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .aside__title {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .facts {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--space-2) var(--space-4);
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .facts .full {
    grid-column: 1 / -1;
  }
</style>
