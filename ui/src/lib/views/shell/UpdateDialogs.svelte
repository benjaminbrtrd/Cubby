<!-- The dialogs of an update of Cubby: what's new, restart, game still running, and "up to date" after the restart. -->
<script lang="ts">
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { appVersion } from '../../format'
  import { Button, Dialog, StatusTag } from '../../cubby'
  import Markdown from '../discover/Markdown.svelte'

  const RELEASES = 'https://github.com/benjaminbrtrd/Cubby/releases/tag/v'

  const kind = $derived(app.updateDialog)
  const info = $derived(app.update.info)
  const ready = $derived(app.update.phase === 'ready')
  const name = $derived(info ? `Cubby ${appVersion(info.version)}` : 'Cubby')
  const close = () => (app.updateDialog = null)
  const game = $derived(Object.keys(app.running).map((id) => app.instance(id)?.name)[0])

  function openNotes(version: string) {
    api.openUrl(RELEASES + version).catch((e) => app.error(e))
  }

  async function download() {
    close()
    await app.downloadUpdate()
  }
</script>

{#if kind === 'notes' && info}
  {#snippet notesActions()}
    <Button variant="ghost" onclick={() => app.skipUpdate()}>Skip this version</Button>
    <Button onclick={close}>Later</Button>
    {#if ready}
      <Button variant="primary" icon="update" onclick={() => (app.updateDialog = 'restart')}>Restart and update</Button>
    {:else}
      <Button variant="primary" icon="download" onclick={download}>Download update</Button>
    {/if}
  {/snippet}
  <Dialog wide tone="info" icon="download" title={name} onClose={close} actions={notesActions}>
    <p class="text-small">
      You have <span class="text-mono">{appVersion(app.info?.version ?? info.currentVersion)}</span>. New: <span class="text-mono">{appVersion(info.version)}</span>
      {#if app.settings?.updates.channel === 'preview' && info.version.includes('-')}<StatusTag state="update">Preview</StatusTag>{/if}
    </p>
    {#if info.notes.trim()}
      <Markdown source={info.notes} level={3} />
    {:else}
      <p>No release notes for this version.</p>
    {/if}
    <p class="text-small">The notes come from the release page on GitHub. Your instances, worlds and accounts are not touched by the update.</p>
  </Dialog>
{:else if kind === 'restart'}
  {#snippet restartActions()}
    <Button variant="ghost" onclick={close}>Not now</Button>
    <Button variant="primary" icon="update" onclick={() => app.installUpdate()}>Restart and update</Button>
  {/snippet}
  <Dialog tone="success" icon="update" title="Restart Cubby to update?" onClose={close} actions={restartActions}>
    <p>Cubby closes, installs {name} and opens again. It takes about ten seconds.</p>
    <p>Your instances, worlds and accounts stay as they are.</p>
  </Dialog>
{:else if kind === 'playing'}
  {#snippet playingActions()}<Button variant="primary" onclick={close}>Update after I play</Button>{/snippet}
  <Dialog tone="warning" icon="clock" title="Minecraft is still running" onClose={close} actions={playingActions}>
    <p>{game ? `${game} is open. ` : ''}Updating now would close it and could lose your progress.</p>
    <p>Cubby installs the update as soon as the game and Cubby are closed.</p>
  </Dialog>
{:else if kind === 'updated'}
  {#snippet updatedActions()}
    <Button variant="ghost" onclick={() => app.info && openNotes(app.info.version)}>See the notes</Button>
    <Button variant="primary" onclick={close}>Got it</Button>
  {/snippet}
  <Dialog tone="success" icon="check" title="Cubby is up to date" onClose={close} actions={updatedActions}>
    <p>You’re now on <span class="text-mono">{appVersion(app.info?.version ?? '')}</span>{app.updatedFrom ? ` (it was ${appVersion(app.updatedFrom)})` : ''}.</p>
  </Dialog>
{/if}
