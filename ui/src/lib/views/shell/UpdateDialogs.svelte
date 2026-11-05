<!-- The dialogs of an update of Cubby: what's new, restart, game still running, and "up to date" after the restart. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { Button, Dialog, MetaChip } from '../../cubby'
  import Markdown from '../discover/Markdown.svelte'

  const RELEASES = 'https://github.com/benjaminbrtrd/Cubby/releases/tag/v'

  const kind = $derived(app.updateDialog)
  const info = $derived(app.update.info)
  const ready = $derived(app.update.phase === 'ready')
  const name = $derived(info ? t('Cubby {version}', { version: info.version }) : 'Cubby')
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
    <Button variant="ghost" onclick={() => app.skipUpdate()}>{t('Skip this version')}</Button>
    <Button onclick={close}>{t('Later')}</Button>
    {#if ready}
      <Button variant="primary" icon="update" onclick={() => (app.updateDialog = 'restart')}>{t('Restart and update')}</Button>
    {:else}
      <Button variant="primary" icon="download" onclick={download}>{t('Download update')}</Button>
    {/if}
  {/snippet}
  <Dialog wide tone="info" icon="download" title={name} onClose={close} actions={notesActions}>
    <p class="text-small">
      {t('You have')} <span class="text-mono">{app.info?.version ?? info.currentVersion}</span>{t('. New:')} <span class="text-mono">{info.version}</span>
      {#if app.settings?.updates.channel === 'preview' && info.version.includes('-')}<MetaChip tone="snapshot">{t('Preview')}</MetaChip>{/if}
    </p>
    {#if info.notes.trim()}
      <Markdown source={info.notes} level={3} />
    {:else}
      <p>{t('No release notes for this version.')}</p>
    {/if}
    <p class="text-small notes-note">{t('The notes come from the release page on GitHub. Your instances, worlds and accounts are not touched by the update.')}</p>
  </Dialog>
{:else if kind === 'restart'}
  {#snippet restartActions()}
    <Button variant="ghost" onclick={close}>{t('Not now')}</Button>
    <Button variant="primary" icon="update" onclick={() => app.installUpdate()}>{t('Restart and update')}</Button>
  {/snippet}
  <Dialog tone="success" icon="update" title={t('Restart Cubby to update?')} onClose={close} actions={restartActions}>
    <p>{t('Cubby closes, installs {name} and opens again. It takes about ten seconds.', { name })}</p>
    <p>{t('Your instances, worlds and accounts stay as they are.')}</p>
  </Dialog>
{:else if kind === 'playing'}
  {#snippet playingActions()}<Button variant="primary" onclick={close}>{t('Update after I play')}</Button>{/snippet}
  <Dialog tone="warning" icon="clock" title={t('Minecraft is still running')} onClose={close} actions={playingActions}>
    <p>{game ? t('{game} is open. ', { game }) : ''}{t('Updating now would close it and could lose your progress.')}</p>
    <p>{t('Cubby installs the update as soon as the game and Cubby are closed.')}</p>
  </Dialog>
{:else if kind === 'updated'}
  {#snippet updatedActions()}
    <Button variant="ghost" onclick={() => app.info && openNotes(app.info.version)}>{t('See the notes')}</Button>
    <Button variant="primary" onclick={close}>{t('Got it')}</Button>
  {/snippet}
  <Dialog tone="success" icon="check" title={t('Cubby is up to date')} onClose={close} actions={updatedActions}>
    <p>{t('You’re now on')} <span class="text-mono">{app.info?.version ?? ''}</span>{app.updatedFrom ? t(' (it was {updatedFrom})', { updatedFrom: app.updatedFrom }) : ''}.</p>
  </Dialog>
{/if}

<style>
  .notes-note {
    margin-top: var(--space-3);
    color: var(--ink-muted);
  }
  p :global(.cb-chip) {
    margin-inline-start: var(--space-2);
  }
</style>
