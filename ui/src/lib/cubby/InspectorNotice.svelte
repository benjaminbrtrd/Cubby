<!-- A warning or a note inside a side panel, with an optional action under the text. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let { tone = 'warn', children, action }: { tone?: 'warn' | 'info' | 'bad'; children: Snippet; action?: Snippet } = $props();
  const icon = $derived(tone === 'info' ? 'info' : tone === 'bad' ? 'error' : 'warning');
</script>

<div class={['cb-ins-notice', tone !== 'warn' && `cb-ins-notice--${tone}`]} role={tone === 'info' ? 'note' : 'status'}>
  <span class="cb-ins-notice__icon"><Icon name={icon} size={18} /></span>
  <div class="cb-ins-notice__body">
    <span>{@render children()}</span>
    {#if action}{@render action()}{/if}
  </div>
</div>
