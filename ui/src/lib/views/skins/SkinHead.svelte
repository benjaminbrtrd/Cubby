<!-- the face of a skin (hat layer included), drawn pixelated. -->
<script lang="ts">
  import { drawHead, getSkin } from '../../skins.svelte'

  let { url, size = 48 }: { url: string; size?: number } = $props()
  let canvas: HTMLCanvasElement | undefined = $state()

  $effect(() => {
    const c = canvas
    const u = url
    const s = size
    if (!c) return
    let stale = false
    drawHead(c, null, s)
    getSkin(u)
      .then((skin) => {
        if (!stale) drawHead(c, skin, s)
      })
      .catch(() => {})
    return () => {
      stale = true
    }
  })
</script>

<canvas bind:this={canvas} class="head" aria-hidden="true"></canvas>

<style>
  .head {
    display: block;
    flex: none;
    image-rendering: pixelated;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-sizing: content-box;
  }
</style>
