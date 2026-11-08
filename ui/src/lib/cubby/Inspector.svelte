<!-- The inner column of every side panel: sections stack with the same spacing, and the destructive action, when
     there is one, sits alone at the bottom with what it does. Layout rules: src/styles/inspector.css. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from './Button.svelte';
  import type { IconName } from './data';

  type Danger = { label: string; icon?: IconName; hint?: string; disabled?: boolean; busy?: boolean; onclick: () => void };
  let { children, danger, class: className }: { children: Snippet; danger?: Danger | null; class?: string } = $props();
</script>

<div class={['cb-ins', className]}>
  {@render children()}
  {#if danger}
    <div class="cb-ins-foot">
      <Button size="sm" variant="danger" icon={danger.icon ?? 'trash'} disabled={danger.disabled} busy={danger.busy} onclick={danger.onclick}>{danger.label}</Button>
      {#if danger.hint}<p class="cb-ins-hint">{danger.hint}</p>{/if}
    </div>
  {/if}
</div>
