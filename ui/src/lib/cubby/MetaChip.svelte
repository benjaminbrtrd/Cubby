<script lang="ts">
  import Icon from './Icon.svelte';
  import type { MetaChipProps } from './types';

  let { children, icon, tone, title, class: className }: MetaChipProps = $props();
</script>

<span class={['cb-chip text-mono', tone && 'cb-chip--' + tone, className]} {title}>
  {#if icon}<Icon name={icon} size={18} />{/if}
  <span>{@render children?.()}</span>
</span>
