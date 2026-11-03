<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import StatusTag from './StatusTag.svelte';
  import Switch from './Switch.svelte';
  import type { ModRowProps } from './types';

  let {
    name,
    version,
    authors,
    description,
    iconSrc,
    enabled = $bindable(false),
    onToggle,
    toggleLabel,
    update,
    warning,
    actions,
    moreLabel,
    onMore,
    class: className,
  }: ModRowProps = $props();

  const on = $derived(!!enabled);

  function toggle(v: boolean) {
    enabled = v;
    onToggle?.(v);
  }
</script>

<div class={['cb-mod', !on && 'is-off', className]}>
  <Switch checked={on} onChange={toggle} label={toggleLabel || name} hideLabel />
  <span class="cb-mod__icon">{#if iconSrc}<img src={iconSrc} alt="" />{:else}<Icon name="mods" size={18} />{/if}</span>
  <div class="cb-mod__body">
    <div class="cb-mod__head">
      <span class="cb-mod__name text-body-strong">{name}</span>
      {#if version}<span class="cb-mod__version text-mono">{version}</span>{/if}
      {#if authors}<span class="cb-mod__authors text-small">{authors}</span>{/if}
    </div>
    {#if description}<p class="cb-mod__desc text-small">{description}</p>{/if}
    {#if update || warning}
      <div class="cb-mod__flags">
        {#if update}<StatusTag state="update">{update}</StatusTag>{/if}
        {#if warning}<span class="cb-mod__warning text-small"><Icon name="warning" size={18} />{warning}</span>{/if}
      </div>
    {/if}
  </div>
  <div class="cb-mod__actions">
    {#if actions}
      <Content value={actions} />
    {:else}
      <IconButton icon="more" size="sm" label={moreLabel || 'Actions for ' + name} onclick={onMore} />
    {/if}
  </div>
</div>
