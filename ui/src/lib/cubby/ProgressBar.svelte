<script lang="ts">
  import Content from './Content.svelte';
  import { pct } from './data';
  import type { ProgressBarProps } from './types';

  let { value, label, detail, valueText, ariaValueText, tone, segments, class: className }: ProgressBarProps = $props();

  const v = $derived(typeof value === 'number' ? Math.max(0, Math.min(100, value)) : null);
  const segs = $derived(segments || 20);
  const on = $derived(v == null ? 0 : Math.round((v / 100) * segs));
  const vt = $derived(valueText || (v == null ? undefined : pct(v)));
</script>

<div class={['cb-progress', v == null && 'cb-progress--indeterminate', tone && 'cb-progress--' + tone, className]}>
  {#if label || vt}
    <div class="cb-progress__head">
      {#if label}<span class="cb-progress__label text-body-strong">{label}</span>{:else}<span></span>{/if}
      {#if vt}<span class="cb-progress__value text-counter">{vt}</span>{/if}
    </div>
  {/if}
  <div
    class="cb-progress__track"
    role="progressbar"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={v == null ? undefined : Math.round(v)}
    aria-valuetext={ariaValueText || vt}
    aria-label={label}
  >
    {#if v == null}
      <span class="cb-progress__runner"></span>
    {:else}
      {#each { length: segs }, i (i)}<span class={['cb-progress__seg', i < on && 'is-on']}></span>{/each}
    {/if}
  </div>
  {#if detail}<div class="cb-progress__detail text-small"><Content value={detail} /></div>{/if}
</div>
