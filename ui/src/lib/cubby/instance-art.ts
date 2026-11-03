/** Original Cubby instance emblems. Smooth 64-unit vector artwork, no bitmap dependencies. */
import type { Motif, GroundDef } from './data';

export const MATERIALS = {
  leaf: ['#c5f38b', '#529e49'],
  forest: ['#75cba0', '#287465'],
  wood: ['#dfb184', '#906047'],
  cream: ['#fff5e1', '#d4bc98'],
  stone: ['#e5eae4', '#8ca4ab'],
  steel: ['#f1f5f3', '#8cabb9'],
  gold: ['#ffe7a5', '#d99a3e'],
  coral: ['#ffb2a0', '#d66371'],
  violet: ['#d7bcff', '#8b64cc'],
  water: ['#a2edf0', '#3ba8bc'],
  ink: ['#586976', '#293943'],
} as const;

type Material = keyof typeof MATERIALS;
interface Shape {
  d: string;
  fill?: Material | `#${string}` | 'none';
  stroke?: Material | `#${string}`;
  width?: number;
  opacity?: number;
  detail?: boolean;
}
const p = (d: string, fill: Shape['fill'], extra: Omit<Shape, 'd' | 'fill'> = {}): Shape => ({ d, fill, ...extra });
const line = (d: string, stroke: Shape['stroke'], width = 2, detail = false): Shape => ({ d, fill: 'none', stroke, width, detail });

// Order and identifiers remain compatible with saved motif:ground instance values.
export const INSTANCE_ART: Record<Motif, readonly Shape[]> = {
  block: [
    p('M13 28 32 18 51 28v15L32 53 13 43Z', 'wood'),
    p('M32 36 51 27v16L32 53Z', '#765749'),
    p('M12 26q0-2 3-3l15-8q2-1 4 0l15 8q3 1 3 3v4L32 40 12 30Z', 'leaf'),
    p('M13 26 32 36 51 26 32 17Z', '#ade672'),
    line('m17 26 15-7 9 4', '#e3ffbd', 1.8, true),
    p('M22 23q-4-5-3-9 6 0 9 5-1-7 5-10 5 9-2 15Z', 'forest'),
  ],
  pickaxe: [
    p('m18 48 22-30 6 5-21 30q-3 3-6 0t-1-5Z', 'wood'),
    line('m21 49 21-28', '#f2cf96', 2, true),
    p('M12 23q14-15 30-8l10 7 1 11-10-8q-10-6-24 3Z', 'steel'),
    p('m12 23 7 5 6-3-7-5Z', '#7c96a0'),
    p('m35 15 8 3-5 10-8-4Z', 'gold'),
    line('M16 22q10-9 20-7', '#ffffff', 1.7, true),
  ],
  sword: [
    p('m26 40 19-28 9-2-1 10-23 25Z', 'steel'),
    p('m30 40 21-27 2 7-23 25Z', '#91afb8'),
    line('m30 34 17-20', '#ffffff', 1.8, true),
    p('m16 34 4-4 19 14-3 5q-1 2-3 0L17 38q-2-2-1-4Z', 'gold'),
    p('m13 49 11-13 7 6-11 13Z', 'wood'),
    line('m19 44 5 4', '#f3cea0', 2, true),
    p('m10 48 5-2 8 6-1 5-5 1-7-6Z', 'gold'),
  ],
  tree: [
    p('M24 50q5-4 4-14l-8-8 4-3 8 7 7-9 4 4-8 11q-1 7 5 12Z', 'wood'),
    p('M16 36C3 29 9 15 20 16 24 4 41 5 45 18 58 19 59 35 47 39 37 42 24 41 16 36Z', 'forest'),
    p('M15 30C9 24 14 16 23 19 25 9 39 10 42 21 50 20 54 28 48 33 36 36 24 35 15 30Z', 'leaf'),
    line('M19 21q6-1 7-5m5 19v9', '#e2efbe', 2, true),
    p('M21 51q11-5 22 0-11 6-22 0Z', '#7bbb76'),
  ],
  island: [
    p('M11 33 25 30 52 32 45 45 35 54q-3 3-6 0L19 45Z', 'stone'),
    p('m32 38 20-6-7 13-10 9q-2 2-3 1Z', '#6b8890'),
    p('M10 30q2-5 10-6l16-2q11 0 18 7 3 5-5 8l-18 5-16-4q-8-3-5-8Z', 'leaf'),
    p('m25 27 10-13q2-3 4 0l9 15-11 4Z', 'stone'),
    p('m31 20 4-6q2-3 4 0l5 9-6-3-3 3Z', 'cream'),
    p('M22 34q8-7 16-4l-6 5-6 3 2 9-4-1-3-10Z', 'water'),
    line('m15 31 7-2', '#e9ffbb', 1.8, true),
  ],
  house: [
    p('M17 30h29v21q0 3-3 3H20q-3 0-3-3Z', 'cream'),
    p('M34 30h12v21q0 3-3 3h-9Z', '#bc9873'),
    p('M40 13h7v15h-7Z', 'wood'),
    p('m10 30 19-18q3-3 6 0l19 18-5 6-17-16-17 16Z', 'coral'),
    p('m32 20 17 16 5-6-19-18q-2-2-3-1Z', '#b95363'),
    p('M26 41a6 6 0 0 1 12 0v13H26Z', 'wood'),
    p('M23 28h10v9H23Z', 'gold'),
    line('M28 29v7m-4-3h8', '#fff2c3', 1.4, true),
  ],
  castle: [
    p('M13 20h10v30H13Zm28 0h10v30H41Z', 'stone'),
    p('M9 18v-6h5v4h5v-4h5v4h4v-4h5v6l-5 8H14Z', 'steel'),
    p('M35 18v-6h5v4h5v-4h5v4h5v-4h3v6l-6 8H40Z', 'steel'),
    p('M19 31h27v23H19Z', 'cream'),
    p('M18 28v-5h6v4h6v-4h6v4h6v-4h6v8H18Z', 'stone'),
    p('M27 54V42a6 6 0 0 1 12 0v12Z', 'wood'),
    p('M17 28h4v7h-4Zm27 0h4v7h-4Z', '#526773'),
    line('M33 7v14', '#e5dac0', 2, true),
    p('M34 7h10l-3 4 3 4H34Z', 'coral'),
  ],
  gear: [
    p('m27 11 10 0 2 7 5 3 7-2 5 9-5 5v6l5 5-5 9-7-2-5 3-2 7H27l-2-7-5-3-7 2-5-9 5-5v-6l-5-5 5-9 7 2 5-3Z', 'gold'),
    p('M32 22a14 14 0 1 1 0 28 14 14 0 0 1 0-28Z', 'steel'),
    p('M32 28a8 8 0 1 1 0 16 8 8 0 0 1 0-16Z', 'ink'),
    line('M22 30a12 12 0 0 1 19-3', '#fff9d9', 2, true),
  ],
  rocket: [
    p('M23 37q-10 0-12 15l13-5Zm6 6 1 12q13-2 13-15Z', 'coral'),
    p('M17 44q-6 2-6 12 11-1 14-8Z', 'gold'),
    p('M20 44q-5 3-4 7 6 0 7-5Z', 'cream'),
    p('M21 38C25 20 39 9 53 10 54 24 43 39 30 45Z', 'cream'),
    p('M32 42q17-13 21-32 1 20-20 36Z', '#c5b69f'),
    p('M41 13q6-3 12-3 1 7-2 13Z', 'coral'),
    p('M38 22a7 7 0 1 1 0 14 7 7 0 0 1 0-14Z', 'gold'),
    p('M38 24a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z', 'water'),
    line('m36 27 2-1', '#ffffff', 2, true),
  ],
  potion: [
    p('M25 12h14v13l12 15c6 9 1 16-9 16H22c-10 0-15-7-9-16l12-15Z', 'stone'),
    p('M24 34q8 5 16 0l9 11q4 8-8 8H23q-12 0-8-8Z', 'violet'),
    p('M22 21h20v6H22Z', 'cream'),
    p('M26 9h12v13H26Z', 'wood'),
    line('M22 33 18 41q-2 4-1 6', '#ffffff', 2.5),
    p('M35 41a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z', '#e8cfff', { detail: true }),
    p('m45 11 2 4 4 2-4 2-2 4-2-4-4-2 4-2Z', 'gold', { detail: true }),
  ],
  gem: [
    p('m12 24 10-12h21l10 12-21 31Z', 'violet'),
    p('m12 24 20 31-8-31Z', '#b195ef'),
    p('m24 24 8 31 10-31Z', '#d4b7ff'),
    p('m42 24-10 31 21-31Z', '#7552b7'),
    p('m22 12 2 12 8-12 10 12 1-12Z', '#f0dfff'),
    line('m14 24 18 0 18 0', '#f4e9ff', 1.5, true),
    p('m48 38 2 4 4 2-4 2-2 4-2-4-4-2 4-2Z', 'cream', { detail: true }),
  ],
  heart: [
    p('M32 54 13 36C-1 20 20 5 32 20 44 5 65 20 51 36Z', 'coral'),
    p('M32 54V23c8-10 19-7 20 1 4 9-7 19-20 30Z', '#d86b83', { opacity: 0.7 }),
    line('M15 26q0-6 7-6', '#ffe3d8', 3),
    p('m40 35 2 4 4 2-4 2-2 4-2-4-4-2 4-2Z', '#ffd4c7', { detail: true }),
  ],
  skull: [
    p('M12 31C10 5 54 5 52 31q-1 11-10 14v9H22v-9q-9-3-10-14Z', 'cream'),
    p('M18 30q7-5 12 1l-2 8q-12 3-10-9Zm28 0q-7-5-12 1l2 8q12 3 10-9Z', 'ink'),
    p('m32 38 4 6h-8Z', '#6b5c51'),
    line('M28 48v6m8-6v6', '#8d7a62', 2),
    line('M20 23q2-5 8-5', '#ffffff', 2, true),
  ],
  flame: [
    p('M34 8c2 13 14 15 17 27 4 15-8 23-20 22C14 57 8 46 14 34q4-8 12-12 0 7 4 10 7-10 4-24Z', 'coral'),
    p('M32 24c-2 10 10 12 10 22q-1 12-12 10-13-2-10-14 2-5 7-9 0 6 3 8 6-8 2-17Z', 'gold'),
    p('M31 43c3 3 5 6 4 10-4 4-10 0-8-4Z', 'cream'),
  ],
  snowflake: [
    p('M32 12 49 22v20L32 52 15 42V22Z', 'water', { opacity: 0.2 }),
    line('M32 10v44M13 21l38 22M13 43l38-22', 'water', 5),
    line('m24 15 8 7 8-7m-16 34 8-7 8 7M13 30l10-3-2-10m22 30-2-10 10-3M13 34l10 3-2 10m22-30-2 10 10 3', '#d0f5f3', 2.8),
    p('m32 26 6 6-6 6-6-6Z', 'cream'),
  ],
  fish: [
    p('m41 31 12-11q3-2 3 1v24q0 3-3 1L41 37Z', 'coral'),
    p('m24 22 5-10 10 11-5 3Zm2 20 5 10 9-11-7-3Z', 'coral'),
    p('M9 33q15-23 36-3l1 6C26 57 12 43 9 33Z', 'water'),
    p('M9 33q7-12 15-11l-1 23q-10-2-14-12Z', '#c7efdf'),
    p('M19 28a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5Z', 'ink'),
    line('m31 28-4 6 5 5', '#dcffff', 2),
  ],
  mushroom: [
    p('M27 32h11l4 19q0 5-10 5t-10-5Z', 'cream'),
    p('M9 32C11 3 51 3 55 32q-21 13-46 0Z', 'coral'),
    p('M9 32q23-7 46 0-20 15-46 0Z', '#bf5a6d'),
    p('M18 24a5 5 0 1 1 0 10 5 5 0 0 1 0-10Zm13-10a5 5 0 1 1 0 10 5 5 0 0 1 0-10Zm13 9a4 4 0 1 1 0 8 4 4 0 0 1 0-8Z', 'cream'),
    line('M28 39 27 48', '#fff6de', 2, true),
  ],
  book: [
    p('M12 17q10-7 20 0 10-7 20 0v32q-10-5-20 2-10-7-20-2Z', 'wood'),
    p('M15 14q9-4 17 2v31q-8-5-17-3Zm34 0q-9-4-17 2v31q8-5 17-3Z', 'cream'),
    p('M32 16q8-6 17-2v30q-9-2-17 3Z', '#d6c4a5'),
    line('M19 24q4-1 8 1m-8 7q4-1 8 1', '#b69c74', 1.7, true),
    p('M38 13h6v23l-3-3-3 3Z', 'coral'),
    line('M32 18v27', '#9e8766', 1.5),
  ],
  map: [
    p('m10 18 14-5 16 6 14-5v33l-14 5-16-6-14 5Z', 'cream'),
    p('m24 13 16 6v33l-16-6Z', '#d1c09c'),
    p('m11 35 8-7 5 2 7-7 9 6 13-4v12l-13 4-10-7-6 5-13 3Z', 'leaf'),
    line('m17 40 6-7 10 5 10-7', '#fff8de', 2.5),
    p('M44 13a8 8 0 0 1 8 8q0 6-8 14-8-8-8-14a8 8 0 0 1 8-8Z', 'coral'),
    p('M44 17a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7Z', 'cream'),
  ],
  star: [
    p('M30 10q2-4 4 0l6 12 14 2q4 1 1 4L45 39l2 14q1 4-3 2l-12-7-12 7q-4 2-3-2l2-14L9 28q-3-3 1-4l14-2Z', 'gold'),
    p('m32 14 0 20-13 5 1 12 12-7 12 7-2-14 10-10-15-2Z', '#edb950'),
    line('m27 24 5-11', '#fff5ca', 2.5),
    p('m32 27 3 5 6 1-4 4 1 5-6-3-5 3 1-5-4-4 6-1Z', '#ffe8a7', { detail: true }),
  ],
};

/** SVG body from trusted, authored paths only. Per-instance ids isolate every gradient. */
export function renderInstanceArt(motif: Motif, ground: GroundDef, id: string, compact = false): string {
  const used = new Set<Material>();
  const shapes = INSTANCE_ART[motif].filter(shape => !compact || !shape.detail);
  for (const shape of shapes) {
    for (const color of [shape.fill, shape.stroke]) {
      if (color && color in MATERIALS) used.add(color as Material);
    }
  }
  const paint = (color: NonNullable<Shape['fill']>) => color in MATERIALS ? `url(#${id}-${color})` : color;
  const gradients = [...used].map(material => {
    const [light, dark] = MATERIALS[material];
    return `<linearGradient id="${id}-${material}" x1="0" y1="0" x2=".65" y2="1"><stop stop-color="${light}"/><stop offset="1" stop-color="${dark}"/></linearGradient>`;
  }).join('');
  const paths = shapes.map(shape => `<path d="${shape.d}" fill="${paint(shape.fill ?? 'none')}"${shape.stroke ? ` stroke="${paint(shape.stroke)}" stroke-width="${shape.width ?? 2}"` : ''}${shape.opacity != null ? ` opacity="${shape.opacity}"` : ''}/>`).join('');
  return `<defs>${gradients}
    <linearGradient id="${id}-ground" x2=".7" y2="1"><stop stop-color="${ground.base}"/><stop offset="1" stop-color="${ground.dark}"/></linearGradient>
    <linearGradient id="${id}-edge" x2=".5" y2="1"><stop stop-color="#fff8e8" stop-opacity=".28"/><stop offset=".6" stop-color="#fff8e8" stop-opacity=".03"/></linearGradient>
  </defs>
  <rect x=".75" y=".75" width="62.5" height="62.5" rx="15" fill="url(#${id}-ground)"/>
  <rect x="1.25" y="1.25" width="61.5" height="61.5" rx="14.5" fill="none" stroke="url(#${id}-edge)"/>
  <ellipse cx="32" cy="54" rx="17" ry="3" fill="#0c1115" opacity=".16"/>
  <g stroke-linecap="round" stroke-linejoin="round" transform="translate(3.2 2.4) scale(.9)">${paths}</g>`;
}
