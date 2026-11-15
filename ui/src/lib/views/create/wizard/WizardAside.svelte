<!-- A project's page opened from the New instance wizard: Add puts it in the instance and goes back to the wizard. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Button, Icon } from '../../../cubby'
  import { loaderName } from '../../../format'
  import { app } from '../../../state.svelte'
  import type { Project, ProjectKind } from '../../../types'
  import { wizard as w, type Pick } from './state.svelte'

  let { project, kind }: { project: Project; kind: ProjectKind } = $props()

  const pick = $derived.by((): Pick | null => {
    if (kind === 'modpack') return null
    const source = project.source === 'curseforge' ? 'curseforge' : 'modrinth'
    // Modrinth projects by slug, like the wizard's list: the same mod is never added twice
    return { source, projectId: source === 'modrinth' ? project.slug || project.id : project.id, kind, title: project.title, author: project.author, iconUrl: project.iconUrl }
  })
  const on = $derived(!!pick && w.has(pick))
  const target = $derived([w.gameVersion, w.gameLoader ? loaderName(w.gameLoader) : null].filter(Boolean).join(' · '))
  /** Nothing says it runs there: the wizard's own list only shows what does, so this is a hint, not a block. */
  const fits = $derived(
    !w.gameVersion || !project.gameVersions.length || project.gameVersions.includes(w.gameVersion),
  )
  const replaces = $derived(kind === 'shader' && !on ? (w.picked('shader')[0]?.title ?? null) : null)

  function add() {
    if (!pick) return
    w.add(pick)
    app.back()
  }
</script>

<section class="wz-aside" aria-label={t('Your new instance')}>
  <p class="cb-ins-eyebrow">{t('Your new instance')}</p>
  {#if target}<p class="text-small muted">{t('For {target}.', { target })}</p>{/if}
  {#if !fits}
    <p class="text-small warn"><Icon name="warning" size={18} />{t('Nothing says it runs on {version}.', { version: w.gameVersion ?? '' })}</p>
  {/if}
  {#if on}
    <Button variant="secondary" size="lg" icon="check" class="wide" onclick={() => app.back()}>{t('Added: back to the new instance')}</Button>
    <Button variant="ghost" size="sm" onclick={() => pick && w.remove(pick)}>{t('Remove it')}</Button>
  {:else}
    <Button variant="primary" size="lg" icon="plus" class="wide" disabled={!pick} onclick={add}>{t('Add to the new instance')}</Button>
    {#if replaces}<p class="text-small muted">{t('It takes the place of {title}: one shader pack at a time.', { title: replaces })}</p>{/if}
    <p class="text-small muted">{t('Back to the new instance. It installs when you create it.')}</p>
  {/if}
</section>

<style>
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--gold-text);
  }
  .wz-aside {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .wz-aside :global(.wide) {
    align-self: stretch;
  }
</style>
