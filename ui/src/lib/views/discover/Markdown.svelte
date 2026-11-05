<!-- Modrinth Markdown (descriptions, changelogs), sanitized. Links open in the browser through the
     backend (or in Cubby for Modrinth project links); the webview itself never navigates. -->
<script lang="ts" module>
  import DOMPurify from 'dompurify'
  import { Marked } from 'marked'
  import { t } from '../../i18n.svelte'
  import type { ProjectKind } from '../../types'

  const marked = new Marked({ gfm: true, breaks: false, async: false })
  // A private instance: its settings don't leak to other users of DOMPurify.
  const purify = DOMPurify(window)

  const SAFE_URL = /^(https?:|mailto:)/i
  const SAFE_IMG = /^(https?:|data:image\/(png|jpe?g|gif|webp|avif);)/i

  /** Modrinth project links open the project page in Cubby. */
  const PROJECT_URL = /^https?:\/\/(?:www\.)?modrinth\.com\/(mod|modpack|shader|resourcepack|plugin|datapack)\/([^/?#]+)/i

  export function projectLink(href: string): { slug: string; kind: ProjectKind } | null {
    const m = PROJECT_URL.exec(href)
    if (!m) return null
    const kind = m[1].toLowerCase()
    return { slug: decodeURIComponent(m[2]), kind: kind === 'plugin' || kind === 'datapack' ? 'mod' : (kind as ProjectKind) }
  }

  /** A link that replaces an embedded video (iframes are never rendered). */
  function videoLink(src: string): string | null {
    const yt = /^https?:\/\/(?:www\.)?(?:youtube(?:-nocookie)?\.com\/embed\/|youtu\.be\/)([\w-]{6,})/i.exec(src)
    if (yt) return `https://www.youtube.com/watch?v=${yt[1]}`
    return SAFE_URL.test(src) && !src.startsWith('mailto:') ? src : null
  }

  function rename(el: Element, tag: string): Element {
    const next = el.ownerDocument.createElement(tag)
    for (const a of Array.from(el.attributes)) next.setAttribute(a.name, a.value)
    while (el.firstChild) next.appendChild(el.firstChild)
    el.replaceWith(next)
    return next
  }

  /** `level`: the heading level a Markdown `# Title` becomes (the page has its own h1 and section h2s). */
  export function renderMarkdown(source: string, level = 2): string {
    const raw = marked.parse(source ?? '', { async: false }) as string
    const frag = purify.sanitize(raw, {
      RETURN_DOM_FRAGMENT: true,
      ADD_TAGS: ['iframe'],
      FORBID_TAGS: ['style', 'script', 'form', 'input', 'button', 'textarea', 'select', 'option', 'object', 'embed', 'video', 'audio', 'source', 'link', 'meta', 'base'],
      FORBID_ATTR: ['style', 'class', 'id', 'target', 'srcset', 'onerror', 'onload'],
    })
    const doc = frag.ownerDocument

    // embedded videos become plain links
    for (const frame of Array.from(frag.querySelectorAll('iframe'))) {
      const href = videoLink(frame.getAttribute('src') ?? '')
      if (!href) {
        frame.remove()
        continue
      }
      const a = doc.createElement('a')
      a.setAttribute('href', href)
      a.textContent = /youtube/.test(href) ? t('Watch the video on YouTube') : t('Open the embedded page')
      frame.replaceWith(a)
    }

    // headings fit under the page's own titles
    for (const h of Array.from(frag.querySelectorAll('h1, h2, h3, h4, h5, h6'))) {
      const n = Number(h.tagName[1])
      const next = rename(h, `h${Math.min(6, n + level - 1)}`)
      next.setAttribute('class', n <= 2 ? 'text-heading' : 'text-body-strong')
    }

    for (const a of Array.from(frag.querySelectorAll('a'))) {
      const href = a.getAttribute('href') ?? ''
      if (!SAFE_URL.test(href) && !href.startsWith('#')) a.removeAttribute('href')
      a.setAttribute('rel', 'noopener noreferrer')
      if (href && SAFE_URL.test(href)) a.setAttribute('title', href)
    }

    for (const img of Array.from(frag.querySelectorAll('img'))) {
      const src = img.getAttribute('src') ?? ''
      if (!SAFE_IMG.test(src)) {
        img.remove()
        continue
      }
      img.setAttribute('loading', 'lazy')
      img.setAttribute('decoding', 'async')
      img.setAttribute('referrerpolicy', 'no-referrer')
      img.setAttribute('draggable', 'false')
      if (!img.hasAttribute('alt')) img.setAttribute('alt', '')
    }

    for (const el of Array.from(frag.querySelectorAll('code, pre, kbd, samp'))) el.setAttribute('class', 'text-mono')
    for (const el of Array.from(frag.querySelectorAll('th'))) el.setAttribute('class', 'text-label')
    for (const el of Array.from(frag.querySelectorAll('small, sub, sup'))) el.setAttribute('class', 'text-small')

    const box = doc.createElement('div')
    box.appendChild(frag)
    return box.innerHTML
  }
</script>

<script lang="ts">
  import { api } from '../../api'
  import { app } from '../../state.svelte'

  interface Props {
    source: string
    /** Heading level of a Markdown `# Title` (2 under a page title, 4 inside a changelog entry). */
    level?: number
  }

  let { source, level = 2 }: Props = $props()

  const html = $derived(renderMarkdown(source, level))

  function follow(e: MouseEvent) {
    if (e.type === 'auxclick' && e.button !== 1) return
    const a = (e.target as Element | null)?.closest?.('a')
    if (!a) return
    e.preventDefault()
    const href = a.getAttribute('href')
    if (!href || href.startsWith('#')) return
    const project = projectLink(href)
    if (project && e.type === 'click' && !e.ctrlKey && !e.shiftKey) app.openProject(project.slug, project.kind)
    else void api.openUrl(href).catch((err) => app.error(err))
  }
</script>

<!-- Clicks on the links inside are handled here (event delegation); the links stay real links for the keyboard. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md text-body selectable" onclick={follow} onauxclick={follow}>
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- sanitized by DOMPurify above -->
  {@html html}
</div>

<style>
  .md {
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--ink);
  }
  .md > :global(:first-child) {
    margin-top: 0;
  }
  .md > :global(:last-child) {
    margin-bottom: 0;
  }
  .md :global(p),
  .md :global(ul),
  .md :global(ol),
  .md :global(blockquote),
  .md :global(pre),
  .md :global(table),
  .md :global(details) {
    margin: 0 0 var(--space-3);
  }
  .md :global(h2),
  .md :global(h3),
  .md :global(h4),
  .md :global(h5),
  .md :global(h6) {
    margin: var(--space-6) 0 var(--space-2);
  }
  .md :global(ul),
  .md :global(ol) {
    padding-inline-start: var(--space-6);
  }
  .md :global(li + li) {
    margin-top: var(--space-1);
  }
  .md :global(li > ul),
  .md :global(li > ol) {
    margin: var(--space-1) 0 0;
  }
  .md :global(a) {
    color: var(--grass-text);
    text-underline-offset: 3px;
  }
  .md :global(a:hover) {
    color: var(--ink);
  }
  .md :global(a:focus-visible) {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .md :global(strong),
  .md :global(b) {
    font-weight: 600;
  }
  /* no italics in Cubby: emphasis is a heavier weight */
  .md :global(em),
  .md :global(i) {
    font-style: normal;
    font-weight: 500;
  }
  .md :global(code) {
    padding: 0 var(--space-1);
    background: var(--field);
    border: var(--border-hair) solid var(--line);
  }
  .md :global(pre) {
    padding: var(--space-3);
    overflow-x: auto;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .md :global(pre code) {
    padding: 0;
    background: none;
    border: 0;
  }
  .md :global(kbd) {
    padding: 0 var(--space-1);
    background: var(--surface);
    border: var(--border-hair) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .md :global(blockquote) {
    padding: var(--space-2) var(--space-4);
    color: var(--ink-muted);
    background: var(--surface);
    border-inline-start: 4px solid var(--outline);
  }
  .md :global(blockquote > :last-child) {
    margin-bottom: 0;
  }
  .md :global(hr) {
    height: 0;
    margin: var(--space-5) 0;
    border: 0;
    border-top: var(--border-hair) solid var(--line);
  }
  .md :global(img) {
    max-width: 100%;
    height: auto;
    vertical-align: middle;
  }
  .md :global(table) {
    display: block;
    max-width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
    border: var(--border-block) solid var(--edge);
  }
  .md :global(th),
  .md :global(td) {
    padding: var(--space-2) var(--space-3);
    text-align: start;
    vertical-align: top;
    border-bottom: var(--border-hair) solid var(--line);
  }
  .md :global(th) {
    color: var(--ink-muted);
    background: var(--bg-chrome);
  }
  .md :global(details) {
    padding: var(--space-2) var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .md :global(summary) {
    cursor: pointer;
    font-weight: 500;
  }
  .md :global(summary:focus-visible) {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .md :global([align='center']) {
    text-align: center;
  }
</style>
