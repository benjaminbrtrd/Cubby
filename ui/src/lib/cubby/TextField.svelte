<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import type { TextFieldProps } from './types';

  let {
    label,
    hint,
    error,
    icon,
    suffix,
    class: className,
    id: idProp,
    hideLabel,
    size,
    mono,
    value = $bindable(),
    ...rest
  }: TextFieldProps = $props();

  const auto = $props.id();
  const id = $derived(idProp || auto);
  const hintId = $derived(hint || error ? id + '-hint' : undefined);
</script>

<div class={['cb-tf', error && 'is-invalid', size === 'sm' && 'cb-tf--sm', className]}>
  {#if label}<label for={id} class={['cb-label text-body-strong', hideLabel && 'cb-visually-hidden']}>{label}</label>{/if}
  <div class={['cb-tf__well', mono ? 'text-mono' : 'text-body']}>
    {#if icon}<Icon name={icon} size={18} class="cb-tf__icon" />{/if}
    <input {id} class="cb-tf__input" aria-invalid={error ? true : undefined} aria-describedby={hintId} {...rest} bind:value />
    {#if suffix}<span class="cb-tf__suffix text-mono"><Content value={suffix} /></span>{/if}
  </div>
  {#if error || hint}
    <p id={hintId} class={['cb-hint text-small', error && 'cb-hint--error']}>
      {#if error}<Icon name="error" size={18} />{/if}{error || hint}
    </p>
  {/if}
</div>
