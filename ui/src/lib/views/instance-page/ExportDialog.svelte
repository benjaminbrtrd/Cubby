<!-- Export an instance as a modpack file other launchers open: .mrpack (Modrinth) or .zip (CurseForge). -->
<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, SegmentedControl, Switch, TextField } from '../../cubby'
  import { bytes, plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { ExportCandidate, ExportResult, Instance } from '../../types'

  let { instance, onClose }: { instance: Instance; onClose: () => void } = $props()

  let format = $state<'mrpack' | 'curseforge'>('mrpack')
  let name = $state(untrack(() => instance.name))
  let version = $state('1.0.0')
  let note = $state('')
  let candidates = $state<ExportCandidate[] | null>(null)
  let include = $state<string[]>([])
  let busy = $state(false)
  let result = $state<ExportResult | null>(null)

  const task = $derived(app.tasks[`export:${instance.id}`])
  const noKey = $derived(format === 'curseforge' && !app.info?.curseforgeKey)

  untrack(() => {
    api
      .exportCandidates(instance.id)
      .then((list) => {
        candidates = list
        include = list.filter((c) => c.suggested).map((c) => c.name)
      })
      .catch((e) => {
        candidates = []
        app.error(e)
      })
  })

  function toggle(c: ExportCandidate, on: boolean) {
    include = on ? [...include, c.name] : include.filter((n) => n !== c.name)
  }

  async function save() {
    busy = true
    try {
      const r = await api.exportInstance(instance.id, { format, name: name.trim(), version: version.trim(), note: note.trim(), include })
      if (r) result = r
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  const FORMATS = [
    { value: 'mrpack', label: 'Modrinth (.mrpack)' },
    { value: 'curseforge', label: 'CurseForge (.zip)' },
  ]
</script>

{#snippet actions()}
  {#if result}
    <Button variant="ghost" icon="folder" onclick={() => result && api.revealFile(result.path).catch((e) => app.error(e))}>{t('Show in folder')}</Button>
    <Button variant="primary" onclick={onClose}>{t('Done')}</Button>
  {:else}
    <Button variant="ghost" onclick={onClose}>{t('Cancel')}</Button>
    <Button variant="primary" icon="download" busy={busy} disabled={busy || !name.trim()} onclick={save}>{t('Export')}</Button>
  {/if}
{/snippet}

<Dialog icon="download" title={t('Export {name}', { name: instance.name })} {onClose} {actions}>
  {#if result}
    <p>{t('Saved to')} <span class="text-mono wrap">{result.path}</span></p>
    <p class="text-small muted">
      {t('{listed} downloaded by the launcher that opens it, {stored} stored in the file.', { listed: plural(result.listed, 'file', 'files'), stored: plural(result.stored, 'file', 'files') })}
    </p>
    {#if result.withoutKey}
      <p class="text-small muted">{t('Without a CurseForge API key, Cubby can’t tell which mods CurseForge has, so they are all stored in the file. It works, but the file is bigger.')}</p>
    {/if}
  {:else}
    <div class="form">
      <SegmentedControl label={t('Format')} options={FORMATS} value={format} onChange={(v) => (format = v as typeof format)} />
      <p class="text-small muted">
        {format === 'mrpack'
          ? t('Opens in Cubby, the Modrinth App, Prism Launcher and most launchers. Mods from Modrinth are downloaded from there, the others are stored in the file.')
          : t('Opens in the CurseForge app, Cubby and Prism Launcher. Mods CurseForge has are downloaded from there, the others are stored in the file.')}
      </p>
      {#if noKey}
        <Banner tone="info" title={t('No CurseForge API key')}>{t('Every mod will be stored in the file.')}</Banner>
      {/if}
      <TextField label={t('Pack name')} bind:value={name} />
      <TextField label={t('Version')} bind:value={version} />
      <TextField label={format === 'mrpack' ? t('Summary') : t('Author')} bind:value={note} />
      <div class="include">
        <p class="text-body-strong">{t('Also include')}</p>
        <p class="text-small muted">{t('Mods, resource packs and shader packs are always in. Turned-off mods are left out.')}</p>
        {#if candidates === null}
          <p class="text-small muted">{t('Reading the instance…')}</p>
        {:else if !candidates.length}
          <p class="text-small muted">{t('Nothing else in this instance yet.')}</p>
        {:else}
          <ul>
            {#each candidates as c (c.name)}
              <li>
                <Switch checked={include.includes(c.name)} label={c.isDir ? `${c.name}/` : c.name} hint={bytes(c.size)} onChange={(on) => toggle(c, on)} />
              </li>
            {/each}
          </ul>
        {/if}
      </div>
      {#if task}
        <p class="text-small muted">{task.label}…</p>
      {/if}
    </div>
  {/if}
</Dialog>

<style>
  p,
  ul {
    margin: 0;
  }
  ul {
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-height: 30vh;
    overflow-y: auto;
  }
  .muted {
    color: var(--ink-muted);
  }
  .wrap {
    overflow-wrap: anywhere;
  }
  .form,
  .include {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .include {
    gap: var(--space-2);
  }
</style>
