<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { tick } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog } from '../../cubby'
  import { locale, num, plural, timeOfDay } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, Screenshot } from '../../types'
  import { when, type InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  /** Newest first (sorted when loaded). */
  const shots = $derived(data.screenshots ?? [])

  let selected = $state<string | null>(null)
  const index = $derived(Math.max(0, shots.findIndex((s) => s.fileName === selected)))
  const cur = $derived(shots[index] ?? null)

  let list: HTMLElement | undefined = $state()
  let viewing = $state<Screenshot | null>(null)
  let deleting = $state<Screenshot | null>(null)
  let busy = $state(false)
  /** Pixel size of the selected picture, read from the image once it loads. */
  let dims = $state<{ file: string; w: number; h: number } | null>(null)

  function dayLabel(iso: string): string {
    const d = new Date(iso)
    const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
    const days = Math.round((start(new Date()) - start(d)) / 86_400_000)
    if (days === 0) return t('Today')
    if (days === 1) return t('Yesterday')
    const sameYear = d.getFullYear() === new Date().getFullYear()
    return d.toLocaleDateString(locale(), { weekday: 'long', month: 'short', day: 'numeric', year: sameYear ? undefined : 'numeric' })
  }

  const groups = $derived.by(() => {
    const out: { key: string; label: string; items: { shot: Screenshot; i: number }[] }[] = []
    shots.forEach((shot, i) => {
      const d = new Date(shot.taken)
      const key = `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
      let g = out[out.length - 1]
      if (!g || g.key !== key) {
        g = { key, label: dayLabel(shot.taken), items: [] }
        out.push(g)
      }
      g.items.push({ shot, i })
    })
    return out
  })

  async function focusAt(i: number) {
    const n = Math.max(0, Math.min(shots.length - 1, i))
    if (!shots[n]) return
    selected = shots[n].fileName
    await tick()
    const el = list?.querySelector<HTMLElement>(`[data-i="${n}"]`)
    el?.focus()
    el?.scrollIntoView({ block: 'nearest' })
  }

  function columns(from: HTMLElement): number {
    const grid = from.closest('.grid')
    if (!grid) return 1
    return getComputedStyle(grid).gridTemplateColumns.split(' ').filter(Boolean).length || 1
  }

  function onkeydown(e: KeyboardEvent) {
    if (!(e.target instanceof HTMLElement) || !e.target.matches('[data-i]')) return
    const cols = columns(e.target)
    const ctrl = e.ctrlKey || e.metaKey
    if (e.key === 'ArrowRight') void focusAt(index + 1)
    else if (e.key === 'ArrowLeft') void focusAt(index - 1)
    else if (e.key === 'ArrowDown') void focusAt(index + cols)
    else if (e.key === 'ArrowUp') void focusAt(index - cols)
    else if (e.key === 'Home') void focusAt(0)
    else if (e.key === 'End') void focusAt(shots.length - 1)
    else if (e.key === 'Enter' && cur) viewing = cur
    else if (e.key === 'Delete' && cur) deleting = cur
    else if (ctrl && e.key.toLowerCase() === 'c' && cur) void copyImage(cur)
    else return
    e.preventDefault()
  }

  async function toPng(blob: Blob): Promise<Blob> {
    const url = URL.createObjectURL(blob)
    try {
      const img = new Image()
      img.src = url
      await img.decode()
      const canvas = document.createElement('canvas')
      canvas.width = img.naturalWidth || 640
      canvas.height = img.naturalHeight || 360
      const ctx = canvas.getContext('2d')
      if (!ctx) throw new Error('No canvas')
      ctx.imageSmoothingEnabled = false
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
      return await new Promise<Blob>((ok, ko) => canvas.toBlob((b) => (b ? ok(b) : ko(new Error(t('No image')))), 'image/png'))
    } finally {
      URL.revokeObjectURL(url)
    }
  }

  async function copyImage(s: Screenshot) {
    try {
      const blob = await (await fetch(s.url)).blob()
      const png = blob.type === 'image/png' ? blob : await toPng(blob)
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': png })])
      app.toast(t('Image copied.'), 'success')
    } catch {
      app.toast(t('Couldn’t copy this image. Open the folder to copy the file instead.'), 'error', { label: t('Open folder'), run: () => data.openFolder('screenshots') })
    }
  }

  async function remove() {
    const s = deleting
    if (!s) return
    busy = true
    const at = shots.findIndex((x) => x.fileName === s.fileName)
    try {
      await api.deleteScreenshot(id, s.fileName)
      deleting = null
      data.screenshots = (data.screenshots ?? []).filter((x) => x.fileName !== s.fileName)
      const next = data.screenshots[Math.min(at, data.screenshots.length - 1)]
      selected = next?.fileName ?? null
      if (viewing?.fileName === s.fileName) viewing = null
      app.toast(t('Screenshot deleted.'), 'success')
      if (next) void focusAt(Math.min(at, shots.length - 1))
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  function measured(e: Event, file: string) {
    const img = e.currentTarget as HTMLImageElement
    if (img.naturalWidth) dims = { file, w: img.naturalWidth, h: img.naturalHeight }
  }

</script>

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label={t('Screenshots')}>
  <div class="toolbar">
    <div class="toolbar__title">
      <h2 class="text-heading">{data.screenshots ? plural(shots.length, 'screenshot', 'screenshots') : t('Screenshots')}</h2>
      <p class="text-small muted">{t('Press F2 in game to take one.')}</p>
    </div>
    <Button icon="folder" onclick={() => data.openFolder('screenshots')}>{t('Open folder')}</Button>
  </div>

  {#if data.screenshots === null}
    <p class="muted text-body">{t('Looking for screenshots…')}</p>
  {:else if !shots.length}
    <div class="empty">
      <p class="text-heading">{t('No screenshots yet')}</p>
      <p class="muted text-body">{t('Press F2 while you play. Your screenshots show up here.')}</p>
    </div>
  {:else}
    <!-- One tab stop on the selected picture, then the arrow keys. -->
    <div class="shots" role="listbox" aria-label={t('Screenshots')} tabindex="-1" bind:this={list} {onkeydown}>
      {#each groups as g (g.key)}
        <div class="day" role="group" aria-labelledby={`day-${g.key}`}>
          <div class="day__head">
            <h3 id={`day-${g.key}`} class="text-label muted">{g.label}</h3>
            <span class="text-counter muted">{num(g.items.length)}</span>
          </div>
          <div class="grid">
            {#each g.items as { shot, i } (shot.fileName)}
              <div class="cell">
                <button
                  type="button"
                  role="option"
                  aria-selected={i === index}
                  aria-label={t('{day} at {time}', { day: g.label, time: timeOfDay(shot.taken) })}
                  tabindex={i === index ? 0 : -1}
                  data-i={i}
                  class={['shot', i === index && 'is-selected']}
                  onclick={() => (selected = shot.fileName)}
                  ondblclick={() => (viewing = shot)}
                >
                  <img src={shot.url} alt="" loading="lazy" decoding="async" />
                </button>
                <span class="text-small muted caption">{timeOfDay(shot.taken)}</span>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<aside class="ip-aside" aria-label={t('Screenshot details')}>
  <div class="ip-aside__inner">
    {#if cur}
      <button type="button" class="preview" aria-label={t('View full size')} onclick={() => (viewing = cur)}>
        <img src={cur.url} alt={t('Screenshot from {true}', { true: when(cur.taken, true) })} onload={(e) => measured(e, cur.fileName)} />
      </button>
      <div class="pager">
        <Button size="sm" variant="ghost" disabled={index === 0} onclick={() => focusAt(index - 1)}>{t('Newer')}</Button>
        <span class="text-counter muted pager__pos">{t('{n} of {n2}', { n: num(index + 1), n2: num(shots.length) })}</span>
        <Button size="sm" variant="ghost" disabled={index === shots.length - 1} onclick={() => focusAt(index + 1)}>{t('Older')}</Button>
      </div>
      <div class="title">
        <h2 class="text-mono wrap">{cur.fileName}</h2>
        <p class="text-small muted">{when(cur.taken)}</p>
      </div>
      {#if dims && dims.file === cur.fileName}
        <dl class="facts">
          <dt class="text-small muted">{t('Resolution')}</dt>
          <dd class="text-body">{num(dims.w)} × {num(dims.h)}</dd>
        </dl>
      {/if}
      <Button variant="primary" icon="copy" class="wide" onclick={() => cur && copyImage(cur)}>{t('Copy image')}</Button>
      <div class="row">
        <Button size="sm" icon="folder" onclick={() => data.openFolder('screenshots')}>{t('Open folder')}</Button>
      </div>
      <div class="danger">
        <Button size="sm" variant="danger" icon="trash" onclick={() => (deleting = cur)}>{t('Delete')}</Button>
        <span class="text-small muted">{t('Deletes the file for good.')}</span>
      </div>
    {:else}
      <p class="text-label muted">{t('Screenshot details')}</p>
      <p class="text-body muted">{t('Pick a screenshot to see it larger, copy it or delete it.')}</p>
    {/if}
  </div>
</aside>

{#if viewing}
  {@const v = viewing}
  {#snippet viewActions()}
    <Button icon="copy" onclick={() => copyImage(v)}>{t('Copy image')}</Button>
    <Button variant="primary" onclick={() => (viewing = null)}>{t('Close')}</Button>
  {/snippet}
  <Dialog wide title={when(v.taken)} onClose={() => (viewing = null)} actions={viewActions}>
    <img class="full" src={v.url} alt={t('Screenshot from {true}', { true: when(v.taken, true) })} />
  </Dialog>
{/if}

{#if deleting}
  {@const s = deleting}
  {#snippet deleteActions()}
    <Button variant="ghost" onclick={() => (deleting = null)}>{t('Cancel')}</Button>
    <Button variant="danger" icon="trash" busy={busy} disabled={busy} onclick={remove}>{t('Delete')}</Button>
  {/snippet}
  <Dialog tone="danger" icon="trash" title={t('Delete this screenshot?')} onClose={() => (deleting = null)} actions={deleteActions}>
    <p>{t('Deletes')} <span class="text-mono">{s.fileName}</span> {t('from the screenshots folder.')}</p>
    <p>{t('It doesn’t go to the trash, so you can’t get it back.')}</p>
  </Dialog>
{/if}

<style>
  p,
  h2,
  h3,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .wrap {
    overflow-wrap: anywhere;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .toolbar__title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-5);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .shots {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    outline: none;
  }
  .day {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .day__head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: var(--space-4);
  }
  .cell {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .shot {
    position: relative;
    display: block;
    width: 100%;
    padding: 0;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: pointer;
  }
  .shot img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .shot.is-selected::after {
    content: '';
    position: absolute;
    inset: -2px;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
  }
  .shot:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .caption {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .preview {
    display: block;
    width: 100%;
    padding: 0;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
    cursor: zoom-in;
  }
  .preview:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring), var(--shadow-block);
  }
  .preview img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .pager {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .pager__pos {
    flex: 1;
    text-align: center;
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .ip-aside__inner :global(.wide) {
    width: 100%;
  }
  .danger {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    margin-top: auto;
    padding-top: var(--space-4);
    border-top: var(--border-hair) solid var(--line);
    text-align: end;
  }
  .full {
    display: block;
    width: 100%;
    max-height: 70vh;
    object-fit: contain;
    image-rendering: pixelated;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
  }
</style>
