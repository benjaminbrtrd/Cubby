<!-- Import: a .mrpack or CurseForge .zip from the PC or from a web address, plus the Prism Launcher import. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon, SegmentedControl, TextField } from '../../cubby'
  import { loaderName, plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { PackFileInfo } from '../../types'
  import Identity from './Identity.svelte'

  type Mode = 'file' | 'url'
  const MODES = [
    { value: 'file', label: t('A file on this PC'), icon: 'folder' as const },
    { value: 'url', label: t('A web address'), icon: 'globe' as const },
  ]

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  let mode = $state<Mode>('file')

  // file

  let path = $state<string | null>(null)
  let info = $state<PackFileInfo | null>(null)
  let inspecting = $state(false)
  let inspectError = $state<string | null>(null)
  let picking = $state(false)

  const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p

  async function choose() {
    if (picking) return
    picking = true
    try {
      const picked = await api.pickPackFile()
      if (picked) await inspect(picked)
    } catch (e) {
      app.error(e)
    } finally {
      picking = false
    }
  }

  async function inspect(p: string) {
    path = p
    info = null
    inspectError = null
    inspecting = true
    try {
      const found = await api.inspectPackFile(p)
      if (path !== p) return
      info = found
      if (!nameTouched) name = found.name
    } catch (e) {
      if (path === p) inspectError = message(e)
    } finally {
      if (path === p) inspecting = false
    }
  }

  // web address

  let url = $state('')
  const urlText = $derived(url.trim())
  const urlBad = $derived(urlText.length > 0 && !/^https?:\/\/\S+$/i.test(urlText))
  const urlName = $derived.by(() => {
    if (urlBad || !urlText) return ''
    try {
      const last = decodeURIComponent(new URL(urlText).pathname.split('/').filter(Boolean).pop() ?? '')
      return last.replace(/\.(mrpack|zip)$/i, '').replace(/[-_]+/g, ' ').trim()
    } catch {
      return ''
    }
  })

  $effect(() => {
    if (mode === 'url' && !nameTouched) name = urlName
  })
  $effect(() => {
    if (mode === 'file' && !nameTouched) name = info?.name ?? ''
  })

  // name, group, icon

  let name = $state('')
  let nameTouched = $state(false)
  let group = $state('Modpacks')
  let submitted = $state(false)
  let busy = $state(false)

  const nameError = $derived(submitted && !name.trim() ? t('Give the instance a name.') : undefined)
  const source = $derived(mode === 'file' ? !!info && !info.blockedReason : !!urlText && !urlBad)
  const canImport = $derived(source && !busy && !inspecting)

  async function run() {
    submitted = true
    if (!canImport || !name.trim()) return
    busy = true
    try {
      const g = group.trim() || null
      const inst = mode === 'file' && path ? await api.importPackFile(path, name.trim(), g) : await api.importPackUrl(urlText, name.trim(), g)
      await app.refreshInstances()
      app.selected = inst.id
      app.openInstance(inst.id)
      app.toast(t('{name} is installing. You can follow it in Downloads.', { name: inst.name }), 'success', { label: t('Downloads'), run: () => app.navigate({ name: 'downloads' }) })
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  const FORMAT = { mrpack: t('Modrinth pack (.mrpack)'), curseforge: t('CurseForge pack (.zip)') } as const
</script>

<div class="import">
  <h1 class="text-title">{t('Import a modpack')}</h1>
  <p class="text-body muted lead">{t('Bring in a pack you downloaded, or one a friend shared. Cubby supports Modrinth')} <span class="text-mono">.mrpack</span> {t('files and CurseForge')} <span class="text-mono">.zip</span> packs.</p>

  <div class="cols">
    <section class="col" aria-labelledby="im-source">
      <h2 id="im-source" class="text-heading">{t('Where is it?')}</h2>
      <SegmentedControl label={t('Where the pack is')} options={MODES} value={mode} onChange={(v) => (mode = v as Mode)} />

      {#if mode === 'file'}
        <div class="pick">
          <Button icon="folder" busy={picking} disabled={picking || inspecting} onclick={choose}>{path ? t('Choose another file') : t('Choose a file')}</Button>
          {#if path}<span class="text-mono muted path" title={path}>{fileName(path)}</span>{:else}<span class="text-small muted">{t('Nothing chosen yet.')}</span>{/if}
        </div>

        {#if inspecting}
          <p class="text-small muted" role="status">{t('Reading {file}…', { file: path ? fileName(path) : t('the file') })}</p>
        {:else if inspectError}
          {#snippet again()}<Button size="sm" onclick={choose}>{t('Choose another file')}</Button>{/snippet}
          <Banner tone="danger" title={t('Cubby can\'t read this file')} actions={again}>{inspectError}</Banner>
        {:else if info}
          <dl class="card">
            <div class="card__head">
              <span class="text-heading">{info.name}</span>
              {#if info.version}<span class="text-mono muted">{info.version}</span>{/if}
            </div>
            <dt class="text-small muted">{t('Format')}</dt>
            <dd class="text-body">{FORMAT[info.format]}</dd>
            <dt class="text-small muted">Minecraft</dt>
            <dd class="text-mono">{info.minecraft}</dd>
            <dt class="text-small muted">{t('Loader')}</dt>
            <dd class="text-mono">{info.loader ? `${loaderName(info.loader.kind)} ${info.loader.version}` : 'Vanilla'}</dd>
            <dt class="text-small muted">{t('Files')}</dt>
            <dd class="text-body">{t('{n} to download', { n: plural(info.fileCount, 'file', 'files') })}</dd>
          </dl>
          {#if info.blockedReason}
            {#snippet settingsAction()}
              <Button size="sm" icon="settings" onclick={() => app.navigate({ name: 'settings', section: 'accounts' })}>{t('Open Settings')}</Button>
            {/snippet}
            <Banner tone="warning" title={t('Cubby can\'t install this pack yet')} actions={settingsAction}>{info.blockedReason}</Banner>
          {/if}
        {/if}
      {:else}
        <TextField label={t('Web address')} icon="globe" placeholder="https://…/pack.mrpack" spellcheck={false} autocomplete="off" bind:value={url} error={urlBad ? t('Enter a web address that starts with http:// or https://.') : undefined} hint={urlBad ? undefined : t('A direct link to a .mrpack or .zip file.')} />
      {/if}
    </section>

    <section class="col" aria-labelledby="im-name">
      <h2 id="im-name" class="text-heading">{t('The new instance')}</h2>
      <Identity bind:name bind:group withIcon={false} {nameError} onName={() => (nameTouched = true)} />
      <Button variant="primary" size="lg" icon="import" busy={busy} disabled={!canImport} onclick={run} class="wide">{t('Import modpack')}</Button>
      <p class="text-small muted">{t('Your file stays where it is. You can keep using Cubby while it installs.')}</p>
    </section>
  </div>

  <div class="prism">
    <Icon name="library" size={18} />
    <span class="text-body">{t('Coming from Prism Launcher?')}</span>
    <Button variant="ghost" size="sm" icon="import" onclick={() => (app.prismImportOpen = true)}>{t('Import from Prism Launcher')}</Button>
  </div>
</div>

<style>
  .import {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  h1,
  h2,
  p,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .lead {
    max-width: 640px;
  }
  .cols {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-6);
  }
  .col {
    flex: 1 1 320px;
    max-width: 460px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .col :global(.wide) {
    width: 100%;
  }
  .pick {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }
  .path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .card__head {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--space-3);
    padding-bottom: var(--space-2);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .prism {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    margin-top: var(--space-2);
    padding-top: var(--space-4);
    color: var(--ink-muted);
    border-top: var(--border-hair) solid var(--line);
  }
</style>
