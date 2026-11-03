<script lang="ts">
  import { ICONS, MIRRORED, iconSize, iconStroke } from './data';
  import type { IconProps } from './types';

  let { name, size, label, class: className }: IconProps = $props();

  const px = $derived(iconSize(size));
  const stroke = $derived(iconStroke(px));
  const shapes = $derived(ICONS[name] || ICONS.cube);
</script>

<svg
  class={['cb-icon', MIRRORED[name] && 'cb-icon--mirror', className]}
  width={px}
  height={px}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="square"
  stroke-linejoin="miter"
  focusable="false"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
>
  {#each shapes as s, i (i)}
    {#if s.fill}<path d={s.d} fill="currentColor" stroke="none" />{:else}<path d={s.d} />{/if}
  {/each}
</svg>
