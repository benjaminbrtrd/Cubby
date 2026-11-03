<script lang="ts">
  import Icon from './Icon.svelte';
  import Kbd from './Kbd.svelte';
  import type { ButtonProps } from './types';

  let {
    variant = 'secondary',
    size = 'md',
    icon,
    iconEnd,
    kbd,
    busy,
    type,
    class: className,
    children,
    ...rest
  }: ButtonProps = $props();
</script>

<button
  type={type || 'button'}
  class={['cb-btn', 'cb-btn--' + variant, 'cb-btn--' + size, className]}
  aria-busy={busy || undefined}
  {...rest}
>
  {#if icon}<Icon name={icon} size={18} />{/if}
  {#if children}<span class={['cb-btn__label', size === 'lg' ? 'text-heading' : 'text-body-strong']}>{@render children()}</span>{/if}
  {#if iconEnd}<Icon name={iconEnd} size={18} />{/if}
  {#if kbd}<Kbd keys={kbd} class="cb-btn__kbd" />{/if}
</button>
