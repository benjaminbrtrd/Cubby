<!-- Top of a side panel: what kind of thing it is, its picture, its name, one line under the name, and one status
     line that never wraps (chips and InspectorStatus). `wide` puts the picture above the name, full width. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    eyebrow,
    title,
    sub,
    mono = false,
    media,
    wide = false,
    meta,
    empty = false,
    big = false,
    titleEl = $bindable(),
  }: {
    eyebrow: string;
    title: string;
    sub?: string | null;
    mono?: boolean;
    media?: Snippet;
    wide?: boolean;
    meta?: Snippet;
    empty?: boolean;
    big?: boolean;
    titleEl?: HTMLElement;
  } = $props();
</script>

<div class="cb-ins-head">
  <p class="cb-ins-eyebrow">{eyebrow}</p>
  {#if wide && media}{@render media()}{/if}
  <div class="cb-ins-who">
    {#if !wide && media}{@render media()}{/if}
    <div class="cb-ins-who__text">
      <h2 class={['cb-ins-title', big && 'cb-ins-title--big', empty && 'cb-ins-title--empty']} tabindex="-1" bind:this={titleEl} {title}>{title}</h2>
      {#if sub}<p class={['cb-ins-sub', mono && 'cb-ins-sub--mono']} title={sub}>{sub}</p>{/if}
    </div>
  </div>
  {#if meta}<div class="cb-ins-meta">{@render meta()}</div>{/if}
</div>
