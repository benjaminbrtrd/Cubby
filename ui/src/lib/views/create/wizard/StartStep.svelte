<!-- Wizard › Start: build your own, start from a modpack, or import; and the one-click presets. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Icon, SegmentedControl, type IconName } from '../../../cubby'
  import ImportSource from '../ImportSource.svelte'
  import LinkSource from '../LinkSource.svelte'
  import PresetCards from '../PresetCards.svelte'
  import { wizard as w, type WizardPath } from './state.svelte'

  const CHOICES: { id: WizardPath; title: string; text: string; foot: string; icon: IconName }[] = [
    { id: 'custom', title: t('Build your own'), text: t('Pick a Minecraft version and a loader, then add the mods and packs you want.'), foot: t('Version, loader, mods, looks'), icon: 'wrench' },
    { id: 'modpack', title: t('Start from a modpack'), text: t('A set of mods tested together, from Modrinth, CurseForge, FTB, ATLauncher or Technic.'), foot: t('The pack picks the version and the loader'), icon: 'library' },
    { id: 'import', title: t('Import'), text: t('A .mrpack or CurseForge .zip, or a link to a pack.'), foot: t('Prism and other launchers: Library › Import'), icon: 'import' },
  ]
  const FROM = [
    { value: 'file', label: t('A modpack file or address') },
    { value: 'link', label: t('A shared Cubby link') },
  ]
</script>

<div class="intro">
  <h2 class="text-heading">{t('How do you want to start?')}</h2>
  <p class="text-small muted">{t('You can add or remove anything later, from the instance.')}</p>
</div>

<div class="choices" role="radiogroup" aria-label={t('How do you want to start?')}>
  {#each CHOICES as c (c.id)}
    {@const on = w.path === c.id}
    <button type="button" role="radio" aria-checked={on} class={['choice', on && 'is-on']} onclick={() => (w.path = c.id)}>
      <span class="ic"><Icon name={c.icon} size={27} /></span>
      <span class="text-heading">{c.title}</span>
      <span class="text-small muted">{c.text}</span>
      <span class="grow"></span>
      <span class="text-small foot">{c.foot}</span>
    </button>
  {/each}
</div>

{#if w.path === 'import'}
  <section class="import" aria-label={t('Import')}>
    <SegmentedControl label={t('Import from')} options={FROM} value={w.importFrom} onChange={(v) => (w.importFrom = v as 'file' | 'link')} />
    <div class="import__body">
      {#if w.importFrom === 'file'}<ImportSource />{:else}<LinkSource />{/if}
    </div>
  </section>
{:else}
  <section class="quick" aria-labelledby="wz-quick">
    <h2 id="wz-quick" class="text-label muted">{t('Or start in one click')}</h2>
    <p class="text-small muted">{t('Cubby sets it up and starts the game right away.')}</p>
    <PresetCards align="grid" />
  </section>
{/if}

<style>
  h2,
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
  }
  .intro {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--space-5);
  }
  .choice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    min-height: 200px;
    padding: var(--space-5);
    font: inherit;
    color: var(--ink);
    text-align: start;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
    cursor: pointer;
    transition:
      transform var(--duration-fast) linear,
      box-shadow var(--duration-fast) linear;
  }
  .choice:hover {
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block-lift);
  }
  .choice:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring), var(--shadow-block);
  }
  .choice.is-on {
    outline: var(--border-block) solid var(--ink);
    outline-offset: 3px;
  }
  .ic {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    color: var(--grass-text);
    background: var(--field);
    border: var(--border-block) solid var(--edge);
  }
  .foot {
    color: var(--grass-text);
  }
  .import,
  .quick {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .import__body {
    display: flex;
    flex-direction: column;
    min-height: 320px;
    border: var(--border-block) solid var(--edge);
  }
</style>
