<!-- Change the version of a mod, shader pack or resource pack already in an instance: an older one to go back, or
     a newer one. It shows the install panel of the project's source (Modrinth or CurseForge) with this instance
     as the target; installing another version replaces the file that is there. -->
<script lang="ts" module>
  import type { ProjectKind } from '../../types'

  /** The project whose version changes, and where it comes from. */
  export interface VersionTarget {
    source: 'modrinth' | 'curseforge'
    projectId: string
    kind: ProjectKind
    title: string
    /** The version (Modrinth) or file (CurseForge) installed now: the dialog starts on it. */
    current?: string
    /** A datapack: the world it is in and its file, removed once the other version is in. */
    datapack?: { world: string; fileName: string }
  }
</script>

<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog } from '../../cubby'
  import { app } from '../../state.svelte'
  import type { Project } from '../../types'
  import CfInstall from '../discover/CfInstall.svelte'
  import InstallPanel from '../discover/InstallPanel.svelte'
  import DatapackInstall from '../discover/DatapackInstall.svelte'

  let { instanceId, item, onClose }: { instanceId: string; item: VersionTarget; onClose: () => void } = $props()

  // The panels install into Discover's target: this instance while the dialog is open, then the previous one again.
  const previous = app.targetId
  untrack(() => {
    app.setTarget(instanceId)
    if (item.datapack) app.targetWorld = item.datapack.world
  })
  function close() {
    if (previous && previous !== untrack(() => instanceId) && app.instance(previous)) app.setTarget(previous)
    onClose()
  }

  /** The version shown in the panel: the installed one first, so the player picks from where they are. */
  let versionId = $state<string | null>(untrack(() => item.current ?? null))
  let project = $state<Project | null>(null)
  let error = $state<string | null>(null)
  $effect(() => {
    const { source, projectId, kind } = item
    let stale = false
    const load = source === 'curseforge' ? api.curseforgeProject(projectId, kind) : api.getProject(projectId)
    load
      .then((p) => {
        if (!stale) project = { ...p, kind }
      })
      .catch((e) => {
        if (!stale) error = typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
      })
    return () => {
      stale = true
    }
  })
</script>

{#snippet actions()}
  <Button onclick={close}>{t('Close')}</Button>
{/snippet}

<Dialog wide title={t('Versions of {title}', { title: project?.title ?? item.title })} onClose={close} {actions}>
  <p class="text-small muted intro">{t('Pick another version to go back to an older one, or to move to a newer one. Cubby replaces the file that is there.')}</p>
  {#if error}
    <Banner tone="danger" title={t('Couldn’t load {title}', { title: item.title })}>{error}</Banner>
  {:else if !project}
    <p class="text-small muted">{t('Loading versions…')}</p>
  {:else if item.source === 'curseforge'}
    <CfInstall {project} pageUrl={project.pageUrl ?? ''} onLeave={close} replaces={item.datapack?.fileName} inInstance bind:fileId={versionId} />
  {:else if item.datapack}
    <DatapackInstall {project} compact onLeave={close} replaces={item.datapack.fileName} />
  {:else}
    <InstallPanel {project} compact toQueue={false} inInstance onLeave={close} bind:versionId />
  {/if}
</Dialog>

<style>
  .intro {
    margin: 0 0 var(--space-3);
    color: var(--ink-muted);
  }
</style>
