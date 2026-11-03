<script lang="ts" module>
  import type { IconName } from './data';
  import type { Tone } from './types';

  const BANNER_ICON: Record<Tone, IconName> = { danger: 'error', warning: 'warning', info: 'info', success: 'check' };
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import type { BannerProps } from './types';

  let { tone: toneProp, title, children, actions, icon, onDismiss, dismissLabel, class: className }: BannerProps = $props();

  const tone = $derived<Tone>(toneProp && Object.hasOwn(BANNER_ICON, toneProp) ? toneProp : 'info');
</script>

<div class={['cb-banner', 'cb-banner--' + tone, className]} role={tone === 'danger' ? 'alert' : 'status'}>
  <span class="cb-banner__slot" aria-hidden="true"><Icon name={icon || BANNER_ICON[tone]} size={27} /></span>
  <div class="cb-banner__main">
    <div class="cb-banner__body">
      {#if title}<p class="cb-banner__title text-heading"><Content value={title} /></p>{/if}
      {#if children}<div class="cb-banner__text text-body">{@render children()}</div>{/if}
    </div>
    {#if actions}<div class="cb-banner__actions"><Content value={actions} /></div>{/if}
  </div>
  {#if onDismiss}
    <IconButton icon="close" size="sm" label={dismissLabel || 'Dismiss'} onclick={onDismiss} class="cb-banner__dismiss" />
  {/if}
</div>
