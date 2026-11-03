<script lang="ts">
  import type { SliderProps } from './types';

  let {
    label,
    min: minProp,
    max: maxProp,
    step,
    value = $bindable(),
    onChange,
    valueText,
    marks,
    hint,
    id: idProp,
    class: className,
  }: SliderProps = $props();

  const auto = $props.id();
  const id = $derived(idProp || auto);
  const min = $derived(minProp != null ? minProp : 0);
  const max = $derived(maxProp != null ? maxProp : 100);
  const current = $derived(value != null ? value : min);
  const fill = $derived(((current - min) / (max - min)) * 100);

  function input(e: Event & { currentTarget: HTMLInputElement }) {
    const v = Number(e.currentTarget.value);
    value = v;
    onChange?.(v);
  }
</script>

<div class={['cb-slider', className]}>
  <div class="cb-slider__head">
    <label for={id} class="cb-label text-body-strong">{label}</label>
    <output for={id} class="cb-slider__value text-mono">{valueText != null ? valueText : current}</output>
  </div>
  <!-- A native range input: arrows, Page Up/Down, Home and End come with it. min, max and step come before value so it is never clamped to the default range. -->
  <input
    {id}
    type="range"
    {min}
    {max}
    {step}
    value={current}
    class="cb-slider__input"
    aria-valuetext={valueText}
    style:--cb-fill="{fill}%"
    oninput={input}
  />
  {#if marks}
    <div class="cb-slider__marks text-small" aria-hidden="true">
      {#each marks as m, i (i)}<span>{m}</span>{/each}
    </div>
  {/if}
  {#if hint}<p class="cb-hint text-small">{hint}</p>{/if}
</div>
