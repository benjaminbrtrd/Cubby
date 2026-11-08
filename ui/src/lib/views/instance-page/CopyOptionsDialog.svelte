<!-- Copy the game's options, key bindings or server list from another instance into this one. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, Dialog, Switch } from '../../cubby'
  import { instanceMeta } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance } from '../../types'

  let { instance, onClose }: { instance: Instance; onClose: () => void } = $props()

  const others = $derived(app.recent.filter((i) => i.id !== instance.id))
  let from = $state(app.recent.find((i) => i.id !== instance.id)?.id ?? '')
  let options = $state(false)
  let keys = $state(true)
  let servers = $state(false)
  let busy = $state(false)

  const source = $derived(others.find((i) => i.id === from))
  const running = $derived(!!app.running[instance.id])

  async function copy() {
    if (!source) return
    busy = true
    try {
      const done = await api.copyOptions(instance.id, source.id, { options, keys, servers })
      const parts = [done.options && t('options'), done.keys && t('key bindings'), done.servers && t('servers')].filter(Boolean)
      if (parts.length) app.toast(t('Copied the {parts} of {name}.', { parts: parts.join(', '), name: source.name }), 'success')
      else app.toast(t('{name} has nothing to copy yet. Play it once first.', { name: source.name }), 'info')
      onClose()
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }
</script>

{#snippet actions()}
  <Button variant="ghost" onclick={onClose}>{t('Cancel')}</Button>
  <Button variant="primary" icon="copy" busy={busy} disabled={busy || running || !source || !(options || keys || servers)} onclick={copy}>{t('Copy')}</Button>
{/snippet}

<Dialog icon="copy" title={t('Copy settings into {name}', { name: instance.name })} {onClose} {actions}>
  {#if !others.length}
    <p>{t('There is no other instance to copy from yet.')}</p>
  {:else}
    <div class="form">
      <label class="field">
        <span class="text-body-strong">{t('From')}</span>
        <select bind:value={from} class="text-body">
          {#each others as i (i.id)}
            <option value={i.id}>{i.name} · {instanceMeta(i)}</option>
          {/each}
        </select>
      </label>
      <Switch bind:checked={keys} label={t('Key bindings')} hint={t('Your controls: jump, sprint, and the keys mods add.')} />
      <Switch bind:checked={options} label={t('Video, sound and other options')} hint={t('Render distance, FOV, volume, language... Resource packs stay as they are.')} />
      <Switch bind:checked={servers} label={t('Server list')} hint={t('Replaces the Multiplayer list of this instance.')} />
      {#if running}<p class="text-small muted">{t('Close the game first: it writes its options when it quits.')}</p>{/if}
    </div>
  {/if}
</Dialog>

<style>
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: min(460px, 80vw);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  select {
    min-height: var(--size-control);
    padding: 0 var(--space-2);
    color: var(--ink);
    background: var(--field);
    border: var(--border-block) solid var(--edge);
  }
  select:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
</style>
