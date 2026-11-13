<!-- Description tab: the author's text, mods that pair well with it, and answers to common questions. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Icon, InstanceIcon } from '../../../cubby'
  import { projectArt } from '../../../icon'
  import type { Dependency, Instance, Project, ProjectKind, ProjectVersion } from '../../../types'
  import Markdown from '../Markdown.svelte'
  import { serverHint } from '../discover'
  import { depName, depStatus, openDep } from './shared'

  let {
    project,
    kind,
    target,
    chosen,
    deps,
  }: { project: Project; kind: ProjectKind; target: Instance | undefined; chosen: ProjectVersion | undefined; deps: Dependency[] } = $props()

  const optionalDeps = $derived(deps.filter((d) => d.kind === 'optional'))
</script>

<div id="pp-description" role="tabpanel" aria-label={t('Description')} class="description">
  <div class="body">
    {#if project.body.trim()}
      <Markdown source={project.body} />
    {:else}
      <p class="text-body muted">{t('The author hasn\'t written a description yet.')}</p>
    {/if}
  </div>
  <div class="side">
    {#if kind === 'mod' && optionalDeps.length}
      <section aria-labelledby="pp-pairs">
        <h2 id="pp-pairs" class="text-heading">{t('Works well with')}</h2>
        <p class="text-small muted">{t('Optional mods that add extras to it.')}</p>
        <ul class="pairs">
          {#each optionalDeps as dep (dep.projectId ?? dep.slug ?? dep.title)}
            {@const st = depStatus(dep, target)}
            <li>
              <InstanceIcon {...dep.iconUrl ? { src: dep.iconUrl } : projectArt(dep.slug ?? dep.projectId ?? 'x')} size={32} />
              <span class="pair-text">
                {#if dep.slug || dep.projectId}
                  <button type="button" class="link text-body-strong" onclick={() => openDep(dep)}>{depName(dep)}</button>
                {:else}
                  <span class="text-body-strong">{depName(dep)}</span>
                {/if}
                <span class={['text-small', 'status', st.tone]}><Icon name={st.icon} size={18} />{st.word}</span>
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
    <section aria-labelledby="pp-faq">
      <h2 id="pp-faq" class="text-heading">{t('Good to know')}</h2>
      <dl class="faq">
        {#if kind === 'mod'}
          {@const hint = serverHint(project.clientSide, project.serverSide)}
          {#if hint}
            <div><dt class="text-body-strong">{t('Does it work on servers?')}</dt><dd class="text-body muted">{hint}</dd></div>
          {/if}
          <div>
            <dt class="text-body-strong">{t('Which version do I need?')}</dt>
            <dd class="text-body muted">{t('The one for your instance\'s Minecraft version and loader. Cubby picks it for you and adds the mods it needs.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Can I remove it later?')}</dt>
            <dd class="text-body muted">{t('Yes. Turn it off or remove it in the instance\'s Mods tab. Your worlds stay, but what the mod added disappears from them.')}</dd>
          </div>
        {:else if kind === 'modpack'}
          <div>
            <dt class="text-body-strong">{t('Can I add my own mods?')}</dt>
            <dd class="text-body muted">{t('Yes. It\'s a normal instance: add or turn off mods in its Mods tab.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Can I play with friends?')}</dt>
            <dd class="text-body muted">{chosen ? t('Yes, if everyone has the same version of the pack ({version}). A server needs the same mods too.', { version: chosen.versionNumber }) : t('Yes, if everyone has the same version of the pack. A server needs the same mods too.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Does it change my other instances?')}</dt>
            <dd class="text-body muted">{t('No. It gets its own instance, with its own worlds and settings.')}</dd>
          </div>
        {:else if kind === 'shader'}
          <div>
            <dt class="text-body-strong">{t('Does it change my worlds?')}</dt>
            <dd class="text-body muted">{t('No. Shaders only change how the game is drawn. Turn them off and everything looks as before.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Will it run on my PC?')}</dt>
            <dd class="text-body muted">{t('Shaders work your graphics card harder than the default look. If the game gets slow, lower the pack\'s settings in game, or turn it off.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Where do I turn it on?')}</dt>
            <dd class="text-body muted">{t('In the instance\'s Shaders tab, or in game: Options, Video settings, Shader packs.')}</dd>
          </div>
        {:else if kind === 'datapack'}
          <div>
            <dt class="text-body-strong">{t('Does it change my worlds?')}</dt>
            <dd class="text-body muted">{t('Only the world you put it in. New chunks may look different, and its changes stay in that world. Back the world up first.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Does it need mods?')}</dt>
            <dd class="text-body muted">{t('No. Datapacks work in Vanilla too. On a server, the server puts them in its own world.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Where do I turn it on?')}</dt>
            <dd class="text-body muted">{t('In the instance\'s Worlds tab, under the world. It loads the next time you open the world.')}</dd>
          </div>
        {:else}
          <div>
            <dt class="text-body-strong">{t('Does it change my worlds?')}</dt>
            <dd class="text-body muted">{t('No. Only how things look or sound. Turn it off and the game looks as before.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Does it work on servers?')}</dt>
            <dd class="text-body muted">{t('Yes. It stays on your side, and only you see it. Some servers send their own pack.')}</dd>
          </div>
          <div>
            <dt class="text-body-strong">{t('Where do I turn it on?')}</dt>
            <dd class="text-body muted">{t('In the instance\'s Resource packs tab, or in game: Options, Resource packs.')}</dd>
          </div>
        {/if}
      </dl>
    </section>
  </div>
</div>

<style>
  .description {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    gap: 28px;
    align-items: start;
    margin-top: var(--space-1);
  }
  .body {
    min-width: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    min-width: 0;
  }
  .side h2 {
    margin-bottom: var(--space-2);
  }
  .side h2 + p {
    margin: calc(-1 * var(--space-1)) 0 10px;
  }
  .pairs {
    padding: 0;
    list-style: none;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .pairs li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 10px var(--space-3);
  }
  .pairs li + li {
    border-top: var(--border-hair) solid var(--line);
  }
  .pair-text {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
  }
  .faq {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .faq > div + div {
    padding-top: 14px;
    border-top: var(--border-hair) solid var(--line);
  }
  .faq dd {
    margin-top: var(--space-1);
  }
  @media (max-width: 1100px) {
    .description {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
