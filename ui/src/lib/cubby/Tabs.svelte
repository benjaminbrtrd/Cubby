<script lang="ts">
  import Content from './Content.svelte';
  import { rovingKeys } from './data';
  import type { TabsProps } from './types';

  let { items = [], value = $bindable(), onChange, label, class: className }: TabsProps = $props();

  const index = $derived(Math.max(0, items.findIndex((it) => it.id === value)));

  function select(id: string) {
    value = id;
    onChange?.(id);
  }
</script>

<!-- One tab stop (on the selected tab, not on the list), then the arrow keys, Home and End; focus follows the selection. -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
  class={['cb-tabs', className]}
  role="tablist"
  aria-label={label}
  onkeydown={(e) => rovingKeys(e, items.length, index, (i) => select(items[i].id))}
>
  {#each items as it (it.id)}
    {@const on = it.id === value}
    <button
      type="button"
      role="tab"
      aria-selected={on}
      aria-controls={it.panel}
      tabindex={on ? 0 : -1}
      class={['cb-tab', on ? 'is-on text-body-strong' : 'text-body-strong']}
      onclick={() => select(it.id)}
    >
      <span>{it.label}</span>
      {#if it.count != null}<span class="cb-tab__count text-counter"><Content value={it.count} /></span>{/if}
    </button>
  {/each}
</div>
