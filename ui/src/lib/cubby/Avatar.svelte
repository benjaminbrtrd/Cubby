<script lang="ts">
  import type { AvatarProps } from './types';

  let { src, name, size: sizeProp, alt, class: className }: AvatarProps = $props();

  const size = $derived(sizeProp || 32);
</script>

<span class={['cb-avatar', className]} style:width="{size}px" style:height="{size}px">
  {#if src}
    <img {src} alt={alt != null ? alt : ''} width={size} height={size} />
  {:else}
    <span class="cb-avatar__fallback" aria-hidden="true">{(name || '?').slice(0, 1).toUpperCase()}</span>
  {/if}
</span>
