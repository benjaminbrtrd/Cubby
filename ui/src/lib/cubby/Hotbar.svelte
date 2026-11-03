<script lang="ts">
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

{#snippet slot(it: HotbarItem, n: number)}
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
        <Icon name={it.icon} size={27} />
        {#if n}<span class="cb-slot__num" aria-hidden="true">{n}</span>{/if}
        {#if it.badge != null}<span class="cb-slot__badge"><Content value={it.badge} /></span>{/if}
      </span>
      <span class={open ? 'cb-slot__label text-body-strong' : 'cb-visually-hidden'}>{it.label}</span>
    </a>
  </li>
{/snippet}

<nav class={['cb-hotbar', open && 'is-open', className]} aria-label={label || 'Main navigation'}>
  <ul class="cb-hotbar__list">
    {#each items as it, i (it.id)}{@render slot(it, i + 1)}{/each}
  </ul>
  {#if footer && footer.length}
    <ul class="cb-hotbar__list cb-hotbar__foot">
      {#each footer as it (it.id)}{@render slot(it, 0)}{/each}
    </ul>
  {/if}
</nav>
