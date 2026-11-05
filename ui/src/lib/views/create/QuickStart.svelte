<!-- Quick start: the one-click presets, also shown in the empty library. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import PresetCards from './PresetCards.svelte'
</script>

<div class="quick">
  <h1 class="text-title">{t('Quick start')}</h1>
  <p class="text-body muted lead">{t('Pick one and Cubby sets it up and starts the game. You can change the version, mods and everything else later.')}</p>
  <PresetCards align="start" />
</div>

<style>
  .quick {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  h1,
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .lead {
    max-width: 640px;
    margin-bottom: var(--space-2);
  }
</style>
