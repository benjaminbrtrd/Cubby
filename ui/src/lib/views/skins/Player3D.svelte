<!-- the player in 3D with skinview3d: drag to turn, the cape hangs on the back. -->
<script lang="ts">
  import { IdleAnimation, SkinViewer, WalkingAnimation } from 'skinview3d'
  import type { Model } from '../../skins.svelte'

  interface Props {
    skin: string
    model: Model
    cape: string | null
    walking?: boolean
    width: number
    height: number
    label: string
  }
  let { skin, model, cape, walking = false, width, height, label }: Props = $props()

  let canvas: HTMLCanvasElement | undefined = $state()
  let viewer: SkinViewer | undefined
  let failed = $state(false)

  $effect(() => {
    const c = canvas
    if (!c) return
    const v = new SkinViewer({ canvas: c, width, height, zoom: 0.8, fov: 40 })
    v.controls.enableZoom = false
    v.controls.enablePan = false
    v.autoRotate = true
    v.autoRotateSpeed = 0.6
    viewer = v
    return () => {
      v.dispose()
      viewer = undefined
    }
  })

  $effect(() => {
    const v = viewer
    if (!v) return
    const url = skin
    const m = model === 'slim' ? 'slim' : 'default'
    failed = false
    v.loadSkin(url, { model: m }).catch(() => (failed = true))
  })

  $effect(() => {
    const v = viewer
    if (!v) return
    const url = cape
    if (url) v.loadCape(url, { backEquipment: 'cape' }).catch(() => {})
    else v.resetCape()
  })

  $effect(() => {
    const v = viewer
    if (v) v.animation = walking ? new WalkingAnimation() : new IdleAnimation()
  })

  $effect(() => {
    viewer?.setSize(width, height)
  })
</script>

<canvas bind:this={canvas} class="player" aria-label={label}>{label}</canvas>
{#if failed}<span class="text-small muted">…</span>{/if}

<style>
  .player {
    display: block;
    cursor: grab;
    touch-action: none;
  }
  .player:active {
    cursor: grabbing;
  }
</style>
