<!-- Wizard: the instance as it stands, beside every step. Each picked project can be removed from here. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { IconButton, InstanceIcon, MetaChip, SegmentedControl } from '../../../cubby'
  import { loaderName } from '../../../format'
  import { projectArt } from '../../../icon'
  import { wizard as w, type PickKind, type StepId } from './state.svelte'

  let { onChange }: { onChange: (step: StepId) => void } = $props()

  const groups = $derived.by(() => {
    const pack = w.path === 'modpack'
    const g: { kind: PickKind; label: string; empty: string; note?: string }[] = [
      {
        kind: 'mod',
        label: pack ? t('Extra mods') : t('Mods'),
        empty: w.vanilla ? t('Vanilla runs no mods.') : pack ? t('Only the pack’s mods.') : t('No mods yet.'),
        note: w.picked('mod').length && (w.gameLoader === 'fabric' || w.gameLoader === 'quilt') ? t('Plus the mods they need, like Fabric API.') : undefined,
      },
      { kind: 'resourcepack', label: t('Resource packs'), empty: t('The game’s own textures.') },
      { kind: 'shader', label: t('Shader pack'), empty: t('No shaders.'), note: w.shaderNeedsIris ? t('Needs Iris: add it in Mods or Looks.') : undefined },
      { kind: 'datapack', label: t('Datapacks'), empty: t('Worlds start as the game makes them.') },
    ]
    return g
  })
  /** In the order they were added, or by name. */
  let sort = $state<'added' | 'name'>('added')
  const sorted = (kind: PickKind) => {
    const items = w.picked(kind)
    return sort === 'name' ? [...items].sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: 'base' })) : items
  }
  const many = $derived(groups.some((g) => w.picked(g.kind).length > 1))
  const step: Record<PickKind, StepId> = { mod: 'mods', resourcepack: 'looks', shader: 'looks', datapack: 'datapacks' }
</script>

<aside class="aside" aria-label={t('Your instance')}>
  <h2 class="cb-ins-eyebrow">{t('Your instance')}</h2>
  <div class="who">
    <InstanceIcon motif={w.motif} ground={w.ground} src={w.src ?? (w.path === 'modpack' && !w.iconTouched ? (w.pack?.iconUrl ?? undefined) : undefined)} size={52} raised label={t('Icon')} />
    <div class="who__txt">
      <span class="text-heading name">{w.shownName}</span>
      <span class="chips">
        {#if w.gameVersion}<MetaChip>{w.gameVersion}</MetaChip>{/if}
        {#if w.gameLoader}<MetaChip>{loaderName(w.gameLoader)}</MetaChip>{/if}
      </span>
    </div>
  </div>
  {#if w.path === 'modpack'}
    <p class="text-small muted">
      {#if w.pack}{t('From {pack}, which sets the version, the loader and its mods.', { pack: w.pack.name })}{:else}{t('Pick a modpack: it sets the version and the loader.')}{/if}
    </p>
  {/if}

  {#if many}
    <div class="sort">
      <span class="text-small muted">{t('Sort by')}</span>
      <SegmentedControl
        label={t('Sort by')}
        options={[
          { value: 'added', label: t('Order added') },
          { value: 'name', label: t('Name') },
        ]}
        value={sort}
        onChange={(v) => (sort = v === 'name' ? 'name' : 'added')}
      />
    </div>
  {/if}

  {#each groups as g (g.kind)}
    {@const items = sorted(g.kind)}
    <section class="sec" aria-label={g.label}>
      <div class="sec__head">
        <button type="button" class="sec__title text-label" onclick={() => onChange(step[g.kind])}>{g.label}</button>
        <MetaChip>{String(items.length)}</MetaChip>
      </div>
      {#if !items.length}
        <p class="text-small muted">{g.empty}</p>
      {:else}
        <ul>
          {#each items as p (p.source + p.projectId)}
            <li class="item">
              <InstanceIcon {...p.iconUrl ? { src: p.iconUrl } : projectArt(p.projectId)} size={20} />
              <span class="item__txt text-small">{p.title}</span>
              <IconButton icon="close" size="sm" variant="ghost" label={t('Remove {title}', { title: p.title })} onclick={() => w.remove(p)} />
            </li>
          {/each}
        </ul>
      {/if}
      {#if g.note}<p class="note">{g.note}</p>{/if}
    </section>
  {/each}

  <span class="grow"></span>
  <p class="cb-ins-hint">{t('Everything installs when you create the instance. Add or remove anything later from it.')}</p>
</aside>

<style>
  .aside {
    box-sizing: border-box;
    flex: 0 0 var(--size-inspector);
    width: var(--size-inspector);
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  h2,
  p,
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
  .grow {
    flex: 1;
  }
  .who {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .who__txt {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .sort {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .sort :global(.cb-seg__opt) {
    min-height: 24px;
    font-size: 13px;
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding-top: var(--space-3);
    border-top: var(--border-hair) solid var(--line);
  }
  .sec__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .sec__title {
    padding: 0;
    color: var(--ink-muted);
    background: none;
    border: 0;
    cursor: pointer;
  }
  .sec__title:hover {
    color: var(--ink);
  }
  .sec__title:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
  }
  .item > :global(.cb-instance-icon) {
    flex: none;
  }
  .item__txt {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .note {
    font-size: 12px;
    line-height: 16px;
    color: var(--ink-muted);
  }
</style>
