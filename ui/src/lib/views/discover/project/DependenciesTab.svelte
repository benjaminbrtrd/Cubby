<!-- Dependencies tab (mods): what the chosen version needs, can use, conflicts with or already contains. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Icon } from '../../../cubby'
  import type { Dependency, Instance, ProjectVersion } from '../../../types'
  import { DEP_GROUPS, depName, depStatus, openDep } from './shared'

  let {
    target,
    chosen,
    deps,
    loading,
  }: { target: Instance | undefined; chosen: ProjectVersion | undefined; deps: Dependency[]; loading: boolean } = $props()
</script>

<div id="pp-dependencies" role="tabpanel" aria-label={t('Dependencies')} class="panel">
  <p class="text-small muted">
    {#if chosen}{t('For version')} <span class="text-mono">{chosen.versionNumber}</span>. {/if}{t('Cubby adds required mods for you and warns you about conflicts.')}
  </p>
  {#if loading}
    <div class="table-skeleton" aria-hidden="true">{#each { length: 3 }, i (i)}<span class="sk tall"></span>{/each}</div>
  {:else if !deps.length}
    <div class="empty"><p class="text-body">{t('Nothing else needed. It works on its own.')}</p></div>
  {:else}
    <div class="dep-groups">
      {#each DEP_GROUPS as g (g.kind)}
        {@const list = deps.filter((d) => d.kind === g.kind)}
        {#if list.length || g.kind !== 'embedded'}
          <section class="dep-group" aria-labelledby="pp-dep-{g.kind}">
            <div>
              <h3 id="pp-dep-{g.kind}" class={['text-label', 'dep-title', g.kind === 'incompatible' && 'bad']}>
                {#if g.kind === 'incompatible'}<Icon name="error" size={18} />{/if}{g.title}
              </h3>
              <p class="text-small muted">{g.hint}</p>
            </div>
            {#if !list.length}
              <p class="text-small muted">{t('None')}</p>
            {/if}
            {#each list as dep, i (i)}
              {@const st = depStatus(dep, target)}
              <div class="dep">
                {#if dep.slug || dep.projectId}
                  <button type="button" class="link text-body-strong" onclick={() => openDep(dep)}>{depName(dep)}</button>
                {:else}
                  <p class="text-body-strong">{depName(dep)}</p>
                {/if}
                <p class={['text-small', 'status', st.tone]}><Icon name={st.icon} size={18} />{st.word}</p>
              </div>
            {/each}
          </section>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .dep-groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--space-3);
    align-items: stretch;
  }
  .dep-group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .dep-title {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dep {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .dep + .dep {
    padding-top: 10px;
    border-top: var(--border-hair) solid var(--line);
  }
</style>
