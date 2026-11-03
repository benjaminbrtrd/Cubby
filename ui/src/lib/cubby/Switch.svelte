<script lang="ts">
  import Icon from './Icon.svelte';
  import type { SwitchProps } from './types';

  let { checked = $bindable(false), onChange, label, hint, hideLabel, disabled, id, class: className }: SwitchProps = $props();

  const on = $derived(!!checked);

  function toggle() {
    // read the new value once: `on` is derived from `checked` and would already be flipped below
    const next = !on;
    checked = next;
    onChange?.(next);
  }
</script>

{#snippet button()}
  <button
    type="button"
    role="switch"
    aria-checked={on}
    {id}
    {disabled}
    class={['cb-switch', on && 'is-on', className]}
    aria-label={hideLabel ? label : undefined}
    onclick={toggle}
  >
    <span class="cb-switch__track" aria-hidden="true">
      <span class="cb-switch__knob"><Icon name={on ? 'check' : 'minimize'} size={9} /></span>
    </span>
  </button>
{/snippet}

{#if hideLabel || !label}
  {@render button()}
{:else}
  <!-- The label wraps the switch: a click on the text toggles it, and its text is the switch's accessible name. -->
  <label class="cb-field-inline">
    {@render button()}
    <span class="cb-field-inline__text">
      <span class="text-body-strong">{label}</span>
      {#if hint}<span class="cb-field-inline__hint text-small">{hint}</span>{/if}
    </span>
  </label>
{/if}
