<script lang="ts" module>
  import type { LogLevel } from './types';

  const LEVEL: Record<LogLevel, string> = { debug: 'Debug', info: 'Info', warn: 'Warn', error: 'Error' };
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import type { LogViewerProps } from './types';

  let { lines = [], levelLabels, live, height, toolbar, label, class: className }: LogViewerProps = $props();

  const labels = $derived({ ...LEVEL, ...levelLabels });
  const maxHeight = $derived(height ? (typeof height === 'number' ? height + 'px' : height) : undefined);

  function level(l: string | undefined): LogLevel {
    return l && Object.hasOwn(LEVEL, l) ? (l as LogLevel) : 'info';
  }
</script>

<div class={['cb-log', className]}>
  {#if toolbar}<div class="cb-log__toolbar"><Content value={toolbar} /></div>{/if}
  <!-- Focusable so the log scrolls from the keyboard. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="cb-log__lines text-log"
    role="log"
    aria-live={live ? 'polite' : 'off'}
    aria-label={label || 'Game log'}
    tabindex="0"
    style:max-height={maxHeight}
  >
    {#each lines as l, i (i)}
      {@const lv = level(l.level)}
      <div class={['cb-log__line', 'cb-log__line--' + lv]}>
        <span class="cb-log__time">{l.time}</span>
        <span class="cb-log__level">{labels[lv]}</span>
        {#if l.source}<span class="cb-log__source">{l.source}</span>{/if}
        <span class="cb-log__text">{l.text}</span>
      </div>
    {/each}
  </div>
</div>
