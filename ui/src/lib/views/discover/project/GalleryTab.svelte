<!-- Gallery tab: the author's images, and a viewer to see one larger (arrows move, Esc closes). -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Button, Icon, IconButton } from '../../../cubby'
  import { plural } from '../../../format'
  import type { Project } from '../../../types'

  let { project }: { project: Project } = $props()

  let viewing = $state<number | null>(null)
  let viewer: HTMLDivElement | undefined = $state()
  let returnFocus: HTMLElement | null = null

  function openImage(i: number, e: MouseEvent) {
    returnFocus = e.currentTarget as HTMLElement
    viewing = i
  }

  function closeViewer() {
    viewing = null
    returnFocus?.focus()
  }

  function move(by: number) {
    if (viewing === null) return
    const n = project.gallery.length
    viewing = (viewing + by + n) % n
  }

  $effect(() => {
    if (viewing !== null && viewer) viewer.querySelector<HTMLElement>('.viewer-close button, button')?.focus()
  })

  // Only while the viewer is open: Esc closes it, the arrows move, Tab stays inside.
  function onWindowKey(e: KeyboardEvent) {
    if (viewing === null || !viewer) return
    if (e.key === 'Escape') {
      e.preventDefault()
      closeViewer()
    } else if (e.key === 'ArrowLeft' && !e.altKey) {
      e.preventDefault()
      move(-1)
    } else if (e.key === 'ArrowRight' && !e.altKey) {
      e.preventDefault()
      move(1)
    } else if (e.key === 'Tab') {
      const items = Array.from(viewer.querySelectorAll<HTMLElement>('button:not(:disabled)'))
      if (!items.length) return
      const at = items.indexOf(document.activeElement as HTMLElement)
      e.preventDefault()
      items[(at + (e.shiftKey ? -1 : 1) + items.length) % items.length].focus()
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<div id="pp-gallery" role="tabpanel" aria-label={t('Gallery')} class="panel">
  <p class="text-small muted">{t('{n} from the author. Select one to see it larger.', { n: plural(project.gallery.length, 'image', 'images') })}</p>
  <ul class="gallery">
    {#each project.gallery as g, i (g.url + i)}
      <li>
        <button type="button" class="thumb" aria-label={g.title ? t('Open image {i} of {length}: {title}', { i: i + 1, length: project.gallery.length, title: g.title }) : t('Open image {i} of {length}', { i: i + 1, length: project.gallery.length })} onclick={(e) => openImage(i, e)}>
          <img src={g.url} alt="" loading="lazy" />
        </button>
        {#if g.title}<span class="text-small caption">{g.title}</span>{/if}
      </li>
    {/each}
  </ul>
</div>

{#if viewing !== null && project.gallery[viewing]}
  {@const img = project.gallery[viewing]}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="cb-dialog-layer viewer-layer" onclick={(e) => e.target === e.currentTarget && closeViewer()}>
    <div bind:this={viewer} class="viewer" role="dialog" aria-modal="true" aria-label={t('Image {viewing} of {length}', { viewing: viewing + 1, length: project.gallery.length })}>
      <div class="viewer-head">
        <div class="viewer-text">
          <p class="text-body-strong">{img.title ?? project.title}</p>
          {#if img.description}<p class="text-small muted">{img.description}</p>{/if}
        </div>
        <span class="text-counter muted">{viewing + 1} / {project.gallery.length}</span>
        <span class="viewer-close"><IconButton icon="close" label={t('Close')} onclick={closeViewer} /></span>
      </div>
      <div class="viewer-stage">
        <img src={img.url} alt={img.title ?? t('{title}, image {viewing}', { title: project.title, viewing: viewing + 1 })} />
      </div>
      {#if project.gallery.length > 1}
        <div class="viewer-nav">
          <Button size="sm" onclick={() => move(-1)}><span class="flip"><Icon name="chevron-right" size={18} /></span>{t('Previous')}</Button>
          <span class="text-small muted">{t('Use the arrow keys to move, Esc to close.')}</span>
          <Button size="sm" iconEnd="chevron-right" onclick={() => move(1)}>{t('Next')}</Button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .gallery li {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }
  .thumb {
    display: block;
    padding: 0;
    line-height: 0;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: zoom-in;
  }
  .thumb:hover {
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block);
  }
  .thumb:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring), var(--shadow-block-sm);
  }
  .thumb img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .caption {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-muted);
  }

  /* viewer */
  .viewer {
    display: flex;
    flex-direction: column;
    width: min(1100px, 100%);
    max-height: calc(100vh - 48px);
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-float);
  }
  .viewer-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    background: var(--bg-chrome);
    border-bottom: var(--border-block) solid var(--edge);
  }
  .viewer-text {
    flex: 1;
    min-width: 0;
  }
  .viewer-stage {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-3);
    background: var(--field);
  }
  .viewer-stage img {
    display: block;
    max-width: 100%;
    max-height: calc(100vh - 200px);
    object-fit: contain;
    image-rendering: pixelated;
  }
  .viewer-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .flip {
    display: inline-flex;
    transform: scaleX(-1);
  }
</style>
