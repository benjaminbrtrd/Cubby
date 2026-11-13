<!-- A file, folder or id on its own line in the details, with a button to show it or copy it. Goes inside .cb-ins-facts. -->
<script lang="ts">
  import IconButton from './IconButton.svelte';

  let { path, onopen, oncopy, openLabel = 'Show in folder', copyLabel = 'Copy' }: { path: string; onopen?: () => void; oncopy?: () => void; openLabel?: string; copyLabel?: string } = $props();
</script>

<div class="cb-ins-path">
  <span>{path}</span>
  {#if oncopy}<IconButton size="sm" icon="copy" label={copyLabel} onclick={oncopy} />{/if}
  {#if onopen}<IconButton size="sm" icon="folder" label={openLabel} onclick={onopen} />{/if}
</div>
