<script lang="ts">
  import Icon from './Icon.svelte';
  import type { IconButtonProps } from './types';

  let { icon, label, variant = 'ghost', size = 'md', class: className, ...rest }: IconButtonProps = $props();
</script>

<button
  type="button"
  class={['cb-iconbtn', 'cb-iconbtn--' + variant, 'cb-iconbtn--' + size, className]}
  aria-label={label}
  data-tip={label}
  {...rest}
>
  <Icon name={icon} size={size === 'lg' ? 27 : 18} />
</button>
