<script lang="ts" module>
  import type { IconName } from './data';

  const PLAY_TEXT: Record<string, string> = { ready: 'Play', installing: 'Installing', running: 'Stop' };
  const PLAY_ICON: Record<string, IconName> = { ready: 'play', installing: 'download', running: 'stop' };
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import { pct } from './data';
  import type { PlayButtonProps } from './types';

  let {
    state = 'ready',
    size = 'md',
    label,
    detail,
    progress,
    percentText,
    class: className,
    ...rest
  }: PlayButtonProps = $props();

  const value = $derived(state === 'installing' && typeof progress === 'number' ? Math.max(0, Math.min(100, progress)) : null);
</script>

<button
  type="button"
  class={['cb-play', 'cb-play--' + state, 'cb-play--' + size, className]}
  aria-busy={state === 'installing' || undefined}
  {...rest}
>
  <span class="cb-play__icon"><Icon name={PLAY_ICON[state] || 'play'} size={size === 'lg' ? 27 : 18} /></span>
  <span class="cb-play__text">
    <span class="cb-play__label">{label || PLAY_TEXT[state]}</span>
    {#if detail}<span class="cb-play__detail text-small"><Content value={detail} /></span>{/if}
  </span>
  {#if value != null}
    <span class="cb-play__pct text-counter">{percentText || pct(value)}</span>
    <span class="cb-play__fill" style:width="{value}%" aria-hidden="true"></span>
  {/if}
</button>
