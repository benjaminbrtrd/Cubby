<!--
  The strip for an update of Cubby, right under the title bar and across the whole window, like the chrome
  around it: available, downloading, ready (or waiting for a game to close) and failed. The title bar chip
  (see App.svelte) shows the same state.
-->
<script lang="ts">
  import { app } from '../../state.svelte'
  import { bytes } from '../../format'
  import { Button, Icon, IconButton } from '../../cubby'
  import type { IconName } from '../../cubby/data'

  const u = $derived(app.update)
  const version = $derived(u.info ? `Cubby ${u.info.version}` : 'Cubby')
  const task = $derived(app.tasks['self-update'])
  const percent = $derived(task && task.total ? Math.round((task.done / task.total) * 100) : 0)
  const playing = $derived(Object.keys(app.running).length > 0)

  const look = $derived.by((): { tone: 'info' | 'success' | 'warning'; icon: IconName; title: string; text: string } | null => {
    if (!u.info) return null
    switch (u.phase) {
      case 'available':
        return { tone: 'info', icon: 'download', title: `${version} is available`, text: `You’re on ${u.info.currentVersion}. Your instances and worlds stay as they are.` }
      case 'downloading':
        return { tone: 'info', icon: 'download', title: `Downloading ${version}`, text: 'Keep using Cubby. It asks before restarting.' }
      case 'ready':
        return playing
          ? { tone: 'warning', icon: 'clock', title: 'The update waits for your game', text: `${version} installs as soon as you close Minecraft and Cubby.` }
          : { tone: 'success', icon: 'check', title: `${version} is ready`, text: 'It installs when you close Cubby, or right now with a restart.' }
      case 'failed':
        return { tone: 'warning', icon: 'warning', title: 'Couldn’t download the update', text: `${u.error ?? ''} You’re still on ${u.info.currentVersion}.` }
      default:
        return null
    }
  })
</script>

{#if look && !u.dismissed}
  <div class="strip strip--{look.tone}" role="status">
    <span class="strip__slot" aria-hidden="true"><Icon name={look.icon} size={22} /></span>
    <p class="strip__title text-body-strong">{look.title}</p>
    {#if u.phase === 'downloading'}
      <span class="bar" role="progressbar" aria-valuenow={percent} aria-valuemin={0} aria-valuemax={100} aria-label="Download progress"><i style:width="{percent}%"></i></span>
      <span class="strip__text text-small muted">{task && task.total ? `${bytes(task.done)} of ${bytes(task.total)}` : 'Starting…'}</span>
    {:else}
      <p class="strip__text text-small muted" title={look.text}>{look.text}</p>
    {/if}
    <div class="strip__actions">
      {#if u.phase === 'available'}
        <Button size="sm" variant="ghost" onclick={() => (app.updateDialog = 'notes')}>What’s new</Button>
        <Button size="sm" variant="primary" icon="download" onclick={() => app.downloadUpdate()}>Download</Button>
      {:else if u.phase === 'ready' && playing}
        <Button size="sm" variant="ghost" onclick={() => (app.updateDialog = 'notes')}>What’s new</Button>
      {:else if u.phase === 'ready'}
        <Button size="sm" variant="ghost" onclick={() => (app.updateDialog = 'notes')}>What’s new</Button>
        <Button size="sm" variant="primary" icon="update" onclick={() => (app.updateDialog = 'restart')}>Restart and update</Button>
      {:else if u.phase === 'failed'}
        <Button size="sm" icon="update" onclick={() => app.downloadUpdate()}>Try again</Button>
      {/if}
      {#if u.phase !== 'downloading'}
        <IconButton icon="close" size="sm" label={u.phase === 'ready' ? 'Later' : 'Not now'} onclick={() => (app.update.dismissed = true)} />
      {/if}
    </div>
  </div>
{/if}

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 44px;
    padding-inline-end: var(--space-3);
    background: var(--surface);
    border-bottom: var(--border-chrome) solid var(--edge);
  }
  .strip p {
    margin: 0;
  }
  .strip__slot {
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 52px;
    border-inline-end: var(--border-block) solid var(--edge);
  }
  .strip--info .strip__slot {
    color: var(--on-diamond);
    background: var(--diamond);
  }
  .strip--success .strip__slot {
    color: var(--on-grass);
    background: var(--grass);
  }
  .strip--warning .strip__slot {
    color: var(--on-gold);
    background: var(--gold);
  }
  .strip__title {
    flex: none;
  }
  .strip__text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .strip__actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: none;
    margin-inline-start: auto;
  }
  .muted {
    color: var(--ink-muted);
  }
  /* Download progress: filled blocks, like the progress bars elsewhere */
  .bar {
    flex: 0 1 220px;
    min-width: 80px;
    height: 14px;
    padding: 2px;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
  }
  .bar i {
    display: block;
    height: 100%;
    background: repeating-linear-gradient(90deg, var(--diamond) 0 10px, transparent 10px 12px);
  }
</style>
