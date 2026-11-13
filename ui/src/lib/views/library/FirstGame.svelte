<!--
  Empty library: one click to a first game (latest release, Fabric for performance mods, or a
  server), or the longer ways in (custom version and loader, a modpack, Prism Launcher).
-->
<script lang="ts">
  import { Button, Logo } from '../../cubby'
  import { app } from '../../state.svelte'
  import { t } from '../../i18n.svelte'
  import PresetCards from '../create/PresetCards.svelte'

  function browseModpacks() {
    app.discover.kind = 'modpack'
    app.navigate({ name: 'discover' })
  }

  function custom() {
    app.newSource = 'custom'
    app.navigate({ name: 'create' })
  }
</script>

<section class="first" aria-labelledby="first-title">
  <Logo size={64} wordmark={false} />
  <h1 id="first-title" class="text-display">{t('Let’s set up your first game')}</h1>
  <p class="text-body muted lead">{t('Each instance is its own copy of Minecraft, with its own version, mods and worlds. Pick one to start with. You can add more any time.')}</p>

  <div class="cards">
    <PresetCards />
  </div>

  <div class="more">
    <span class="text-label muted">{t('Or')}</span>
    <Button variant="ghost" icon="settings" onclick={custom}>{t('Pick a version and loader')}</Button>
    <Button variant="ghost" icon="discover" onclick={browseModpacks}>{t('Install a modpack')}</Button>
    <Button variant="ghost" icon="import" onclick={() => (app.prismImportOpen = true)}>{t('Import from another launcher')}</Button>
  </div>
</section>

<style>
  .first {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding: var(--space-12) var(--space-6);
    text-align: center;
  }
  h1,
  p {
    margin: 0;
  }
  h1 {
    margin-top: var(--space-2);
  }
  .muted {
    color: var(--ink-muted);
  }
  .lead {
    max-width: 560px;
  }
  .cards {
    margin-top: var(--space-5);
  }
  .more {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    margin-top: var(--space-5);
  }
</style>
