// Places a menu next to its anchor (the menu's parent) in fixed position: it opens below, flips above when there is
// more room there, stays inside the window, and scrolls inside itself when it is still too tall. A menu placed this
// way never makes its page grow or scroll. Use it as an attachment, before any attachment that focuses the menu:
//   <div class="menu" {@attach floating}>
export function floating(node: HTMLElement) {
  const anchor = node.parentElement
  if (!anchor) return
  // a menu that sets only its end edge (`inset-inline-end`) lines up with the right edge of its anchor
  const alignEnd = getComputedStyle(node).insetInlineStart === 'auto'

  const place = () => {
    const r = anchor.getBoundingClientRect()
    node.style.position = 'fixed'
    node.style.insetInline = 'auto'
    node.style.right = 'auto'
    node.style.bottom = 'auto'
    node.style.maxHeight = 'none'
    node.style.overflowY = 'auto'
    const width = node.offsetWidth
    const height = node.offsetHeight
    const below = window.innerHeight - r.bottom - 8
    const above = r.top - 8
    const flip = height > below && above > below
    const room = flip ? above : below
    const maxHeight = Math.min(height, Math.max(120, room - 4))
    node.style.maxHeight = `${maxHeight}px`
    node.style.top = `${flip ? Math.max(8, r.top - 4 - maxHeight) : r.bottom + 4}px`
    const left = alignEnd ? r.right - width : r.left
    node.style.left = `${Math.max(8, Math.min(left, window.innerWidth - width - 8))}px`
  }

  place()
  window.addEventListener('resize', place)
  window.addEventListener('scroll', place, true)
  return () => {
    window.removeEventListener('resize', place)
    window.removeEventListener('scroll', place, true)
  }
}
