<script lang="ts">
  import type { KbdProps } from './types';

  let { keys, children, class: className }: KbdProps = $props();
</script>

<span class={['cb-kbd-group', className]}>
  {#if Array.isArray(keys)}
    {#each keys as k, i (i)}<kbd class="cb-kbd">{k}</kbd>{/each}
  {:else if keys != null}
    <kbd class="cb-kbd">{keys}</kbd>
  {:else}
    <kbd class="cb-kbd">{@render children?.()}</kbd>
  {/if}
</span>
