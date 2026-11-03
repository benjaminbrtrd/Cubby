<script lang="ts">
  import Icon from './Icon.svelte';
  import { rovingKeys } from './data';
  import type { SegmentedControlProps } from './types';

  let { options = [], value = $bindable(), onChange, label, class: className }: SegmentedControlProps = $props();

  const index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));

  function select(v: string) {
    value = v;
    onChange?.(v);
  }
</script>

<!-- One tab stop (on the chosen option, not on the group), then the arrow keys, Home and End; focus follows the choice. -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
  class={['cb-seg', className]}
  role="radiogroup"
  aria-label={label}
  onkeydown={(e) => rovingKeys(e, options.length, index, (i) => select(options[i].value))}
>
  {#each options as o (o.value)}
    {@const on = o.value === value}
    <button
      type="button"
      role="radio"
      aria-checked={on}
      tabindex={on ? 0 : -1}
      class={['cb-seg__opt text-body-strong', on && 'is-on']}
      onclick={() => select(o.value)}
    >
      {#if o.icon}<Icon name={o.icon} size={18} />{/if}<span>{o.label}</span>
    </button>
  {/each}
</div>
