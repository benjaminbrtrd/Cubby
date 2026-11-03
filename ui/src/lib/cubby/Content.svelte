<!-- Internal helper: renders a Renderable (what the React reference typed as ReactNode), text or a snippet. No wrapper element. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let { value }: { value?: string | number | Snippet | null } = $props();
</script>

{#if typeof value === 'function'}{@render value()}{:else if value != null}{value}{/if}
