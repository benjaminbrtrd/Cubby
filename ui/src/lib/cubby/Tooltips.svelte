<!-- One tooltip layer for the whole window. Any element with `data-tip` shows its text on hover and on
     keyboard focus. The tooltip is fixed to the viewport: it never adds scrollbars, flips to the other
     side near an edge and always stays on screen. Side: `data-tip-side` (top, right, bottom, left);
     hotbar slots default to right, icon picker swatches to top, everything else to bottom. Mount it once. -->
<script lang="ts">
  import { onMount } from 'svelte';

  type Side = 'top' | 'right' | 'bottom' | 'left';
  const GAP = 8;
  const MARGIN = 8;

  let text = $state('');
  let open = $state(false);
  let x = $state(0);
  let y = $state(0);
  let tip: HTMLDivElement | undefined = $state();
  let anchor: HTMLElement | null = null;

  function sideOf(el: HTMLElement): Side {
    const s = el.dataset.tipSide;
    if (s === 'top' || s === 'right' || s === 'bottom' || s === 'left') return s;
    if (el.classList.contains('cb-slot')) return 'right';
    if (el.closest('.cb-ipick')) return 'top';
    return 'bottom';
  }

  function place() {
    if (!anchor || !tip) return;
    const a = anchor.getBoundingClientRect();
    const w = tip.offsetWidth;
    const h = tip.offsetHeight;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const fits: Record<Side, boolean> = {
      top: a.top - GAP - h >= MARGIN,
      bottom: a.bottom + GAP + h <= vh - MARGIN,
      left: a.left - GAP - w >= MARGIN,
      right: a.right + GAP + w <= vw - MARGIN,
    };
    const flip: Record<Side, Side> = { top: 'bottom', bottom: 'top', left: 'right', right: 'left' };
    let side = sideOf(anchor);
    if (!fits[side] && fits[flip[side]]) side = flip[side];
    let left: number;
    let top: number;
    if (side === 'top' || side === 'bottom') {
      left = a.left + a.width / 2 - w / 2;
      top = side === 'top' ? a.top - GAP - h : a.bottom + GAP;
    } else {
      left = side === 'left' ? a.left - GAP - w : a.right + GAP;
      top = a.top + a.height / 2 - h / 2;
    }
    x = Math.round(Math.max(MARGIN, Math.min(left, vw - MARGIN - w)));
    y = Math.round(Math.max(MARGIN, Math.min(top, vh - MARGIN - h)));
  }

  function show(el: HTMLElement) {
    const t = el.dataset.tip;
    if (!t) return hide();
    anchor = el;
    text = t;
    open = true;
    // measure with the new text, then place
    requestAnimationFrame(place);
  }

  function hide() {
    anchor = null;
    open = false;
  }

  const target = (e: Event) => (e.target instanceof Element ? e.target.closest<HTMLElement>('[data-tip]') : null);

  onMount(() => {
    const over = (e: PointerEvent) => {
      const el = target(e);
      if (el && el !== anchor) show(el);
      else if (!el && anchor && !(e.target instanceof Node && anchor.contains(e.target))) hide();
    };
    const out = (e: PointerEvent) => {
      if (anchor && !(e.relatedTarget instanceof Node && anchor.contains(e.relatedTarget))) hide();
    };
    const focus = (e: FocusEvent) => {
      const el = target(e);
      if (el && el.matches(':focus-visible')) show(el);
    };
    const blur = () => hide();
    // Focus also leaves when Svelte removes the focused element, and state can't change during that
    // teardown: close a moment later, unless focus already opened the tip of the next element.
    const focusOut = () => {
      const was = anchor;
      queueMicrotask(() => {
        if (anchor === was) hide();
      });
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) hide();
    };
    document.addEventListener('pointerover', over);
    document.addEventListener('pointerout', out);
    document.addEventListener('pointerdown', blur, true);
    document.addEventListener('focusin', focus);
    document.addEventListener('focusout', focusOut);
    document.addEventListener('keydown', key, true);
    window.addEventListener('scroll', blur, true);
    window.addEventListener('resize', blur);
    return () => {
      document.removeEventListener('pointerover', over);
      document.removeEventListener('pointerout', out);
      document.removeEventListener('pointerdown', blur, true);
      document.removeEventListener('focusin', focus);
      document.removeEventListener('focusout', focusOut);
      document.removeEventListener('keydown', key, true);
      window.removeEventListener('scroll', blur, true);
      window.removeEventListener('resize', blur);
    };
  });
</script>

{#if open}
  <div bind:this={tip} class="cb-tooltip" role="presentation" aria-hidden="true" style:left="{x}px" style:top="{y}px">{text}</div>
{/if}
