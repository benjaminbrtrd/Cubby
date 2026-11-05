<!-- Side of a project page: the install panel, then links and details. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Icon } from '../../../cubby'
  import type { IconName } from '../../../cubby'
  import { date, num } from '../../../format'
  import type { Project, ProjectKind, ProjectVersion } from '../../../types'
  import InstallPanel from '../InstallPanel.svelte'
  import { loaderLabel, mcRange, modLoadersOf, modrinthUrl, sideText } from '../discover'
  import { follow } from './shared'

  let {
    project,
    kind,
    versions,
    versionsError,
    versionId = $bindable(),
  }: { project: Project; kind: ProjectKind; versions: ProjectVersion[] | null; versionsError: string | null; versionId: string | null } = $props()

  const side = $derived(sideText(project.clientSide, project.serverSide))

  const links = $derived.by(() => {
    const out: { icon: IconName; label: string; url: string }[] = [{ icon: 'globe', label: t('Page on Modrinth'), url: modrinthUrl(kind, project.slug) }]
    if (project.links.source) out.push({ icon: 'terminal', label: t('Source code'), url: project.links.source })
    if (project.links.issues) out.push({ icon: 'warning', label: t('Report an issue'), url: project.links.issues })
    if (project.links.wiki) out.push({ icon: 'library', label: 'Wiki', url: project.links.wiki })
    if (project.links.discord) out.push({ icon: 'accounts', label: 'Discord', url: project.links.discord })
    for (const d of project.links.donation) out.push({ icon: 'globe', label: t('Donate on {platform}', { platform: d.platform }), url: d.url })
    return out
  })
</script>

<InstallPanel {project} {versions} {versionsError} bind:versionId />

{#if links.length}
  <section class="aside-section" aria-labelledby="pp-links">
    <h3 id="pp-links" class="text-label muted">{t('Links')}</h3>
    <ul class="links">
      {#each links as l (l.url + l.label)}
        <li>
          <a href={l.url} class="text-body" title={l.url} onclick={(e) => follow(e, l.url)} onauxclick={(e) => follow(e, l.url)}>
            <Icon name={l.icon} size={18} />{l.label}
          </a>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<section class="aside-section" aria-labelledby="pp-details">
  <h3 id="pp-details" class="text-label muted">{t('Details')}</h3>
  <dl class="details">
    {#if project.license}<dt class="text-small muted">{t('License')}</dt><dd class="text-mono">{project.license}</dd>{/if}
    {#if kind === 'mod' && side}<dt class="text-small muted">{t('Runs on')}</dt><dd class="text-body">{side}</dd>{/if}
    {#if project.gameVersions.length}<dt class="text-small muted">Minecraft</dt><dd class="text-mono">{mcRange(project.gameVersions)}</dd>{/if}
    {#if project.loaders.length && kind !== 'resourcepack'}
      <dt class="text-small muted">{kind === 'shader' ? t('Runs with') : 'Loaders'}</dt>
      <dd class="text-mono">{(kind === 'shader' ? project.loaders : modLoadersOf(project.loaders)).map(loaderLabel).join(', ') || '—'}</dd>
    {/if}
    <dt class="text-small muted">{t('Versions')}</dt><dd class="text-body">{num(Math.max(project.versionCount, versions?.length ?? 0))}</dd>
    <dt class="text-small muted">{t('Source')}</dt><dd class="text-body">Modrinth</dd>
    <dt class="text-small muted">{t('Since')}</dt><dd class="text-body">{date(project.published)}</dd>
  </dl>
</section>

<style>
  .aside-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .links {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0;
    list-style: none;
  }
  .links a {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--grass-text);
  }
  .links a:hover {
    color: var(--ink);
  }
  .details {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
  }
</style>
