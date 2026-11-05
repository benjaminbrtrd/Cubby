<!-- Top of a project page: icon, title, author, chips and stats, then what the kind of project is. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Banner, Icon, IconButton, InstanceIcon, Kbd, MetaChip } from '../../../cubby'
  import type { IconName } from '../../../cubby'
  import { compactNum, relativeTime } from '../../../format'
  import { projectArt } from '../../../icon'
  import { app } from '../../../state.svelte'
  import type { Project, ProjectKind, ProjectVersion } from '../../../types'
  import { categoryLabel, loaderLabel, mcRange, modLoadersOf, modrinthUrl, newestMc, resolutionOf, sideText } from '../discover'
  import { follow, openUrl } from './shared'

  let { project, kind, chosen }: { project: Project; kind: ProjectKind; chosen: ProjectVersion | undefined } = $props()

  const icon = $derived(project.iconUrl ? { src: project.iconUrl } : projectArt(project.slug))
  const featured = $derived(project.featuredImage ?? project.gallery.find((g) => g.featured)?.url ?? project.gallery[0]?.url ?? null)
  const pageUrl = $derived(modrinthUrl(kind, project.slug))
  const authorUrl = $derived(`https://modrinth.com/user/${encodeURIComponent(project.author)}`)
  const side = $derived(sideText(project.clientSide, project.serverSide))
  const resolution = $derived(resolutionOf(project.categories))
  const packMc = $derived(chosen ? newestMc(chosen.gameVersions) : null)
  const packLoader = $derived(chosen ? modLoadersOf(chosen.loaders).map(loaderLabel)[0] : undefined)
  const lower = (s: string) => s.charAt(0).toLowerCase() + s.slice(1)

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(pageUrl)
      app.toast(t('Link copied'), 'success')
    } catch (e) {
      app.error(e)
    }
  }
</script>

{#snippet chip(text: string, iconName?: IconName, title?: string)}
  <MetaChip icon={iconName} {title}>{text}</MetaChip>
{/snippet}

<header class="header">
  <InstanceIcon {...icon} size={64} raised label={project.title} />
  <div class="header-text">
    <div class="title-line">
      <h1 class="text-title">{project.title}</h1>
      <span class="text-body muted">
        {t('by')} <a href={authorUrl} class="author" onclick={(e) => follow(e, authorUrl)} onauxclick={(e) => follow(e, authorUrl)}>{project.author}</a>
      </span>
    </div>
    <p class="text-body">{project.summary}</p>
    <div class="chips">
      {#each project.categories as c (c)}{@render chip(categoryLabel(c))}{/each}
      {#if kind === 'mod' && side}{@render chip(side, 'servers', t('Where it runs'))}{/if}
      {#if (kind === 'mod' || kind === 'modpack') && modLoadersOf(project.loaders).length}
        {@render chip(modLoadersOf(project.loaders).map(loaderLabel).join(', '), undefined, 'Loaders')}
      {/if}
      {#if kind === 'shader' && project.loaders.length}{@render chip(t('Runs with {loaders}', { loaders: project.loaders.map(loaderLabel).join(', ') }))}{/if}
      {#if project.gameVersions.length}{@render chip(mcRange(project.gameVersions), undefined, t('Minecraft versions'))}{/if}
      {#if project.license}{@render chip(project.license, undefined, 'License')}{/if}
    </div>
    <ul class="stats text-small" aria-label={t('Project stats')}>
      <li><Icon name="download" size={18} />{t('{n} downloads', { n: compactNum(project.downloads) })}</li>
      <li><Icon name="accounts" size={18} />{t('{n} followers', { n: compactNum(project.follows) })}</li>
      <li><Icon name="clock" size={18} />{t('Updated {never}', { never: lower(relativeTime(project.updated, 'never')) })}</li>
      <li><Icon name="globe" size={18} />{t('Source: Modrinth')}</li>
    </ul>
  </div>
  <div class="header-actions">
    <IconButton icon="copy" label={t('Copy link')} onclick={copyLink} />
    <IconButton icon="globe" label={t('Open on Modrinth')} onclick={() => openUrl(pageUrl)} />
  </div>
</header>

{#if kind === 'modpack'}
  <Banner tone="info" title={t('What is a modpack?')}>
    <p>
      {t('A modpack is a set of mods chosen and tested together. Cubby installs it as a new instance with everything set up: Minecraft {mc}{loader} and every mod with its settings, in one step. Your other instances stay as they are.', { mc: packMc ?? '', loader: packLoader ? t(', the {packLoader} loader', { packLoader }) : '' })}
    </p>
  </Banner>
{:else if kind === 'shader' || kind === 'resourcepack'}
  <section class={['what', !featured && 'no-image']} aria-labelledby="pp-what">
    {#if featured}
      <figure>
        <img src={featured} alt={t('{title} in game', { title: project.title })} loading="lazy" />
        <figcaption class="text-body-strong">{t('With {title}', { title: project.title })}</figcaption>
      </figure>
    {/if}
    <div class="what-text">
      {#if kind === 'shader'}
        <h2 id="pp-what" class="text-body-strong">{t('What is a shader pack?')}</h2>
        <p class="text-small muted">{t('It changes how light, shadows, water and sky are drawn. Blocks, mobs and the way you play stay the same, so it\'s safe in any world.')}</p>
        <p class="text-small muted">
          {t('It runs through a mod: Iris, which needs Sodium, or OptiFine. Both Iris and Sodium are in Discover, under Mods. With Iris, press')} <Kbd keys="K" /> in game to turn shaders on or off.
        </p>
      {:else}
        <h2 id="pp-what" class="text-body-strong">{t('What is a resource pack?')}</h2>
        <p class="text-small muted">{t('It changes how blocks, items, mobs and menus look, and sometimes how they sound. It adds nothing new, and the game plays the same.')}</p>
        {#if resolution}
          <p class="text-small muted">
            {t('{n}x means each texture is {n} × {n} pixels. Minecraft’s own are 16 × 16.', { n: resolution })}{resolution > 32 ? t(' Bigger textures need more video memory.') : ''}
          </p>
        {/if}
        <p class="text-small muted">
          {t('The order matters: when two packs change the same thing, the one higher in the list wins. Press')} <Kbd keys="F3" /> + <Kbd keys="T" /> in game to reload them.
        </p>
      {/if}
    </div>
  </section>
{/if}

<style>
  .header {
    display: flex;
    align-items: flex-start;
    gap: var(--space-5);
  }
  .header-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .title-line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-3);
  }
  .author {
    color: var(--grass-text);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 6px var(--space-5);
    padding: 0;
    list-style: none;
    color: var(--ink-muted);
  }
  .stats li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .header-actions {
    display: flex;
    gap: var(--space-1);
  }

  /* what is it */
  .what {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--space-4);
    align-items: stretch;
  }
  .what.no-image {
    grid-template-columns: minmax(0, 1fr);
  }
  figure {
    position: relative;
  }
  figure img {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 160px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
    box-sizing: border-box;
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  figcaption {
    position: absolute;
    top: var(--space-2);
    inset-inline-start: var(--space-2);
    padding: 2px var(--space-2);
    background: var(--bg-chrome);
    border: var(--border-block) solid var(--edge);
  }
  .what-text {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) 14px;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  @media (max-width: 1100px) {
    .what {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
