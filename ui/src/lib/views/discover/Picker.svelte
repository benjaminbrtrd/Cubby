<!-- A button that opens a list to pick one item (instance, version). The list floats above the page
     (fixed position, so dialogs and scrolling panels don't cut it); arrows, Home, End, Enter, Esc. -->
<script lang="ts" generics="T extends { id: string; group?: string; disabled?: boolean }">
  import type { Snippet } from 'svelte'
  import { Icon } from '../../cubby'

  interface Props {
    items: T[]
    value: string | null
    onSelect: (id: string) => void
    /** Accessible name of the list ("Install to", "Version"). */
    label: string
    /** Full accessible name of the button, e.g. "Install to Friends SMP. Change instance". */
    buttonLabel?: string
    /** Content of the button. */
    trigger: Snippet
    /** Content of an option. */
    option: Snippet<[T, boolean]>
    /** 'field' = a form well (in panels), 'block' = a raised block (in toolbars), 'ghost' = text on a tab bar or a header. */
    variant?: 'field' | 'block' | 'ghost'
    disabled?: boolean
    /** Min width of the list, in px. */
    listWidth?: number
  }

  let { items, value, onSelect, label, buttonLabel, trigger, option, variant = 'field', disabled, listWidth = 300 }: Props = $props()

  const uid = $props.id()
  let open = $state(false)
  let active = $state(0)
  let button: HTMLButtonElement | undefined = $state()
  let list: HTMLUListElement | undefined = $state()
  let pos = $state({ left: 0, top: 0, width: 300, maxHeight: 360 })

  function show() {
    if (disabled || !items.length) return
    const at = items.findIndex((i) => i.id === value)
    active = at >= 0 ? at : Math.max(0, items.findIndex((i) => !i.disabled))
    place()
    open = true
  }

  function close(refocus = true) {
    open = false
    if (refocus) button?.focus()
  }

  function place() {
    if (!button) return
    const r = button.getBoundingClientRect()
    const width = Math.max(r.width, listWidth)
    const left = Math.max(8, Math.min(r.left, window.innerWidth - width - 8))
    const below = window.innerHeight - r.bottom - 12
    const above = r.top - 12
    if (below >= 220 || below >= above) pos = { left, top: r.bottom + 4, width, maxHeight: Math.max(160, Math.min(360, below)) }
    else {
      const maxHeight = Math.max(160, Math.min(360, above))
      pos = { left, top: Math.max(8, r.top - 4 - maxHeight), width, maxHeight }
    }
  }

  function pick(i: number) {
    const item = items[i]
    if (!item || item.disabled) return
    close()
    if (item.id !== value) onSelect(item.id)
  }

  function step(from: number, by: number): number {
    for (let n = 1; n <= items.length; n++) {
      const i = (from + by * n + items.length * n) % items.length
      if (!items[i].disabled) return i
    }
    return from
  }

  function onListKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = step(active, 1)
    else if (e.key === 'ArrowUp') active = step(active, -1)
    else if (e.key === 'Home') active = step(-1, 1)
    else if (e.key === 'End') active = step(items.length, -1)
    else if (e.key === 'Enter' || e.key === ' ') pick(active)
    else if (e.key === 'Escape') {
      // keep a surrounding dialog open
      e.stopPropagation()
      close()
    } else if (e.key === 'Tab') {
      close(false)
      return
    } else return
    e.preventDefault()
  }

  function onButtonKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      show()
    }
  }

  function onListClick(e: MouseEvent) {
    const li = (e.target as Element).closest<HTMLElement>('[data-index]')
    if (li) pick(Number(li.dataset.index))
  }

  function onListPointer(e: PointerEvent) {
    const li = (e.target as Element).closest<HTMLElement>('[data-index]')
    if (li && !items[Number(li.dataset.index)]?.disabled) active = Number(li.dataset.index)
  }

  // While open: focus the list, keep the active option in view, close on outside press, scroll or resize.
  $effect(() => {
    if (!open || !list) return
    const el = list
    el.focus({ preventScroll: true })
    const outside = (e: PointerEvent) => {
      if (!el.contains(e.target as Node) && !button?.contains(e.target as Node)) close(false)
    }
    const away = (e: Event) => {
      if (!el.contains(e.target as Node)) close(false)
    }
    const resize = () => close(false)
    window.addEventListener('pointerdown', outside, true)
    window.addEventListener('scroll', away, true)
    window.addEventListener('resize', resize)
    return () => {
      window.removeEventListener('pointerdown', outside, true)
      window.removeEventListener('scroll', away, true)
      window.removeEventListener('resize', resize)
    }
  })

  $effect(() => {
    if (!open || !list) return
    // by hand: scrollIntoView could also scroll the page under the list
    const el = list.querySelector<HTMLElement>(`[data-index="${active}"]`)
    if (!el) return
    if (el.offsetTop < list.scrollTop) list.scrollTop = el.offsetTop - 4
    else if (el.offsetTop + el.offsetHeight > list.scrollTop + list.clientHeight) list.scrollTop = el.offsetTop + el.offsetHeight - list.clientHeight + 4
  })
</script>

<button
  bind:this={button}
  type="button"
  class={['trigger', variant]}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={open ? `${uid}-list` : undefined}
  aria-label={buttonLabel}
  {disabled}
  onclick={() => (open ? close() : show())}
  onkeydown={onButtonKey}
>
  <span class="content">{@render trigger()}</span>
  <Icon name="chevron-down" size={18} />
</button>

{#if open}
  <ul
    bind:this={list}
    id="{uid}-list"
    class="list"
    role="listbox"
    tabindex="-1"
    aria-label={label}
    aria-activedescendant="{uid}-opt-{active}"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:width="{pos.width}px"
    style:max-height="{pos.maxHeight}px"
    onkeydown={onListKey}
    onclick={onListClick}
    onpointermove={onListPointer}
  >
    {#each items as item, i (item.id)}
      {#if item.group && item.group !== items[i - 1]?.group}
        <li class="group text-label" role="presentation">{item.group}</li>
      {/if}
      <li
        id="{uid}-opt-{i}"
        class={['opt', i === active && 'is-active', item.id === value && 'is-on']}
        role="option"
        aria-selected={item.id === value}
        aria-disabled={item.disabled || undefined}
        data-index={i}
      >
        <span class="mark" aria-hidden="true">{#if item.id === value}<Icon name="check" size={18} />{/if}</span>
        <span class="body">{@render option(item, item.id === value)}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .trigger {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    min-height: var(--size-control-lg);
    padding: var(--space-1) var(--space-3) var(--space-1) var(--space-1);
    font: inherit;
    text-align: start;
    color: var(--ink);
    cursor: pointer;
  }
  .trigger.field {
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .trigger.block {
    width: auto;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .trigger.block:hover {
    background: var(--surface-hover);
  }
  .trigger.ghost {
    width: auto;
    min-height: var(--size-control);
    padding: 0 var(--space-2) 0 var(--space-3);
    gap: var(--space-2);
    color: var(--ink-muted);
    background: transparent;
    border: 0;
  }
  .trigger.ghost:hover,
  .trigger.ghost[aria-expanded='true'] {
    color: var(--ink);
    background: var(--surface);
  }
  .trigger:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .trigger:disabled {
    cursor: not-allowed;
    color: var(--ink-disabled);
    border-style: dashed;
    box-shadow: none;
  }
  .content {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex: 1;
    min-width: 0;
  }
  .list {
    position: fixed;
    z-index: var(--layer-menu);
    margin: 0;
    padding: var(--space-1) 0;
    overflow: auto;
    list-style: none;
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-float);
    outline: none;
  }
  .list:focus-visible {
    box-shadow: var(--focus-ring), var(--shadow-float);
  }
  .group {
    margin: var(--space-2) 0 var(--space-1);
    padding: 0 var(--space-3);
    color: var(--ink-muted);
  }
  .group:first-child {
    margin-top: var(--space-1);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--size-control);
    padding: var(--space-1) var(--space-3) var(--space-1) var(--space-2);
    cursor: pointer;
    border: var(--border-block) solid transparent;
  }
  .opt.is-active {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .opt[aria-disabled='true'] {
    cursor: not-allowed;
    color: var(--ink-disabled);
  }
  .mark {
    display: flex;
    flex: none;
    width: 18px;
    color: var(--grass-text);
  }
  .body {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex: 1;
    min-width: 0;
  }
</style>
