/** Wraps SVG markup in a data: URL. */
export function svgUrl(viewBox: string, body: string, width?: number, height = width): string {
  const size = width ? ` width="${width}" height="${height}"` : ''
  return 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg"${size} viewBox="${viewBox}" shape-rendering="crispEdges">${body}</svg>`)
}

/** A square pixel image: one character per pixel, looked up in `palette` (unknown characters stay transparent). */
export function pixelArt(rows: string[], palette: Record<string, string>, size?: number): string {
  let rects = ''
  rows.forEach((row, y) =>
    [...row].forEach((ch, x) => {
      if (palette[ch]) rects += `<rect x="${x}" y="${y}" width="1" height="1" fill="${palette[ch]}"/>`
    }),
  )
  return svgUrl(`0 0 ${rows.length} ${rows.length}`, rects, size)
}
