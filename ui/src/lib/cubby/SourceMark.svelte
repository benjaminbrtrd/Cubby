<!-- Where a file comes from, as the source's logo (Modrinth, CurseForge) or a folder for a local file. The name
     is in the tooltip and read by screen readers. -->
<script lang="ts">
  import Icon from './Icon.svelte'

  let { tone, label, size = 18 }: { tone: 'modrinth' | 'curseforge' | 'local'; label: string; size?: number } = $props()
</script>

<span class="cb-source" title={label}>
  {#if tone === 'local'}
    <Icon name="folder" {size} />
  {:else}
    <img src="/providers/{tone}.svg" alt="" width={size} height={size} />
  {/if}
  <span class="cb-visually-hidden">{label}</span>
</span>

<style>
  .cb-source {
    flex: none;
    display: inline-flex;
    align-items: center;
    color: var(--ink-muted);
    vertical-align: middle;
  }
  img {
    display: block;
    object-fit: contain;
  }
</style>
