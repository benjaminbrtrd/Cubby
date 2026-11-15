<script lang="ts">
  import { t } from '../i18n.svelte'
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import StatusTag from './StatusTag.svelte';
  import SourceMark from './SourceMark.svelte';
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
    source,
    kind,
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
      {#if kind || source}
        <span class="cb-mod__meta">
          {#if kind}<span class="cb-mod__kind" title={kind.label}><Icon name={kind.icon} size={18} /><span class="cb-visually-hidden">{kind.label}</span></span>{/if}
          {#if source}<SourceMark tone={source.tone} label={source.label} />{/if}
        </span>
      {/if}
    </div>
    {#if description}<p class="cb-mod__desc text-small">{description}</p>{/if}
    {#if update || warning}
      <div class="cb-mod__flags">
        {#if update}<StatusTag state="update">{update}</StatusTag>{/if}
        {#if warning}<span class="cb-mod__warning text-small"><Icon name="warning" size={18} />{warning}</span>{/if}
      </div>
    {/if}
  </div>
  {#if actions || onMore}
    <div class="cb-mod__actions">
      {#if actions}
        <Content value={actions} />
      {:else}
        <IconButton icon="more" size="sm" label={moreLabel || t('Actions for ') + name} onclick={onMore} />
      {/if}
    </div>
  {/if}
</div>
