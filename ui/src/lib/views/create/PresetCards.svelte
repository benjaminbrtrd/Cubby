<!--
  The three one-click starts: the latest release, Fabric ready for performance mods, or a server.
  Each creates the instance, selects it in the library and launches it. Used by the empty library
  and by Quick start in New instance.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import { Button, InstanceIcon, MetaChip, TextField } from '../../cubby'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { t } from '../../i18n.svelte'
  import { addressProblem, discoverPerformanceMods, findNewest, hostOf, presetSpec, type Newest, type Preset } from './presets'

  let { align = 'center' }: { align?: 'center' | 'start' } = $props()

  let newest = $state<Newest | null>(null)
  let busy = $state<Preset | null>(null)
  let address = $state('')
  let addressError = $state('')

  onMount(() => {
    void findNewest().then((n) => (newest = n))
  })

  const release = $derived(newest?.release ?? null)
  const fabric = $derived(newest?.fabric ?? null)

  function fabricNote(): string {
    if (!newest) return t('Looking for the newest Fabric loader…')
    if (!release) return t('Cubby couldn’t reach Mojang’s servers.')
    if (newest.fabricFailed) return t('Cubby couldn’t reach Fabric’s servers.')
    if (!fabric) return t('Fabric isn’t ready for {version} yet.', { version: release })
    return t('The Fabric loader, ready for performance mods like Sodium. Cubby suggests them right after.')
  }

  async function start(preset: Preset) {
    if (busy || !release) return
    const addr = address.trim()
    if (preset === 'server') {
      addressError = addressProblem(addr)
      if (addressError) {
        document.getElementById('preset-server')?.focus()
        return
      }
    }
    busy = preset
    try {
      const created = await api.createInstance(presetSpec(preset, release, fabric, addr))
      if (preset === 'server') await api.saveServers(created.id, [{ name: hostOf(addr), address: addr, iconUrl: null, acceptTextures: null }])
      await app.refreshInstances()
      app.selected = created.id
      app.navigate({ name: 'library' })
      void app.launch(created.id, false, preset === 'server' ? addr : null)
      if (preset === 'fast') app.toast(t('Add performance mods like Sodium from Discover.'), 'info', { label: t('Open Discover'), run: () => discoverPerformanceMods(created.id) })
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }
</script>

<div class={['presets', align === 'start' && 'is-start']}>
  <div class="preset is-main">
    <div class="preset__head">
      <InstanceIcon motif="block" ground="moss" size={52} raised />
      <div class="preset__title">
        <h2 class="text-heading">{t('Latest release')}</h2>
        <div class="chips">
          {#if release}<MetaChip>{release}</MetaChip>{/if}
          <MetaChip>{t('Vanilla')}</MetaChip>
        </div>
      </div>
    </div>
    <p class="text-small muted grow">{newest && !release ? t('Cubby couldn’t reach Mojang’s servers. Check your connection.') : t('The game as Mojang made it, nothing added.')}</p>
    <Button variant="primary" icon="play" busy={busy === 'latest'} disabled={!release || !!busy} onclick={() => start('latest')}>{t('Create and play')}</Button>
  </div>

  <div class="preset">
    <div class="preset__head">
      <InstanceIcon motif="rocket" ground="sky" size={52} raised />
      <div class="preset__title">
        <h2 class="text-heading">{t('Smooth and fast')}</h2>
        <div class="chips">
          {#if release}<MetaChip>{release}</MetaChip>{/if}
          <MetaChip>{fabric ? `Fabric ${fabric}` : 'Fabric'}</MetaChip>
        </div>
      </div>
    </div>
    <p class="text-small muted grow">{fabricNote()}</p>
    <Button icon="play" busy={busy === 'fast'} disabled={!release || !fabric || !!busy} onclick={() => start('fast')}>{t('Create and play')}</Button>
  </div>

  <div class="preset">
    <div class="preset__head">
      <InstanceIcon motif="castle" ground="amethyst" size={52} raised />
      <div class="preset__title">
        <h2 class="text-heading">{t('Join a server')}</h2>
        <div class="chips">
          {#if release}<MetaChip>{release}</MetaChip>{/if}
          <MetaChip>{t('Vanilla')}</MetaChip>
        </div>
      </div>
    </div>
    <p class="text-small muted">{t('Paste an address. Cubby makes the instance and adds the server to it.')}</p>
    <form
      class="server"
      onsubmit={(e) => {
        e.preventDefault()
        void start('server')
      }}
    >
      <TextField
        id="preset-server"
        label={t('Server address')}
        hideLabel
        placeholder="play.example.net"
        icon="servers"
        mono
        spellcheck={false}
        autocomplete="off"
        error={addressError}
        bind:value={address}
        oninput={() => (addressError = '')}
      />
      <Button type="submit" busy={busy === 'server'} disabled={!release || !!busy}>{t('Join')}</Button>
    </form>
  </div>
</div>

<style>
  h2,
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-5);
    text-align: start;
  }
  .presets.is-start {
    justify-content: flex-start;
  }
  .preset {
    width: 320px;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .preset.is-main {
    border-color: var(--ink);
  }
  .preset__head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .preset__title {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .grow {
    flex: 1;
  }
  .preset > :global(.cb-btn) {
    justify-content: center;
  }
  .server {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .server > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }
</style>
