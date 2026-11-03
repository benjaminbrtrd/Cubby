<script lang="ts">
  import type { LogoProps } from './types';

  let { size, wordmark, label, class: className }: LogoProps = $props();

  const markSize = $derived(Math.max(16, size ?? 32));
  const wordHeight = $derived(markSize * 0.625);
</script>

<span
  class={['cb-logo', className]}
  style:--cb-logo-gap={`${Math.max(8, markSize * 0.2)}px`}
  role="img"
  aria-label={label || 'Cubby'}
>
  <img
    class="cb-logo__mark"
    src="/branding/cubby-128.png"
    srcset="/branding/cubby-32.png 32w, /branding/cubby-64.png 64w, /branding/cubby-128.png 128w, /branding/cubby-256.png 256w, /branding/cubby-512.png 512w, /branding/cubby-1024.png 1024w"
    sizes={`${markSize}px`}
    width={markSize}
    height={markSize}
    alt=""
    aria-hidden="true"
    draggable="false"
  />
  {#if wordmark !== false}
    <span
      class="cb-logo__word"
      style:width={`${wordHeight * 676 / 253}px`}
      style:height={`${wordHeight}px`}
      aria-hidden="true"
    ></span>
  {/if}
</span>
