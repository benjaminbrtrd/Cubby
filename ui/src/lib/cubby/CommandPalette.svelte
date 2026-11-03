<script lang="ts" module>
  const CMD_FOOT: { keys: string | string[]; label: string }[] = [
    { keys: ['↑', '↓'], label: 'Browse' },
    { keys: 'Enter', label: 'Launch' },
    { keys: ['Ctrl', 'Enter'], label: 'Offline' },
    { keys: 'Tab', label: 'More actions' },
  ];
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import Kbd from './Kbd.svelte';
  import StatusTag from './StatusTag.svelte';
  import type { CommandItem, CommandPaletteProps } from './types';

  let {
    groups = [],
    activeId = $bindable(),
    query = $bindable(''),
    onQuery,
    onSelect,
    onActiveChange,
    onMore,
    onClose,
    placeholder,
    label,
    resultsLabel,
    closeKey,
    footer,
    modal,
    idBase,
    class: className,
  }: CommandPaletteProps = $props();

  const base = $derived(idBase || 'cb-cmd');
  const flat = $derived(groups.flatMap((g) => g.items));

  function setActive(id: string) {
    activeId = id;
    onActiveChange?.(id);
    document.getElementById(base + '-' + id)?.scrollIntoView({ block: 'nearest' });
  }

  function select(item: CommandItem, offline: boolean) {
    onSelect?.(item, { offline });
  }

  // The arrows move the active result without leaving the field; Enter launches it, Ctrl Enter offline,
  // Tab opens its other actions (when onMore is given), Esc closes (when onClose is given).
  function keydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    const i = flat.findIndex((it) => it.id === activeId);
    const active = i >= 0 ? flat[i] : undefined;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!flat.length) return;
      e.preventDefault();
      const step = e.key === 'ArrowDown' ? 1 : -1;
      const to = i < 0 ? (step > 0 ? 0 : flat.length - 1) : (i + step + flat.length) % flat.length;
      setActive(flat[to].id);
    } else if (e.key === 'Enter') {
      if (!active) return;
      e.preventDefault();
      select(active, e.ctrlKey || e.metaKey);
    } else if (e.key === 'Tab' && !e.shiftKey && !e.altKey && !e.ctrlKey && !e.metaKey) {
      if (!onMore || !active) return;
      e.preventDefault();
      onMore(active);
    } else if (e.key === 'Escape') {
      if (!onClose) return;
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }

  function input(e: Event & { currentTarget: HTMLInputElement }) {
    query = e.currentTarget.value;
    onQuery?.(query);
  }
</script>

<div
  class={['cb-cmd', className]}
  role="dialog"
  aria-modal={modal ? true : undefined}
  aria-label={label || 'Search and commands'}
>
  <div class="cb-cmd__search">
    <Icon name="search" size={18} />
    <input
      class="cb-cmd__input"
      role="combobox"
      aria-expanded={true}
      aria-controls={base + '-list'}
      aria-autocomplete="list"
      aria-activedescendant={activeId ? base + '-' + activeId : undefined}
      aria-label={label || 'Search and commands'}
      placeholder={placeholder || 'Search instances, servers, actions…'}
      value={query || ''}
      oninput={input}
      onkeydown={keydown}
    />
    {#if closeKey !== ''}<Kbd keys={closeKey || 'Esc'} />{/if}
  </div>
  <div class="cb-cmd__list" id={base + '-list'} role="listbox" aria-label={resultsLabel || 'Results'}>
    {#each groups as g, gi (gi)}
      <div role="group" aria-labelledby={base + '-g' + gi} class="cb-cmd__group">
        <div id={base + '-g' + gi} class="cb-cmd__group-label text-label">{g.label}</div>
        {#each g.items as it (it.id)}
          {@const act = it.id === activeId}
          <!-- Options are reached from the field (aria-activedescendant), so they take no focus; a mouse press keeps it in the field. -->
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
          <div
            id={base + '-' + it.id}
            role="option"
            aria-selected={act}
            class={['cb-cmd__item', act && 'is-active']}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => select(it, false)}
          >
            <span class="cb-cmd__item-icon">
              {#if it.art}<Content value={it.art} />{:else}<Icon name={it.icon || 'chevron-right'} size={18} />{/if}
            </span>
            <span class="cb-cmd__item-text">
              <span class="cb-cmd__item-label text-body-strong">{it.label}</span>
              {#if it.detail}<span class="cb-cmd__item-detail text-mono">{it.detail}</span>{/if}
            </span>
            {#if it.status}<StatusTag state={it.status} />{/if}
            {#if act && it.hint}<span class="cb-cmd__item-hint text-small">{it.hint}</span>{/if}
            {#if it.kbd}<Kbd keys={it.kbd} />{/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>
  {#if (footer || CMD_FOOT).length}
    <div class="cb-cmd__foot">
      {#each footer || CMD_FOOT as f, i (i)}
        <span class="cb-cmd__hint text-small"><Kbd keys={f.keys} />{f.label}</span>
      {/each}
    </div>
  {/if}
</div>
