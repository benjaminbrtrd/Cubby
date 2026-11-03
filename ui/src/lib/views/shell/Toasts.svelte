<script lang="ts">
  import { app } from '../../state.svelte'
  import { Banner, Button } from '../../cubby'

  const TONE = { info: 'info', success: 'success', error: 'danger' } as const
</script>

<!-- Polite live region: finished downloads, errors and short confirmations. -->
<div class="toasts" aria-live="polite" aria-relevant="additions">
  {#each app.toasts as t (t.id)}
    <Banner tone={TONE[t.kind]} onDismiss={() => app.dismissToast(t.id)} dismissLabel="Dismiss">
      {t.text}
      {#if t.action}
        <span class="action">
          <Button
            size="sm"
            onclick={() => {
              t.action?.run()
              app.dismissToast(t.id)
            }}>{t.action.label}</Button
          >
        </span>
      {/if}
    </Banner>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    inset-inline-end: var(--space-6);
    bottom: var(--space-6);
    z-index: var(--layer-toast);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    width: min(420px, calc(100vw - 2 * var(--space-6)));
    pointer-events: none;
  }
  .toasts > :global(*) {
    pointer-events: auto;
    box-shadow: var(--shadow-float);
  }
  .action {
    display: block;
    margin-top: var(--space-2);
  }
</style>
