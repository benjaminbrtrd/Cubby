<!-- Changelog tab: what changed in each version, newest first. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Banner, Button, MetaChip, Switch } from '../../../cubby'
  import { date } from '../../../format'
  import type { Instance, ProjectVersion } from '../../../types'
  import Markdown from '../Markdown.svelte'
  import { CHANNEL, mcRange } from '../discover'

  let {
    target,
    versions,
    versionsError,
    listed,
    onlyFits = $bindable(),
  }: {
    target: Instance | undefined
    versions: ProjectVersion[] | null
    versionsError: string | null
    listed: ProjectVersion[]
    onlyFits: boolean
  } = $props()

  let limit = $state(8)
  const filtering = $derived(onlyFits && !!target)
  const newestRelease = $derived(versions?.find((v) => v.channel === 'release'))
</script>

<div id="pp-changelog" role="tabpanel" aria-label={t('Changelog')} class="panel">
  <div class="panel-head">
    <p class="text-small muted">{t('What changed in each version, newest first.')}</p>
    <span class="grow"></span>
    {#if target}
      <Switch checked={onlyFits} onChange={(v) => ((onlyFits = v), (limit = 8))} label={t('Only for {name}', { name: target.name })} />
    {/if}
  </div>
  {#if versionsError}
    <Banner tone="warning" title={t('Couldn\'t load the changelog')}>{versionsError}</Banner>
  {:else if !versions}
    <div class="table-skeleton" aria-hidden="true">{#each { length: 3 }, i (i)}<span class="sk tall"></span>{/each}</div>
  {:else if !listed.length}
    <div class="empty">
      <p class="text-body">{filtering && target ? t('No version for {name} yet.', { name: target.name }) : t('No versions yet.')}</p>
      {#if filtering}<Button size="sm" onclick={() => (onlyFits = false)}>{t('Show all versions')}</Button>{/if}
    </div>
  {:else}
    <ol class="changes">
      {#each listed.slice(0, limit) as v (v.id)}
        <li>
          <div class="change-head">
            <h3 class="text-body-strong"><span class="text-mono">{v.versionNumber}</span></h3>
            <MetaChip icon={CHANNEL[v.channel].icon} tone={CHANNEL[v.channel].tone}>{CHANNEL[v.channel].word}</MetaChip>
            <span class="text-small muted">{t('{published} · Minecraft', { published: date(v.published) })} <span class="text-mono">{mcRange(v.gameVersions)}</span></span>
            {#if v.id === newestRelease?.id}<span class="text-small good">{t('Latest release')}</span>{/if}
          </div>
          {#if v.changelog.trim()}
            <Markdown source={v.changelog} level={4} />
          {:else}
            <p class="text-small muted">{t('No notes for this version.')}</p>
          {/if}
        </li>
      {/each}
    </ol>
    {#if listed.length > limit}
      <Button size="sm" onclick={() => (limit += 8)}>{t('Show older versions')}</Button>
    {/if}
  {/if}
</div>

<style>
  .changes {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: 0;
    list-style: none;
  }
  .changes > li {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .change-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-3);
  }
</style>
