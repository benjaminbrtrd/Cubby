<script lang="ts">
  import { t } from '../i18n.svelte'
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import type { HotbarItem, HotbarProps } from './types';

  let { items = [], footer, value, onChange, expanded, label, class: className }: HotbarProps = $props();

  const open = $derived(!!expanded);

  function click(e: MouseEvent, id: string) {
    if (onChange) {
      e.preventDefault();
      onChange(id);
    }
  }
</script>

{#snippet slot(it: HotbarItem)}
  {@const on = it.id === value}
  <li>
    <a
      href={it.href || '#' + it.id}
      class={['cb-slot', on && 'is-on']}
      aria-current={on ? 'page' : undefined}
      data-tip={open ? undefined : it.label}
      onclick={(e) => click(e, it.id)}
    >
      <span class="cb-slot__box">
        <Icon name={it.icon} size={28} />
        {#if it.badge != null}<span class="cb-slot__badge"><Content value={it.badge} /></span>{/if}
      </span>
      <span class={open ? 'cb-slot__label text-body-strong' : 'cb-visually-hidden'}>{it.label}</span>
    </a>
  </li>
{/snippet}

<nav class={['cb-hotbar', open && 'is-open', className]} aria-label={label || t('Main navigation')}>
  <ul class="cb-hotbar__list">
    {#each items as it (it.id)}{@render slot(it)}{/each}
  </ul>
  {#if footer && footer.length}
    <ul class="cb-hotbar__list cb-hotbar__foot">
      {#each footer as it (it.id)}{@render slot(it)}{/each}
    </ul>
  {/if}
</nav>
