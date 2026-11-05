<!-- Text from crashText.ts: plain words, with mod ids, packages and versions in mono. -->
<script lang="ts">
  import type { Part } from './crashText'

  let { parts }: { parts: Part[] } = $props()
</script>

{#each parts as p, i (i)}{#if typeof p === 'string'}{p}{:else}<span class="text-mono">{p.mono}</span>{/if}{/each}
