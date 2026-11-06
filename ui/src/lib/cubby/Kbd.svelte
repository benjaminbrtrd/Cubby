<script lang="ts">
  import { keyName } from '../platform';
  import type { KbdProps } from './types';

  let { keys, children, class: className }: KbdProps = $props();
</script>

<span class={['cb-kbd-group', className]}>
  {#if Array.isArray(keys)}
    {#each keys as k, i (i)}<kbd class="cb-kbd">{keyName(k)}</kbd>{/each}
  {:else if keys != null}
    <kbd class="cb-kbd">{keyName(keys)}</kbd>
  {:else}
    <kbd class="cb-kbd">{@render children?.()}</kbd>
  {/if}
</span>
