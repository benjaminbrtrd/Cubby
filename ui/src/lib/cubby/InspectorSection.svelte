<!-- A part of a side panel: its title (with a count), an optional small action on the right, then its content. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, count, action, children }: { title: string; count?: number | string | null; action?: Snippet; children: Snippet } = $props();
  const id = `ins-${Math.random().toString(36).slice(2, 9)}`;
</script>

<section class="cb-ins-sec" aria-labelledby={id}>
  <div class="cb-ins-sec__h">
    <h3 {id} class="cb-ins-eyebrow">{title}{#if count != null && count !== ''}<span class="cb-ins-sec__n"> · {count}</span>{/if}</h3>
    {#if action}{@render action()}{/if}
  </div>
  {@render children()}
</section>
