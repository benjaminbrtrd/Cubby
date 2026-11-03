<script lang="ts" module>
  import type { IconName, Status } from './data';

  const STATUS: Record<Status, [IconName, string]> = {
    ready: ['check', 'Ready'],
    download: ['download', 'Not installed'],
    installing: ['download', 'Installing'],
    running: ['play', 'Playing'],
    crashed: ['error', 'Crashed'],
    update: ['update', 'Update'],
    online: ['check', 'Online'],
    offline: ['offline', 'Offline'],
    disabled: ['minimize', 'Disabled'],
  };
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import type { StatusTagProps } from './types';

  let { state: stateProp, detail, children, class: className }: StatusTagProps = $props();

  const state = $derived<Status>(Object.hasOwn(STATUS, stateProp) ? stateProp : 'ready');
  const s = $derived(STATUS[state]);
</script>

<span class={['cb-status', 'cb-status--' + state, className]}>
  <Icon name={s[0]} size={18} />
  <span class="cb-status__text text-body">{#if children}{@render children()}{:else}{s[1]}{/if}</span>
  {#if detail}<span class="cb-status__detail text-counter"><Content value={detail} /></span>{/if}
</span>
