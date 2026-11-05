<script lang="ts">
  import { t } from '../i18n.svelte'
  import Avatar from './Avatar.svelte';
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import Kbd from './Kbd.svelte';
  import Logo from './Logo.svelte';
  import type { TitleBarProps } from './types';

  let {
    account,
    onAccount,
    onSearch,
    searchLabel,
    searchKeys,
    labels = {},
    onMinimize,
    onMaximize,
    onClose,
    children,
    class: className,
  }: TitleBarProps = $props();
</script>

<!-- The whole bar is a Tauri drag region; its buttons stay clickable. -->
<header class={['cb-titlebar', className]} data-tauri-drag-region="">
  <div class="cb-titlebar__brand" data-tauri-drag-region=""><Logo size={32} /></div>
  <button type="button" class="cb-titlebar__search text-body" onclick={onSearch}>
    <Icon name="search" size={18} />
    <span class="cb-titlebar__search-text">{searchLabel || t('Search or launch…')}</span>
    {#if searchKeys?.length}<Kbd keys={searchKeys} />{/if}
  </button>
  {#if children}<div class="cb-titlebar__extra">{@render children()}</div>{/if}
  {#if account}
    <button type="button" class="cb-titlebar__account text-body-strong" onclick={onAccount} aria-label={account.label}>
      {#if account.art}<Content value={account.art} />{:else}<Avatar src={account.src} name={account.name} size={24} />{/if}
      <span class="cb-keep-case">{account.name}</span>
    </button>
  {/if}
  <div class="cb-titlebar__win">
    <button type="button" class="cb-win" aria-label={labels.minimize || 'Minimize'} onclick={onMinimize}><Icon name="minimize" size={16} /></button>
    <button type="button" class="cb-win" aria-label={labels.maximize || 'Maximize'} onclick={onMaximize}><Icon name="maximize" size={16} /></button>
    <button type="button" class="cb-win cb-win--close" aria-label={labels.close || 'Close'} onclick={onClose}><Icon name="close" size={16} /></button>
  </div>
</header>
