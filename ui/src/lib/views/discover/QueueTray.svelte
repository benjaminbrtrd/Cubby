<!-- The tray at the bottom of Discover. What you added with "+" waits here until you press "Install all":
     then everything downloads together, and the tray shows the progress of the whole batch. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { Button, IconButton, InstanceIcon, ProgressBar } from '../../cubby'
  import { bytes } from '../../format'
  import { iconOf, projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import { queue } from '../../queue.svelte'

  interface Props {
    instanceId: string
  }
  let { instanceId }: Props = $props()

  let open = $state(false)

  const inst = $derived(app.instance(instanceId))
  const mine = $derived(queue.forInstance(instanceId))
  const staged = $derived(mine.filter((i) => i.state === 'staged'))
  const running = $derived(mine.filter((i) => i.state === 'running'))
  const failed = $derived(mine.filter((i) => i.state === 'failed'))
  const done = $derived(mine.filter((i) => i.state === 'done'))
  const size = $derived(staged.reduce((s, i) => s + i.bytes, 0))
  const extra = $derived(staged.filter((i) => i.neededBy).length)
  const task = $derived(app.taskFor(`install:${instanceId}`))
  const pct = $derived(task?.total ? (task.done / task.total) * 100 : undefined)
</script>

{#if mine.length}
  <section class="tray" aria-label={t('Download queue')}>
    {#if open && staged.length}
      <ul class="list">
        {#each staged as i (i.key)}
          <li class="item">
            <InstanceIcon {...(i.iconUrl ? { src: i.iconUrl } : projectArt(i.title))} size={24} />
            <span class="text-body-strong name">{i.title}</span>
            {#if i.neededBy}<span class="text-small muted">{t('needed by {name}', { name: i.neededBy })}</span>{/if}
            <span class="grow"></span>
            {#if i.bytes}<span class="text-small muted text-counter">{bytes(i.bytes)}</span>{/if}
            {#if !i.neededBy}<IconButton icon="close" size="sm" label={t('Remove {title} from the queue', { title: i.title })} onclick={() => queue.remove(i.key)} />{/if}
          </li>
        {/each}
      </ul>
    {/if}

    <div class="bar">
      {#if inst}<InstanceIcon {...iconOf(inst)} seed={inst.name} size={32} raised />{/if}

      {#if running.length}
        <div class="sum grow">
          <ProgressBar tone="diamond" value={pct} label={t('Installing {n} items together', { n: running.length })} valueText={pct != null ? `${Math.round(pct)}%` : undefined} />
        </div>
        <Button size="sm" icon="download" onclick={() => app.navigate({ name: 'downloads' })}>{t('See downloads')}</Button>
      {:else if staged.length}
        <div class="sum">
          <span class="text-body-strong">{t('{n} ready to install', { n: staged.length })}{#if extra}{' '}<span class="muted">{t('({n} added because they are needed)', { n: extra })}</span>{/if}</span>
          <span class="text-small muted">{t('into {name}{size}, downloaded together', { name: inst?.name ?? '', size: size ? ` · ${bytes(size)}` : '' })}</span>
        </div>
        <Button size="sm" variant="ghost" iconEnd={open ? 'chevron-down' : 'chevron-right'} onclick={() => (open = !open)}>{open ? t('Hide') : t('Review')}</Button>
        <Button size="sm" variant="ghost" onclick={() => queue.clearStaged(instanceId)}>{t('Empty')}</Button>
        <Button variant="primary" icon="download" onclick={() => ((open = false), void queue.startAll(instanceId))}>{t('Install all')}</Button>
      {:else if failed.length}
        <div class="sum grow">
          <span class="text-body-strong">{t('{n} could not be installed', { n: failed.length })}</span>
          <span class="text-small muted">{failed[0].error}</span>
        </div>
        <Button size="sm" icon="update" onclick={() => void queue.startAll(instanceId)}>{t('Retry')}</Button>
        <Button size="sm" variant="ghost" onclick={() => queue.clearFinished(instanceId)}>{t('Dismiss')}</Button>
      {:else}
        <div class="sum grow">
          <span class="text-body-strong">{t('{n} installed', { n: done.length })}</span>
        </div>
        <Button size="sm" variant="ghost" icon="close" onclick={() => queue.clearFinished(instanceId)}>{t('Dismiss')}</Button>
      {/if}
    </div>
  </section>
{/if}

<style>
  .tray {
    position: sticky;
    bottom: var(--space-4);
    z-index: 5;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-float);
    margin-top: var(--space-4);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
  }
  .sum {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: var(--space-2) var(--space-4);
    max-height: 200px;
    overflow: auto;
    border-bottom: var(--border-block) solid var(--edge);
    background: var(--field);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--size-control-sm);
  }
</style>
