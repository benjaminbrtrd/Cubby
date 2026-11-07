<!-- New instance › From a link. Paste a Cubby pack link: Cubby shows what is inside before anything downloads,
     then creates the instance and installs every file together. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon, InstanceIcon, MetaChip, Switch, TextField } from '../../cubby'
  import { bytes, loaderFull, plural } from '../../format'
  import { projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import { codeFromText, decodePack, type SharedPack } from '../../share'
  import type { VersionBrief } from '../../types'

  let text = $state('')
  // a cubby:// link opened from a web page lands here
  $effect(() => {
    const link = app.pendingPackLink
    if (!link) return
    text = link
    app.pendingPackLink = null
  })
  const code = $derived(codeFromText(text))
  const bad = $derived(text.trim().length > 0 && !code)

  let pack = $state<SharedPack | null>(null)
  let briefs = $state<VersionBrief[]>([])
  let loading = $state(false)
  let error = $state<string | null>(null)
  let skip = $state<Record<string, boolean>>({})
  let name = $state('')
  let installing = $state(false)

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  $effect(() => {
    const c = code
    pack = null
    briefs = []
    error = null
    skip = {}
    if (!c) return
    let stale = false
    loading = true
    ;(async () => {
      try {
        const decoded = await decodePack(c)
        const found = await api.describeVersions(decoded.versions)
        if (stale) return
        pack = decoded
        briefs = decoded.versions.flatMap((v) => found.filter((b) => b.versionId === v))
        name = decoded.name || t('Shared pack')
        for (const o of decoded.optional) skip[o] = false
      } catch (e) {
        if (!stale) error = message(e)
      } finally {
        if (!stale) loading = false
      }
    })()
    return () => {
      stale = true
    }
  })

  const missing = $derived(pack ? pack.versions.length - briefs.length : 0)
  const chosen = $derived(briefs.filter((b) => !skip[b.versionId]))
  const total = $derived(chosen.reduce((s, b) => s + b.size, 0))

  async function paste() {
    try {
      text = await navigator.clipboard.readText()
    } catch {
      app.toast(t('Cubby couldn’t read the clipboard. Paste the link with Ctrl+V.'), 'error')
    }
  }

  async function install() {
    if (!pack || !chosen.length || !name.trim()) return
    installing = true
    try {
      const instance = await api.createInstance({
        name: name.trim(),
        group: null,
        icon: null,
        minecraft: pack.minecraft,
        loader: pack.loader,
      })
      await app.refreshInstances()
      app.selected = instance.id
      // the whole pack is one batch: one plan, one pool of downloads
      api.installVersions(instance.id, chosen.map((b) => b.versionId)).catch((e) => app.error(e))
      app.toast(t('Installing {name}. Follow it in Downloads.', { name: instance.name }), 'success')
      app.navigate({ name: 'downloads' })
    } catch (e) {
      app.error(e)
    } finally {
      installing = false
    }
  }
</script>

<div class="link">
  <h1 class="text-title">{t('Join with a link')}</h1>
  <p class="text-body muted lead">{t('Paste a pack link to see what is inside before anything downloads.')}</p>

  <div class="field">
    <div class="grow">
      <TextField
        label={t('Pack link')}
        hideLabel
        icon="globe"
        placeholder="https://www.cubbylauncher.com/p#…"
        mono
        bind:value={text}
        error={bad ? t('That is not a Cubby pack link.') : (error ?? undefined)}
      />
    </div>
    <Button icon="copy" onclick={paste}>{t('Paste')}</Button>
  </div>

  {#if loading}
    <p class="text-body muted" role="status">{t('Reading the pack…')}</p>
  {:else if pack}
    <section class="card" aria-label={t('Pack preview')}>
      <div class="top">
        <InstanceIcon seed={name || pack.name} size={64} raised />
        <div class="top__text">
          <span class="text-title">{pack.name || t('Shared pack')}</span>
          {#if pack.creator}
            <span class="by text-small muted"><img src={`https://minotar.net/helm/${pack.creator.uuid}/32`} alt="" width="20" height="20" />{t('Shared by {name}', { name: pack.creator.name })}</span>
          {/if}
          {#if pack.note}<p class="text-body">{pack.note}</p>{/if}
        </div>
      </div>
      <div class="chips">
        <MetaChip>Minecraft {pack.minecraft}</MetaChip>
        <MetaChip>{loaderFull(pack.loader)}</MetaChip>
        <MetaChip>{plural(briefs.length, 'file', 'files')}</MetaChip>
      </div>

      {#if missing > 0}
        <Banner tone="warning" icon="warning" title={t('{n} files are not on Modrinth any more', { n: missing })}>
          {t('They were removed or hidden by their authors. Everything else installs.')}
        </Banner>
      {/if}

      <h2 class="text-label muted">{t('Files')}</h2>
      <ul class="mods">
        {#each briefs as b (b.versionId)}
          {@const opt = pack.optional.includes(b.versionId)}
          <li class="mod">
            <InstanceIcon {...(b.iconUrl ? { src: b.iconUrl } : projectArt(b.slug || b.title))} size={24} />
            {#if opt}
              <Switch checked={!skip[b.versionId]} onChange={(v) => (skip[b.versionId] = !v)} label={b.title} />
              <span class="text-small muted grow">{t('optional')}</span>
            {:else}
              <span class="text-body grow">{b.title}</span>
              <Icon name="check" size={16} />
            {/if}
            <span class="text-small muted text-mono">{b.versionNumber}</span>
            <span class="text-small muted text-counter">{bytes(b.size)}</span>
          </li>
        {/each}
      </ul>

      <TextField label={t('Instance name')} bind:value={name} />
      <div class="go">
        <span class="text-small muted">{t('{n} files · {size} · installs as a new instance, downloaded together', { n: chosen.length, size: bytes(total) })}</span>
        <Button variant="primary" size="lg" icon="download" busy={installing} disabled={installing || !name.trim() || !chosen.length} onclick={install}>{t('Install pack')}</Button>
      </div>
    </section>
  {:else if !text.trim()}
    <Banner tone="info" icon="info" title={t('Where do links come from?')}>
      {t('From the Share button on any instance. People post them on Discord, in a README or by message. Anyone with the link can install the pack.')}
    </Banner>
  {/if}
</div>

<style>
  .link {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
    max-width: 760px;
    box-sizing: border-box;
  }
  h1,
  h2,
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .field {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .top {
    display: flex;
    gap: var(--space-4);
    align-items: flex-start;
  }
  .top__text {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .by {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .by img {
    image-rendering: pixelated;
    border: var(--border-hair) solid var(--edge);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .mods {
    list-style: none;
    margin: 0;
    padding: var(--space-2) var(--space-3);
    max-height: 260px;
    overflow: auto;
    background: var(--field);
    box-shadow: var(--shadow-well);
    border: var(--border-block) solid var(--edge);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .mod {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--size-control-sm);
  }
  .go {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
</style>
