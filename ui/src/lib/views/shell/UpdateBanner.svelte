<!--
  The banner for an update of Cubby, at the top of the content area: available, downloading, ready
  (or waiting for a game to close) and failed. The title bar chip (see App.svelte) shows the same state.
-->
<script lang="ts">
  import { app } from '../../state.svelte'
  import { appVersion, bytes } from '../../format'
  import { Banner, Button, ProgressBar } from '../../cubby'

  const u = $derived(app.update)
  const version = $derived(u.info ? `Cubby ${appVersion(u.info.version)}` : 'Cubby')
  const task = $derived(app.tasks['self-update'])
  const percent = $derived(task && task.total ? Math.round((task.done / task.total) * 100) : null)
  const playing = $derived(Object.keys(app.running).length > 0)
</script>

{#snippet notes()}<Button variant="ghost" onclick={() => (app.updateDialog = 'notes')}>What’s new</Button>{/snippet}

{#snippet availableActions()}
  {@render notes()}
  <Button variant="primary" icon="download" onclick={() => app.downloadUpdate()}>Download update</Button>
{/snippet}

{#snippet readyActions()}
  {@render notes()}
  {#if !playing}
    <Button variant="ghost" onclick={() => (app.update.dismissed = true)}>Later</Button>
    <Button variant="primary" icon="update" onclick={() => (app.updateDialog = 'restart')}>Restart and update</Button>
  {/if}
{/snippet}

{#snippet retryActions()}
  <Button icon="update" onclick={() => app.downloadUpdate()}>Try again</Button>
{/snippet}

{#if u.info && !u.dismissed}
  {#if u.phase === 'available'}
    <Banner tone="info" icon="download" title="{version} is available" actions={availableActions} onDismiss={() => (app.update.dismissed = true)} dismissLabel="Not now">
      You’re on {appVersion(u.info.currentVersion)}. Your instances, worlds and accounts stay exactly as they are.
    </Banner>
  {:else if u.phase === 'downloading'}
    <Banner tone="info" icon="download" title="Downloading {version}">
      <ProgressBar
        value={percent ?? undefined}
        detail={task && task.total ? `${bytes(task.done)} of ${bytes(task.total)}` : 'Starting…'}
        tone="diamond"
      />
      <p class="text-small">Keep using Cubby. It asks before restarting.</p>
    </Banner>
  {:else if u.phase === 'ready' && playing}
    <Banner tone="warning" icon="clock" title="The update waits for your game" actions={notes}>
      {version} is downloaded. It installs as soon as you close Minecraft and Cubby.
    </Banner>
  {:else if u.phase === 'ready'}
    <Banner tone="success" icon="check" title="{version} is ready to install" actions={readyActions}>
      Cubby closes, installs it and opens again in about ten seconds. If you do nothing, it installs when you close Cubby.
    </Banner>
  {:else if u.phase === 'failed'}
    <Banner tone="warning" title="Couldn’t download the update" actions={retryActions} onDismiss={() => (app.update.dismissed = true)}>
      {u.error} You’re still on {appVersion(u.info.currentVersion)} and nothing changed.
    </Banner>
  {/if}
{/if}
