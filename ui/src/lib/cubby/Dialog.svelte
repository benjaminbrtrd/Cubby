<script lang="ts" module>
  // Open modal dialogs, innermost last: only the top one answers Esc and keeps Tab inside.
  const stack: symbol[] = [];

  const FOCUSABLE =
    'a[href], button, input:not([type="hidden"]), select, textarea, [tabindex], [contenteditable]:not([contenteditable="false"])';

  function tabbable(root: ParentNode, selector = FOCUSABLE): HTMLElement[] {
    return Array.from(root.querySelectorAll<HTMLElement>(selector)).filter(
      (n) => n.tabIndex >= 0 && !n.matches(':disabled') && n.getClientRects().length > 0,
    );
  }
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import Icon from './Icon.svelte';
  import IconButton from './IconButton.svelte';
  import type { DialogProps } from './types';

  let { title, children, actions, icon, tone, onClose, closeLabel, inline, wide }: DialogProps = $props();

  const tid = $props.id();
  let dialog: HTMLDivElement | undefined = $state();

  // Modal (not inline): focus goes to the first field, else the least destructive action (actions list Cancel first),
  // else the first control; Tab and Shift Tab stay inside; Esc calls onClose; focus returns where it was on close.
  $effect(() => {
    const root = dialog;
    if (inline || !root) return;
    const me = Symbol('dialog');
    stack.push(me);
    const back = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const start =
      tabbable(root, '.cb-dialog__body input:not([type="hidden"]), .cb-dialog__body select, .cb-dialog__body textarea')[0] ||
      tabbable(root.querySelector('.cb-dialog__actions') || root)[0] ||
      tabbable(root)[0];
    start?.focus();

    function keydown(e: KeyboardEvent) {
      if (stack[stack.length - 1] !== me || !root) return;
      if (e.key === 'Escape') {
        if (!onClose) return;
        e.preventDefault();
        onClose();
      } else if (e.key === 'Tab') {
        const items = tabbable(root);
        if (!items.length) {
          e.preventDefault();
          return;
        }
        const first = items[0];
        const last = items[items.length - 1];
        const active = document.activeElement;
        const inside = active instanceof Node && root.contains(active);
        if (e.shiftKey && (active === first || !inside)) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && (active === last || !inside)) {
          e.preventDefault();
          first.focus();
        }
      }
    }

    window.addEventListener('keydown', keydown);
    return () => {
      window.removeEventListener('keydown', keydown);
      const at = stack.indexOf(me);
      if (at >= 0) stack.splice(at, 1);
      if (back && back.isConnected) back.focus();
    };
  });
</script>

<div class={['cb-dialog-layer', inline && 'is-inline']}>
  <div
    bind:this={dialog}
    class={['cb-dialog', wide && 'cb-dialog--wide']}
    role={tone === 'danger' ? 'alertdialog' : 'dialog'}
    aria-modal={inline ? undefined : true}
    aria-labelledby={tid}
  >
    <div class="cb-dialog__head">
      {#if icon}
        <span class={['cb-dialog__slot', tone && 'cb-dialog__slot--' + tone]} aria-hidden="true"><Icon name={icon} size={18} /></span>
      {/if}
      <h2 id={tid} class="cb-dialog__title text-title"><Content value={title} /></h2>
      {#if onClose}<IconButton icon="close" size="sm" label={closeLabel || 'Close'} onclick={onClose} />{/if}
    </div>
    <div class="cb-dialog__body text-body">{@render children?.()}</div>
    {#if actions}<div class="cb-dialog__actions"><Content value={actions} /></div>{/if}
  </div>
</div>
