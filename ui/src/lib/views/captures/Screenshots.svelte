<script lang="ts">
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { Button, Dialog, Icon, InstanceIcon, Tabs, type TabItem } from '../../cubby'
  import { LOCALE, date, num, plural, timeOfDay } from '../../format'
  import { iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Instance } from '../../types'

  interface Shot {
    key: string
    instanceId: string
    fileName: string
    url: string
    taken: string
    time: number
  }

  let shots = $state<Shot[] | null>(null)
  let tab = $state('all')
  let selectedKey = $state<string | null>(null)
  let generation = 0

  async function load() {
    const gen = ++generation
    const lists = await Promise.all(
      app.instances.map((inst) =>
        api.listScreenshots(inst.id).then(
          (list) => ({ inst, list }),
          () => ({ inst, list: [] }),
        ),
      ),
    )
    if (gen !== generation) return
    shots = lists
      .flatMap(({ inst, list }) =>
        list.map((s) => ({
          key: `${inst.id}/${s.fileName}`,
          instanceId: inst.id,
          fileName: s.fileName,
          url: s.url,
          taken: s.taken,
          time: new Date(s.taken).getTime() || 0,
        })),
      )
      .sort((a, b) => b.time - a.time || b.fileName.localeCompare(a.fileName))
  }

  const instanceIds = $derived(app.instances.map((i) => i.id).join(','))
  $effect(() => {
    void instanceIds
    untrack(() => void load())
  })

  function nameOf(id: string): string {
    return app.instance(id)?.name ?? id
  }

  // Tabs by instance, with counts
  const counts = $derived.by(() => {
    const map = new Map<string, number>()
    for (const s of shots ?? []) map.set(s.instanceId, (map.get(s.instanceId) ?? 0) + 1)
    return map
  })
  const tabs = $derived<TabItem[]>([
    { id: 'all', label: 'All', count: num(shots?.length ?? 0) },
    ...[...counts.entries()]
      .sort((a, b) => b[1] - a[1] || nameOf(a[0]).localeCompare(nameOf(b[0])))
      .map(([id, n]) => ({ id, label: nameOf(id), count: num(n) })),
  ])
  const activeTab = $derived(tabs.some((t) => t.id === tab) ? tab : 'all')
  const list = $derived(activeTab === 'all' ? (shots ?? []) : (shots ?? []).filter((s) => s.instanceId === activeTab))
  const index = $derived(Math.max(0, list.findIndex((s) => s.key === selectedKey)))
  const cur = $derived(list[index] ?? null)
  const curInstance = $derived(app.instance(cur?.instanceId))

  // Grouped by day: Today, Yesterday, then dates
  function dayStart(t: number): number {
    const d = new Date(t)
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  }

  function dayLabel(t: number): string {
    const diff = Math.round((dayStart(Date.now()) - dayStart(t)) / 86_400_000)
    if (diff === 0) return 'Today'
    if (diff === 1) return 'Yesterday'
    return date(new Date(t).toISOString())
  }

  function weekday(t: number): string {
    return new Date(t).toLocaleDateString(LOCALE, { weekday: 'long', month: 'short', day: 'numeric' })
  }

  function takenLabel(t: number): string {
    const label = dayLabel(t)
    const time = timeOfDay(new Date(t).toISOString())
    return label === 'Today' || label === 'Yesterday' ? `${label} at ${time}` : `${weekday(t)} at ${time}`
  }

  const groups = $derived.by(() => {
    const out: { day: number; label: string; date: string; items: { shot: Shot; idx: number }[] }[] = []
    list.forEach((shot, idx) => {
      const day = dayStart(shot.time)
      let g = out[out.length - 1]
      if (!g || g.day !== day) {
        g = { day, label: dayLabel(shot.time), date: weekday(shot.time), items: [] }
        out.push(g)
      }
      g.items.push({ shot, idx })
    })
    return out
  })

  // Selection and keyboard
  let gridsEl: HTMLElement | undefined = $state()

  function thumbs(): HTMLButtonElement[] {
    return [...(gridsEl?.querySelectorAll<HTMLButtonElement>('.thumb') ?? [])]
  }

  async function select(i: number, focus = true) {
    if (!list.length) return
    const at = Math.max(0, Math.min(list.length - 1, i))
    selectedKey = list[at].key
    if (!focus) return
    await tick()
    const el = thumbs()[at]
    el?.focus()
    el?.scrollIntoView({ block: 'nearest' })
  }

  /** Up and Down go to the thumbnail above or below, across day groups. */
  function vertical(i: number, dir: 1 | -1): number {
    const all = thumbs()
    const from = all[i]?.getBoundingClientRect()
    if (!from) return i
    const cx = from.left + from.width / 2
    let row: number | null = null
    let best = i
    let bestDx = Infinity
    for (let j = dir > 0 ? i + 1 : i - 1; j >= 0 && j < all.length; j += dir) {
      const r = all[j].getBoundingClientRect()
      const sameRow = Math.abs(r.top - from.top) < 4
      if (sameRow) continue
      if (row == null) row = r.top
      else if (Math.abs(r.top - row) >= 4) break
      const dx = Math.abs(r.left + r.width / 2 - cx)
      if (dx < bestDx) {
        bestDx = dx
        best = j
      }
    }
    return best
  }

  function onThumbKey(e: KeyboardEvent, i: number) {
    const ctrl = e.ctrlKey || e.metaKey
    if (ctrl && e.key.toLowerCase() === 'c') {
      e.preventDefault()
      if (cur) void copyImage(cur)
      return
    }
    if (ctrl || e.altKey) return
    let to: number | null = null
    if (e.key === 'ArrowRight') to = i + 1
    else if (e.key === 'ArrowLeft') to = i - 1
    else if (e.key === 'ArrowDown') to = vertical(i, 1)
    else if (e.key === 'ArrowUp') to = vertical(i, -1)
    else if (e.key === 'Home') to = 0
    else if (e.key === 'End') to = list.length - 1
    else if (e.key === 'Enter') {
      e.preventDefault()
      viewing = true
      return
    } else if (e.key === 'Delete') {
      e.preventDefault()
      if (cur) confirmDelete = cur
      return
    }
    if (to != null) {
      e.preventDefault()
      void select(to)
    }
  }

  // Large image
  let resolution = $state<string | null>(null)
  let viewing = $state(false)

  function onLoad(e: Event) {
    const img = e.currentTarget as HTMLImageElement
    resolution = img.naturalWidth ? `${num(img.naturalWidth)} × ${num(img.naturalHeight)}` : null
  }

  $effect(() => {
    void cur?.key
    resolution = null
  })

  // Actions
  async function copyImage(s: Shot) {
    try {
      const png = new Promise<Blob>((resolve, reject) => {
        const img = new Image()
        img.crossOrigin = 'anonymous'
        img.onload = () => {
          const canvas = document.createElement('canvas')
          canvas.width = img.naturalWidth
          canvas.height = img.naturalHeight
          const ctx = canvas.getContext('2d')
          if (!ctx) return reject(new Error('No canvas'))
          ctx.drawImage(img, 0, 0)
          canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('No image'))), 'image/png')
        }
        img.onerror = () => reject(new Error('Not loaded'))
        img.src = s.url
      })
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': png })])
      app.toast('Image copied to the clipboard.', 'success')
    } catch {
      app.toast('Couldn’t copy the image. Open the folder and copy the file from there.', 'error')
    }
  }

  function openFolder(instanceId: string) {
    api.openFolder(instanceId, 'screenshots').catch((e) => app.error(e))
  }

  let settingIcon = $state(false)

  async function setIcon(s: Shot) {
    const inst = app.instance(s.instanceId)
    if (!inst) return
    settingIcon = true
    const current = $state.snapshot(inst) as Instance
    const saved = await app.updateInstance({ ...current, icon: s.url })
    settingIcon = false
    if (!saved) return
    if (saved.icon === s.url) app.toast(`This screenshot is now the icon of ${inst.name}.`, 'success')
    else app.toast(`Cubby couldn’t use this screenshot as the icon of ${inst.name}. Its icon didn’t change.`, 'error')
  }

  let confirmDelete = $state<Shot | null>(null)
  let deleting = $state(false)

  async function remove(s: Shot) {
    deleting = true
    try {
      await api.deleteScreenshot(s.instanceId, s.fileName)
      const at = list.findIndex((x) => x.key === s.key)
      shots = (shots ?? []).filter((x) => x.key !== s.key)
      confirmDelete = null
      app.toast('Screenshot deleted.', 'success')
      const next = list[Math.min(at, list.length - 1)]
      selectedKey = next?.key ?? null
      if (next) void select(Math.min(at, list.length - 1))
    } catch (e) {
      app.error(e)
    } finally {
      deleting = false
    }
  }
</script>

{#snippet deleteActions()}
  <Button variant="ghost" onclick={() => (confirmDelete = null)}>Cancel</Button>
  <Button variant="danger" icon="trash" busy={deleting} disabled={deleting} onclick={() => confirmDelete && remove(confirmDelete)}>Delete</Button>
{/snippet}

{#snippet viewTitle()}<span class="text-mono">{cur?.fileName}</span>{/snippet}

<div class="screen">
  <div class="layout">
    <div class="content">
      <div class="head">
        <div class="titles">
          <h1 class="text-title">Screenshots</h1>
          <p class="text-small muted">Press F2 in game to take one.</p>
        </div>
        <span class="grow"></span>
        {#if activeTab !== 'all'}
          <Button icon="folder" onclick={() => openFolder(activeTab)}>Open folder</Button>
        {/if}
      </div>

      {#if shots == null}
        <p class="text-small muted">Looking for screenshots…</p>
      {:else if !shots.length}
        <div class="card empty">
          <Icon name="captures" size={27} />
          <div class="grow">
            <p class="text-body-strong">No screenshots yet</p>
            <p class="text-small muted">Press F2 while you play. Your screenshots from every instance show up here.</p>
          </div>
          <Button icon="library" onclick={() => app.navigate({ name: 'library' })}>Go to the library</Button>
        </div>
      {:else}
        <Tabs label="Instances" items={tabs} value={activeTab} onChange={(v) => (tab = v)} />
        <p class="text-small muted">
          Showing {plural(list.length, 'screenshot', 'screenshots')}{activeTab === 'all' && counts.size > 1 ? ` from ${plural(counts.size, 'instance', 'instances')}` : ''}
        </p>

        <div class="groups" bind:this={gridsEl}>
          {#each groups as g (g.day)}
            <section class="group" aria-label={`${g.label}, ${plural(g.items.length, 'screenshot', 'screenshots')}`}>
              <div class="group-head">
                <h2 class="text-label muted">{g.label}</h2>
                <span class="text-counter muted">{num(g.items.length)}</span>
                <span class="text-small muted when">{g.date}</span>
              </div>
              <div class="grid">
                {#each g.items as { shot, idx } (shot.key)}
                  {@const on = cur?.key === shot.key}
                  <div class="cell">
                    <button
                      type="button"
                      class={['thumb', on && 'is-selected']}
                      tabindex={on ? 0 : -1}
                      aria-pressed={on}
                      aria-label={`${nameOf(shot.instanceId)}, ${takenLabel(shot.time)}`}
                      onclick={() => select(idx, false)}
                      ondblclick={() => (viewing = true)}
                      onkeydown={(e) => onThumbKey(e, idx)}
                    >
                      <img src={shot.url} alt="" loading="lazy" />
                    </button>
                    <span class="caption text-small muted">
                      {timeOfDay(shot.taken)}{activeTab === 'all' ? ` · ${nameOf(shot.instanceId)}` : ''}
                    </span>
                  </div>
                {/each}
              </div>
            </section>
          {/each}
        </div>
      {/if}
    </div>

    <aside class="aside" aria-label="Screenshot details">
      <div class="aside-inner">
        {#if cur}
          <div class="big">
            <img src={cur.url} alt={`Screenshot from ${nameOf(cur.instanceId)}, ${takenLabel(cur.time)}`} onload={onLoad} />
          </div>
          <div class="nav">
            <Button size="sm" variant="ghost" disabled={index === 0} onclick={() => select(index - 1, false)}>Newer</Button>
            <span class="text-counter muted pos">{num(index + 1)} of {num(list.length)}</span>
            <Button size="sm" variant="ghost" disabled={index >= list.length - 1} onclick={() => select(index + 1, false)}>Older</Button>
          </div>

          <div class="file">
            <h2 class="text-mono name">{cur.fileName}</h2>
            <p class="text-small muted">{takenLabel(cur.time)}</p>
          </div>

          <dl class="facts">
            <dt class="text-small muted">Instance</dt>
            <dd class="dd-row">
              {#if curInstance}<InstanceIcon {...iconOf(curInstance)} seed={curInstance.name} size={32} />{/if}
              <span class="text-body">{nameOf(cur.instanceId)}</span>
            </dd>
            {#if resolution}
              <dt class="text-small muted">Resolution</dt>
              <dd class="text-body">{resolution}</dd>
            {/if}
          </dl>

          <Button variant="primary" icon="copy" onclick={() => cur && copyImage(cur)}>Copy image</Button>
          <div class="row">
            <Button size="sm" icon="folder" onclick={() => cur && openFolder(cur.instanceId)}>Open folder</Button>
          </div>
          {#if curInstance}
            <div>
              <Button size="sm" icon="cube" busy={settingIcon} disabled={settingIcon} onclick={() => cur && setIcon(cur)}>Set as instance icon</Button>
              <p class="text-small muted hint">Shown as a square next to {curInstance.name}.</p>
            </div>
          {/if}

          <div class="danger-zone">
            <Button size="sm" variant="danger" icon="trash" onclick={() => (confirmDelete = cur)}>Delete</Button>
            <span class="text-small muted">Deletes the file for good.</span>
          </div>
        {:else}
          <h2 class="text-heading">Screenshot details</h2>
          <p class="text-small muted">{shots?.length ? 'Select a screenshot to see it larger.' : 'Your screenshots show up here.'}</p>
        {/if}
      </div>
    </aside>
  </div>
</div>

{#if viewing && cur}
  <Dialog wide title={viewTitle} onClose={() => (viewing = false)} closeLabel="Close">
    <img class="full" src={cur.url} alt={`Screenshot from ${nameOf(cur.instanceId)}, ${takenLabel(cur.time)}`} />
  </Dialog>
{/if}

{#if confirmDelete}
  <Dialog title="Delete this screenshot?" icon="trash" tone="danger" onClose={() => (confirmDelete = null)} actions={deleteActions}>
    <p><span class="text-mono">{confirmDelete.fileName}</span> is deleted from {nameOf(confirmDelete.instanceId)}.</p>
    <p>It doesn’t go to the Recycle Bin, so this can’t be undone.</p>
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .layout {
    flex: 1;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  h1,
  h2,
  p,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .titles {
    min-width: 0;
  }
  .titles p {
    margin-top: var(--space-1);
  }
  .card {
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .empty {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-4);
    color: var(--ink-muted);
  }
  .empty .text-body-strong {
    color: var(--ink);
  }

  .groups {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .group-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }
  .when {
    margin-inline-start: auto;
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
  .thumb {
    position: relative;
    display: block;
    width: 100%;
    padding: 0;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
    cursor: pointer;
  }
  .thumb img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
  }
  .thumb.is-selected::after {
    content: '';
    position: absolute;
    inset: -2px;
    border: var(--border-block) solid var(--ink);
    pointer-events: none;
  }
  .thumb:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .caption {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .aside {
    flex: none;
    width: var(--size-inspector);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .aside-inner {
    position: sticky;
    top: 0;
    max-height: calc(100vh - var(--size-titlebar));
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
  }
  .big {
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .big img {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: contain;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .pos {
    flex: 1;
    text-align: center;
  }
  .file {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .name {
    overflow-wrap: anywhere;
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
  .dd-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .hint {
    margin-top: 6px;
  }
  .danger-zone {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    margin-top: auto;
    padding-top: var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .full {
    display: block;
    width: 100%;
    height: auto;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
  }
</style>
