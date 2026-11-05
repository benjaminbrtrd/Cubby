<!-- Versions tab: every file of the project, the best one for the target marked, any one can be chosen. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Banner, Button, Icon, MetaChip, Switch } from '../../../cubby'
  import { bytes, compactNum, date, num } from '../../../format'
  import type { Instance, ProjectKind, ProjectVersion } from '../../../types'
  import { CHANNEL, loaderLabel, mcRange, modLoadersOf, primarySize } from '../discover'

  let {
    kind,
    target,
    versions,
    versionsError,
    listed,
    best,
    chosen,
    onlyFits = $bindable(),
    onRetry,
    onChoose,
  }: {
    kind: ProjectKind
    target: Instance | undefined
    versions: ProjectVersion[] | null
    versionsError: string | null
    /** The versions shown: all of them, or only those that fit the target. */
    listed: ProjectVersion[]
    best: ProjectVersion | undefined
    chosen: ProjectVersion | undefined
    onlyFits: boolean
    onRetry: () => void
    onChoose: (v: ProjectVersion) => void
  } = $props()

  let limit = $state(20)
  const filtering = $derived(onlyFits && !!target)

  const loaderColumn = $derived(kind === 'mod' ? 'Loaders' : kind === 'modpack' ? 'Loader' : kind === 'shader' ? t('Runs with') : null)
  const columns = $derived(loaderColumn ? 8 : 7)

  function loadersOf(v: ProjectVersion): string {
    const list = kind === 'mod' || kind === 'modpack' ? modLoadersOf(v.loaders) : v.loaders
    return list.map(loaderLabel).join(', ') || '—'
  }

  /** Why the best version is the best one, in words. */
  const bestWhy = $derived.by(() => {
    if (!best || !target) return ''
    const what = best.channel === 'release' ? t('Newest release') : t('Newest version')
    if (kind === 'mod' && target.loader.kind !== 'vanilla') return t('{what} for Minecraft {minecraft} and {kind}', { what, minecraft: target.minecraft, kind: loaderLabel(target.loader.kind) })
    return t('{what} for Minecraft {minecraft}', { what, minecraft: target.minecraft })
  })
</script>

<div id="pp-versions" role="tabpanel" aria-label={t('Versions')} class="panel">
  <div class="panel-head">
    <p class="text-small muted">
      {target ? t('Cubby picks the best one for {name}.', { name: target.name }) : t('Newest first.')} {t('Betas can have bugs.')}
    </p>
    <span class="grow"></span>
    {#if target}
      <Switch checked={onlyFits} onChange={(v) => ((onlyFits = v), (limit = 20))} label={t('Only for {name}', { name: target.name })} />
    {/if}
  </div>
  {#if versionsError}
    {#snippet retryVersions()}<Button size="sm" icon="update" onclick={onRetry}>{t('Try again')}</Button>{/snippet}
    <Banner tone="warning" title={t('Couldn\'t load the versions')} actions={retryVersions}>{versionsError}</Banner>
  {:else if !versions}
    <div class="table-skeleton" aria-hidden="true">{#each { length: 5 }, i (i)}<span class="sk"></span>{/each}</div>
  {:else if !listed.length}
    <div class="empty">
      <p class="text-body">{filtering && target ? t('No version for {name} ({minecraft}) yet.', { name: target.name, minecraft: target.minecraft }) : t('No versions yet.')}</p>
      {#if filtering}<Button size="sm" onclick={() => (onlyFits = false)}>{t('Show all versions')}</Button>{/if}
    </div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th scope="col" class="text-label">{t('Version')}</th>
            <th scope="col" class="text-label">Minecraft</th>
            {#if loaderColumn}<th scope="col" class="text-label">{loaderColumn}</th>{/if}
            <th scope="col" class="text-label">{t('Channel')}</th>
            <th scope="col" class="text-label">{t('Date')}</th>
            <th scope="col" class="text-label end">{t('Downloads')}</th>
            <th scope="col" class="text-label end">{t('Size')}</th>
            <th scope="col" class="text-label end"><span class="cb-visually-hidden">{t('Action')}</span></th>
          </tr>
        </thead>
        <tbody>
          {#each listed.slice(0, limit) as v (v.id)}
            {@const isBest = !!best && v.id === best.id && !!target}
            {@const isChosen = !!chosen && v.id === chosen.id}
            {#if isBest}
              <tr class="best-note">
                <td colspan={columns}>
                  <span class="text-body-strong best-word"><Icon name="check" size={18} />{t('Best for {name}', { name: target?.name })}</span>
                  <span class="text-small muted">{bestWhy}</span>
                </td>
              </tr>
            {/if}
            <tr class={[isBest && 'best', isChosen && 'chosen']}>
              <td class="text-mono">{v.versionNumber}</td>
              <td class="text-mono">{mcRange(v.gameVersions)}</td>
              {#if loaderColumn}<td class="text-mono">{loadersOf(v)}</td>{/if}
              <td><MetaChip icon={CHANNEL[v.channel].icon} tone={CHANNEL[v.channel].tone} title={CHANNEL[v.channel].hint}>{CHANNEL[v.channel].word}</MetaChip></td>
              <td class="text-body" title={date(v.published)}>{date(v.published)}</td>
              <td class="text-counter end">{compactNum(v.downloads)}</td>
              <td class="text-mono end">{bytes(primarySize(v))}</td>
              <td class="end">
                {#if isChosen}
                  <span class="text-small chosen-word"><Icon name="check" size={18} />{t('Chosen')}</span>
                {:else}
                  <Button size="sm" variant="ghost" onclick={() => onChoose(v)} aria-label={t('Choose version {versionNumber}', { versionNumber: v.versionNumber })}>{t('Choose')}</Button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if listed.length > limit}
      <Button size="sm" onclick={() => (limit += 20)}>{t('Show more ({n} left)', { n: num(listed.length - limit) })}</Button>
    {/if}
  {/if}
</div>

<style>
  .table-wrap {
    overflow-x: auto;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    padding: 10px var(--space-2);
    text-align: start;
    white-space: nowrap;
    color: var(--ink-muted);
    background: var(--bg-chrome);
    border-bottom: var(--border-block) solid var(--edge);
  }
  td {
    padding: var(--space-2);
    vertical-align: middle;
    border-bottom: var(--border-hair) solid var(--line);
  }
  th:first-child,
  td:first-child {
    padding-inline-start: var(--space-3);
  }
  th:last-child,
  td:last-child {
    padding-inline-end: var(--space-3);
  }
  .end {
    text-align: end;
  }
  tr.best,
  tr.best-note {
    background: var(--surface-hover);
  }
  tr.best td:first-child,
  tr.best-note td {
    border-inline-start: 4px solid var(--grass);
  }
  tr.best-note td {
    padding: 10px var(--space-3) 0;
    border-bottom: 0;
  }
  tr.best-note td > span {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  tr.best-note td > span + span {
    margin-inline-start: var(--space-2);
  }
  .best-word {
    color: var(--grass-text);
  }
  tr.chosen td {
    box-shadow:
      inset 0 2px 0 var(--ink),
      inset 0 -2px 0 var(--ink);
  }
  tr.chosen td:first-child {
    box-shadow:
      inset 2px 2px 0 var(--ink),
      inset 0 -2px 0 var(--ink);
  }
  tr.chosen td:last-child {
    box-shadow:
      inset -2px 2px 0 var(--ink),
      inset 0 -2px 0 var(--ink);
  }
  .chosen-word {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--grass-text);
  }
</style>
