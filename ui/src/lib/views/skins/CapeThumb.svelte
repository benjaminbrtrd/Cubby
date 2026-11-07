<!-- the back of a cape, or an empty frame for "no cape". -->
<script lang="ts">
  import { drawCapeBack, getCapeImage, type Cape } from '../../skins.svelte'

  let { cape, scale = 4 }: { cape: Cape | null; scale?: number } = $props()
  let canvas: HTMLCanvasElement | undefined = $state()

  $effect(() => {
    const c = canvas
    const cp = cape
    const s = scale
    if (!c) return
    let stale = false
    drawCapeBack(c, null, s)
    if (cp)
      getCapeImage(cp.url)
        .then((img) => {
          if (!stale) drawCapeBack(c, img, s)
        })
        .catch(() => {})
    return () => {
      stale = true
    }
  })
</script>

<canvas bind:this={canvas} class:empty={!cape} class="cape" aria-hidden="true"></canvas>

<style>
  .cape {
    display: block;
    flex: none;
    image-rendering: pixelated;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-sizing: content-box;
  }
  .empty {
    border-style: dashed;
    border-color: var(--line);
  }
</style>
