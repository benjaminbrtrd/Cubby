<!-- Switch an instance installed from a modpack to another version of that pack: go back to an older one, or
     move to a newer one. The backend replaces the pack's mods and configs; worlds and the player's own files stay. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon } from '../../cubby'
  import { relativeTime } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, PackVersion, ProviderId } from '../../types'
  import Picker from '../discover/Picker.svelte'
  import { CHANNEL, loaderLabel } from '../discover/discover'

  let { instance, onClose }: { instance: Instance; onClose: () => void } = $props()

  const pack = $derived(instance.modpack!)
  const provider = $derived((pack.provider ?? 'modrinth') as ProviderId)

  let versions = $state<PackVersion[] | null>(null)
  let error = $state<string | null>(null)
  $effect(() => {
    const id = pack.projectId
    const p = provider
    let stale = false
    api
      .packVersions(p, id)
      .then((list) => {
        if (!stale) versions = list
      })
      .catch((e) => {
        if (!stale) error = typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
      })
    return () => {
      stale = true
    }
  })

  const current = $derived(versions?.find((v) => v.id === pack.versionId) ?? null)
  const currentIndex = $derived(versions ? versions.findIndex((v) => v.id === pack.versionId) : -1)
  let versionId = $state<string | null>(null)
  /** By default the version just before the installed one: the usual reason to come here is to go back. */
  const chosen = $derived(
    versions?.find((v) => v.id === versionId) ?? (versions && currentIndex >= 0 ? (versions[currentIndex + 1] ?? versions[0]) : versions?.[0]) ?? null,
  )
  const older = $derived(!!versions && !!chosen && currentIndex >= 0 && versions.indexOf(chosen) > currentIndex)
  const items = $derived((versions ?? []).map((v) => ({ id: v.id, v, group: undefined })))

  const running = $derived(!!app.running[instance.id])
  const busy = $derived(!!app.instanceTask(instance.id))
  let starting = $state(false)

  function meta(v: PackVersion): string {
    return [CHANNEL[v.channel].word, v.minecraft, v.loader ? loaderLabel(v.loader) : null].filter(Boolean).join(' · ')
  }

  async function change() {
    if (!chosen) return
    starting = true
    const name = chosen.name
    const done = api.changePackVersion(instance.id, chosen.id)
    onClose()
    try {
      await done
      app.toast(t('{name} now runs {pack} {version}.', { name: instance.name, pack: pack.title, version: name }), 'success')
    } catch (e) {
      app.error(e)
    } finally {
      starting = false
    }
  }
</script>

{#snippet actions()}
  <Button onclick={onClose}>{t('Cancel')}</Button>
  <Button variant="primary" icon={older ? 'clock' : 'update'} disabled={!chosen || chosen.id === pack.versionId || running || busy || starting} busy={starting} onclick={change}>
    {chosen && chosen.id !== pack.versionId ? t('Switch to {version}', { version: chosen.name }) : t('Switch version')}
  </Button>
{/snippet}

<Dialog title={t('Versions of {title}', { title: pack.title })} onClose={onClose} {actions}>
  <div class="body">
    <p class="text-body">
      {t('Installed:')} <span class="text-mono">{pack.versionNumber}</span>{#if current}<span class="muted"> · {meta(current)}</span>{/if}
    </p>
    {#if error}
      <Banner tone="danger" title={t('Couldn’t load the versions')}>{error}</Banner>
    {:else if !versions}
      <p class="text-small muted">{t('Loading versions…')}</p>
    {:else if chosen}
      <div class="field">
        <span class="text-label muted">{t('Version')}</span>
        <Picker {items} value={chosen.id} onSelect={(id) => (versionId = id)} label={t('Version')} listWidth={380}>
          {#snippet trigger()}
            <span class="stack">
              <span class="text-body-strong">{chosen.name}{#if chosen.id === pack.versionId} · {t('installed')}{/if}</span>
              <span class="text-small muted"><span class="text-mono">{meta(chosen)}</span></span>
            </span>
          {/snippet}
          {#snippet option(item)}
            <span class="stack">
              <span class="text-body-strong">{item.v.name}{#if item.v.id === pack.versionId} · {t('installed')}{/if}</span>
              <span class="text-small muted"><span class="text-mono">{meta(item.v)}</span>{#if item.v.released} · {relativeTime(item.v.released, '')}{/if}</span>
            </span>
          {/snippet}
        </Picker>
      </div>
      <ul class="notes text-small">
        <li><Icon name="check" size={18} /><span>{t('Your worlds, screenshots, servers and the mods you added yourself stay.')}</span></li>
        <li><Icon name="update" size={18} /><span>{t('The pack’s mods and settings are replaced by the ones of this version. Mods it no longer has go to the instance’s trash folder.')}</span></li>
        {#if chosen.minecraft && chosen.minecraft !== instance.minecraft}
          <li class="warn"><Icon name="warning" size={18} /><span>{t('This version runs Minecraft {mc}. Back up your worlds first: a world opened in a newer version may not open in an older one.', { mc: chosen.minecraft })}</span></li>
        {/if}
      </ul>
      {#if running}<p class="text-small muted">{t('Close the game first.')}</p>{/if}
    {/if}
  </div>
</Dialog>

<style>
  p,
  ul {
    margin: 0;
  }
  .body,
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .field {
    gap: var(--space-1);
  }
  .stack {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .notes {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0;
    list-style: none;
    color: var(--ink-muted);
  }
  .notes li {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .notes li :global(.cb-icon) {
    flex: none;
    margin-top: 1px;
  }
  .notes .warn {
    color: var(--gold-text);
  }
</style>
