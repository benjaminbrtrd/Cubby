<!-- The Install panel of a datapack (project page aside, or the dialog of a Discover card): a datapack goes into
     one world, so it asks for the instance and the world, then downloads the chosen version there. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Icon } from '../../cubby'
  import { bytes, relativeTime } from '../../format'
  import { app } from '../../state.svelte'
  import type { ProjectHit, ProjectVersion } from '../../types'
  import Picker from './Picker.svelte'
  import TargetPicker from './TargetPicker.svelte'
  import WorldPicker from './WorldPicker.svelte'
  import { CHANNEL, fits, mcRange } from './discover'

  interface Props {
    project: ProjectHit
    /** Every version of the project, newest first. Leave it out and the panel loads them; null = loading. */
    versions?: ProjectVersion[] | null
    versionsError?: string | null
    /** In a dialog: no title. */
    compact?: boolean
    /** Called when a link of the panel leaves the screen (to close a dialog). */
    onLeave?: () => void
  }

  let { project, versions: given, versionsError = null, compact = false, onLeave }: Props = $props()

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  let loaded = $state<ProjectVersion[] | null>(null)
  let loadError = $state<string | null>(null)
  $effect(() => {
    if (given !== undefined) return
    const id = project.id
    let stale = false
    api
      .listProjectVersions(id, null, null)
      .then((v) => {
        if (!stale) loaded = v
      })
      .catch((e) => {
        if (!stale) loadError = message(e)
      })
    return () => {
      stale = true
    }
  })

  const target = $derived(app.target)
  /** Versions that are datapacks (a project can also ship as a mod), the ones for the target first. */
  const all = $derived((given !== undefined ? given : loaded)?.filter((v) => v.loaders.includes('datapack')) ?? null)
  const failed = $derived(given !== undefined ? versionsError : loadError)
  const ordered = $derived(all ? [...all.filter((v) => fits(v, 'datapack', target)), ...all.filter((v) => !fits(v, 'datapack', target))] : [])
  let versionId = $state<string | null>(null)
  const chosen = $derived(ordered.find((v) => v.id === versionId) ?? ordered[0] ?? null)
  const chosenFits = $derived(!!chosen && fits(chosen, 'datapack', target))
  const zip = $derived(chosen?.files.find((f) => f.primary && f.fileName.toLowerCase().endsWith('.zip')) ?? chosen?.files.find((f) => f.fileName.toLowerCase().endsWith('.zip')) ?? null)

  // A new instance gets its own best version.
  let lastTarget: string | undefined
  $effect(() => {
    const id = target?.id
    if (id !== lastTarget) {
      if (lastTarget !== undefined) versionId = null
      lastTarget = id
    }
  })

  const items = $derived(ordered.map((v) => ({ id: v.id, v, group: fits(v, 'datapack', target) ? (target ? t('For {name}', { name: target.name }) : undefined) : t('Other versions') })))

  let busy = $state(false)
  /** "<instance>/<world>/<version>" installed from this panel. */
  let done = $state<string[]>([])
  const key = $derived(`${target?.id}/${app.targetWorld}/${chosen?.id}`)
  const running = $derived(!!target && !!app.running[target.id])

  async function install() {
    if (!target || !app.targetWorld || !chosen) return
    busy = true
    const k = key
    try {
      await api.installDatapack(target.id, app.targetWorld, project.id, chosen.id)
      done = [...done, k]
      app.toast(t('{title} is in the world. It loads the next time you open it.', { title: project.title }), 'success')
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  function openWorlds() {
    if (!target) return
    onLeave?.()
    app.openInstance(target.id, 'worlds')
  }
</script>

<section class="panel" aria-label={t('Install')}>
  {#if !compact}<h2 class="text-heading">{t('Install')}</h2>{/if}
  <div class="field">
    <span class="text-label muted">{t('Install to')}</span>
    <TargetPicker />
  </div>
  <div class="field">
    <span class="text-label muted">{t('Into world')}</span>
    <WorldPicker />
  </div>

  {#if failed}
    <Banner tone="danger" title={t('Couldn’t load the versions')}>{failed}</Banner>
  {:else if all === null}
    <p class="text-small muted">{t('Loading versions…')}</p>
  {:else if !ordered.length}
    <p class="text-small muted">{t('{title} has no datapack version: it only comes as a mod.', { title: project.title })}</p>
  {:else if chosen}
    <div class="field">
      <span class="text-label muted">{t('Version')}</span>
      <Picker {items} value={chosen.id} onSelect={(id) => (versionId = id)} label={t('Version')} listWidth={340}>
        {#snippet trigger()}
          <span class="stack">
            <span class="text-body-strong">{chosen.versionNumber}</span>
            <span class="text-small muted">{CHANNEL[chosen.channel].word} · <span class="text-mono">{mcRange(chosen.gameVersions)}</span></span>
          </span>
        {/snippet}
        {#snippet option(item)}
          <span class="stack">
            <span class="text-body-strong">{item.v.versionNumber}</span>
            <span class="text-small muted">{CHANNEL[item.v.channel].word} · <span class="text-mono">{mcRange(item.v.gameVersions)}</span> · {relativeTime(item.v.published, '')}</span>
          </span>
        {/snippet}
      </Picker>
    </div>
    {#if target && !chosenFits}
      <p class="warn text-small"><Icon name="warning" size={18} />{t('This version isn’t made for Minecraft {mc}. The world may not load it.', { mc: target.minecraft })}</p>
    {/if}
    {#if done.includes(key)}
      <div class="row">
        <span class="ok text-small"><Icon name="check" size={18} />{t('In the world')}</span>
        <Button size="sm" variant="ghost" iconEnd="chevron-right" onclick={openWorlds}>{t('See its datapacks')}</Button>
      </div>
    {:else}
      <Button variant="primary" icon="download" busy={busy} disabled={busy || !target || !app.targetWorld || !zip || running} onclick={install}>
        {zip ? t('Install · {size}', { size: bytes(zip.size) }) : t('Install')}
      </Button>
      {#if running}<p class="text-small muted">{t('Close the game first: the world reads its datapacks when it opens.')}</p>{/if}
      {#if !zip}<p class="text-small muted">{t('This version has no .zip to put in a world.')}</p>{/if}
    {/if}
  {/if}
</section>

<style>
  p,
  h2 {
    margin: 0;
  }
  .panel,
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
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .muted {
    color: var(--ink-muted);
  }
  .warn,
  .ok {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .warn {
    color: var(--gold-text);
  }
  .ok {
    color: var(--grass-text);
  }
</style>
