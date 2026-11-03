/* Cubby design system data: control icons, legacy logo data, instance metadata and helpers.
   The instance illustrations are authored in instance-art.ts. Keep motif and ground IDs stable:
   saved instances use these identifiers, and their order determines the seeded defaults. */

/** One horizontal run of pixels: [x, y, width]; every run is 1 pixel tall. */
export type Run = readonly [x: number, y: number, w: number];
/** A run with a colour: [x, y, width, index into the palette]. */
export type ColorRun = readonly [x: number, y: number, w: number, c: number];

export type IconName = "play" | "stop" | "plus" | "search" | "library" | "discover" | "servers" | "captures" | "accounts" | "settings" | "folder" | "download" | "import" | "update" | "check" | "info" | "warning" | "error" | "close" | "wrench" | "trash" | "copy" | "terminal" | "clock" | "cube" | "mods" | "memory" | "key" | "globe" | "offline" | "minimize" | "maximize" | "chevron-right" | "chevron-down" | "more";
export type Status = 'ready' | 'download' | 'installing' | 'running' | 'crashed' | 'update' | 'online' | 'offline' | 'disabled';
export type Motif = "block" | "pickaxe" | "sword" | "tree" | "island" | "house" | "castle" | "gear" | "rocket" | "potion" | "gem" | "heart" | "skull" | "flame" | "snowflake" | "fish" | "mushroom" | "book" | "map" | "star";
export type Ground = "moss" | "sky" | "lagoon" | "amber" | "brick" | "amethyst" | "rose" | "sand" | "slate" | "night";

export interface MotifDef {
  /** English name, shown in the icon picker. */
  label: string;
  /** The ground drawn behind the motif when none is chosen. */
  ground: Ground;
}
export interface GroundDef { label: string; base: string; dark: string }

/* Icons: 24 x 24 grid, 2 px strokes, square caps and mitred corners, one ink */
/** A shape of an icon: an SVG path, stroked unless `fill` is set. */
export interface IconShape { d: string; fill?: true }
export const ICONS: Record<IconName, readonly IconShape[]> = {
  play: [{ d: "M8 4.5 20 12 8 19.5z", fill: true }],
  stop: [{ d: "M6 6h12v12h-12z", fill: true }],
  plus: [{ d: "M12 5v14M5 12h14" }],
  search: [{ d: "M4 11a7 7 0 1 0 14 0a7 7 0 1 0 -14 0z" }, { d: "M16.5 16.5 20 20" }],
  library: [{ d: "M4 4h6v6h-6z" }, { d: "M14 4h6v6h-6z" }, { d: "M4 14h6v6h-6z" }, { d: "M14 14h6v6h-6z" }],
  discover: [{ d: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z" }, { d: "M15.5 8.5 13.5 13.5 8.5 15.5 10.5 10.5z" }],
  servers: [{ d: "M3 4h18v7h-18z" }, { d: "M3 13h18v7h-18z" }, { d: "M7 7.5h2M7 16.5h2" }],
  captures: [{ d: "M3 4h18v16h-18z" }, { d: "M7 8h3v3h-3z" }, { d: "M21 15l-5-5-9 10" }],
  accounts: [{ d: "M8 3h8v8h-8z" }, { d: "M4 21v-6h16v6" }],
  settings: [{ d: "M4 7h9M17 7h3M4 17h3M11 17h9" }, { d: "M13 5h4v4h-4z" }, { d: "M7 15h4v4h-4z" }],
  folder: [{ d: "M3 5h7l2 2h9v12H3z" }],
  download: [{ d: "M12 4v11M7 10l5 5 5-5M5 20h14" }],
  import: [{ d: "M12 3v10M7 8l5 5 5-5M4 14v6h16v-6" }],
  update: [{ d: "M12 20V8M7 13l5-5 5 5M5 4h14" }],
  check: [{ d: "M5 12l4 4 10-10" }],
  info: [{ d: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z" }, { d: "M12 11v6M12 7v1" }],
  warning: [{ d: "M12 3 2 20h20z" }, { d: "M12 9v5M12 16.5v1" }],
  error: [{ d: "M8 3h8l5 5v8l-5 5H8l-5-5V8z" }, { d: "M12 7.5v5.5M12 16v1" }],
  close: [{ d: "M6 6l12 12M18 6 6 18" }],
  wrench: [{ d: "M14.5 3.5a5 5 0 0 0-4.6 6.9L3.5 16.8l3.7 3.7 6.4-6.4a5 5 0 0 0 6.9-4.6l-3 1.5-3.2-1.3-1.3-3.2z" }],
  trash: [{ d: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v5M14 11v5" }],
  copy: [{ d: "M9 9h11v11h-11z" }, { d: "M15 9V4H4v11h5" }],
  terminal: [{ d: "M3 4h18v16h-18z" }, { d: "M7 9l3 3-3 3M13 15h4" }],
  clock: [{ d: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z" }, { d: "M12 7v5l3 2" }],
  cube: [{ d: "M12 3l8 4.5v9L12 21l-8-4.5v-9z" }, { d: "M4 7.5l8 4.5 8-4.5M12 12v9" }],
  mods: [{ d: "M4 7h4.5a2.5 2.5 0 0 1 5 0H18v4.5a2.5 2.5 0 0 1 0 5V21H4z" }],
  memory: [{ d: "M6 6h12v12h-12z" }, { d: "M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4" }],
  key: [{ d: "M3 9h8v8h-8z" }, { d: "M11 13h10M17 13v4M20 13v3" }],
  globe: [{ d: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z" }, { d: "M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18" }],
  offline: [{ d: "M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z" }, { d: "M5.6 5.6l12.8 12.8" }],
  minimize: [{ d: "M5 12h14" }],
  maximize: [{ d: "M5 5h14v14h-14z" }],
  "chevron-right": [{ d: "M9 5l7 7-7 7" }],
  "chevron-down": [{ d: "M5 9l7 7 7-7" }],
  more: [{ d: "M4 10.5h3v3h-3z", fill: true }, { d: "M10.5 10.5h3v3h-3z", fill: true }, { d: "M17 10.5h3v3h-3z", fill: true }],
};
export const ICON_NAMES = Object.keys(ICONS) as IconName[];
/** Directional icons that flip in right-to-left layouts. */
export const MIRRORED: Partial<Record<IconName, true>> = { 'chevron-right': true };

/* Logo: 16 x 16 mark and a 7 pixel tall wordmark */
export const MARK: { c: readonly string[]; r: readonly ColorRun[] } = {
  c: ["#15120f","#f6c96a","#b07a1e","#24201c","#b8ec86","#8bd450","#5e9a2e","#3e6b1d"],
  r: [[0,0,16,0],[0,1,1,0],[1,1,13,1],[14,1,1,2],[15,1,1,0],[0,2,1,0],[1,2,1,1],[2,2,12,3],[14,2,1,2],[15,2,1,0],[0,3,1,0],[1,3,1,1],[2,3,5,3],[7,3,2,4],[9,3,5,3],[14,3,1,2],[15,3,1,0],[0,4,1,0],[1,4,1,1],[2,4,3,3],[5,4,2,4],[7,4,4,5],[11,4,3,3],[14,4,1,2],[15,4,1,0],[0,5,1,0],[1,5,1,1],[2,5,1,3],[3,5,1,4],[4,5,9,5],[13,5,1,3],[14,5,1,2],[15,5,1,0],[0,6,1,0],[1,6,1,1],[2,6,1,3],[3,6,2,6],[5,6,6,5],[11,6,2,7],[13,6,1,0],[14,6,1,2],[15,6,1,0],[0,7,1,0],[1,7,1,1],[2,7,1,3],[3,7,4,6],[7,7,2,5],[9,7,4,7],[13,7,1,0],[14,7,1,2],[15,7,1,0],[0,8,1,0],[1,8,1,1],[2,8,1,3],[3,8,5,6],[8,8,5,7],[13,8,1,0],[14,8,1,2],[15,8,1,0],[0,9,1,0],[1,9,1,1],[2,9,1,3],[3,9,5,6],[8,9,5,7],[13,9,1,0],[14,9,1,2],[15,9,1,0],[0,10,1,0],[1,10,1,1],[2,10,1,3],[3,10,5,6],[8,10,5,7],[13,10,1,0],[14,10,1,2],[15,10,1,0],[0,11,1,0],[1,11,1,1],[2,11,2,3],[4,11,1,0],[5,11,3,6],[8,11,3,7],[11,11,3,0],[14,11,1,2],[15,11,1,0],[0,12,1,0],[1,12,1,1],[2,12,4,3],[6,12,1,0],[7,12,1,6],[8,12,1,7],[9,12,3,0],[12,12,2,3],[14,12,1,2],[15,12,1,0],[0,13,1,0],[1,13,1,1],[2,13,6,3],[8,13,2,0],[10,13,4,3],[14,13,1,2],[15,13,1,0],[0,14,1,0],[1,14,1,1],[2,14,13,2],[15,14,1,0],[0,15,16,0]],
};
export const WORDMARK: readonly Run[] = [[1,0,4],[0,1,1],[0,2,1],[0,3,1],[0,4,1],[0,5,1],[1,6,4],[6,0,1],[10,0,1],[6,1,1],[10,1,1],[6,2,1],[10,2,1],[6,3,1],[10,3,1],[6,4,1],[10,4,1],[6,5,1],[10,5,1],[7,6,3],[12,0,4],[12,1,1],[16,1,1],[12,2,1],[16,2,1],[12,3,4],[12,4,1],[16,4,1],[12,5,1],[16,5,1],[12,6,4],[18,0,4],[18,1,1],[22,1,1],[18,2,1],[22,2,1],[18,3,4],[18,4,1],[22,4,1],[18,5,1],[22,5,1],[18,6,4],[24,0,1],[28,0,1],[24,1,1],[28,1,1],[25,2,1],[27,2,1],[26,3,1],[26,4,1],[26,5,1],[26,6,1]];
export const WORD_W = 29;

/* Instance emblems: stable identifiers, vector artwork in instance-art.ts */
export const MOTIFS: Record<Motif, MotifDef> = {
  block: { label: 'Meadow', ground: 'moss' },
  pickaxe: { label: 'Miner', ground: 'slate' },
  sword: { label: 'Adventure', ground: 'brick' },
  tree: { label: 'Grove', ground: 'moss' },
  island: { label: 'Sky island', ground: 'sky' },
  house: { label: 'Refuge', ground: 'moss' },
  castle: { label: 'Citadel', ground: 'slate' },
  gear: { label: 'Workshop', ground: 'slate' },
  rocket: { label: 'Orbit', ground: 'night' },
  potion: { label: 'Alchemy', ground: 'amethyst' },
  gem: { label: 'Crystal', ground: 'amethyst' },
  heart: { label: 'Heart', ground: 'rose' },
  skull: { label: 'Hardcore', ground: 'night' },
  flame: { label: 'Ember', ground: 'brick' },
  snowflake: { label: 'Frost', ground: 'sky' },
  fish: { label: 'Ocean', ground: 'lagoon' },
  mushroom: { label: 'Mushroom', ground: 'moss' },
  book: { label: 'Enchantment', ground: 'amethyst' },
  map: { label: 'Explorer', ground: 'sand' },
  star: { label: 'Starlight', ground: 'night' },
};
export const GROUNDS: Record<Ground, GroundDef> = {
  moss: { label: 'Moss', base: '#426751', dark: '#1f352c' },
  sky: { label: 'Sky', base: '#46779c', dark: '#243c59' },
  lagoon: { label: 'Lagoon', base: '#397b7d', dark: '#1c4048' },
  amber: { label: 'Amber', base: '#95734c', dark: '#51402c' },
  brick: { label: 'Terracotta', base: '#8d5450', dark: '#482b32' },
  amethyst: { label: 'Amethyst', base: '#6b5a8e', dark: '#352e50' },
  rose: { label: 'Rose', base: '#95697c', dark: '#503342' },
  sand: { label: 'Sand', base: '#9d8c70', dark: '#5d503f' },
  slate: { label: 'Slate', base: '#5a6b77', dark: '#2b3742' },
  night: { label: 'Midnight', base: '#424763', dark: '#21253b' },
};
/* Helpers shared by the components */

/** Joins the truthy class names (the reference implementation's cx). */
export function cx(...names: unknown[]): string {
  return names.filter(Boolean).join(' ');
}

/** A rounded percentage, `42%`. Translated UIs pass their own Intl-formatted text instead. */
export function pct(v: number): string {
  return Math.round(v) + '%';
}

/** Icons are vectors: any size works. 18 px in controls and rows, 24 to 27 px in the hotbar. */
export function iconSize(size?: number): number {
  return Math.max(9, Math.round(size || 18));
}

/** Stroke width in grid units so the line stays about 1.5 px or thicker on screen, even on small icons. */
export function iconStroke(px: number): number {
  return Math.max(2, Math.round((24 * 1.5 / px) * 100) / 100);
}

/** The logo mark is 16 x 16 logo pixels and the wordmark 7 pixels tall, both drawn at k screen pixels per logo pixel. */
export function logoScale(size?: number): number {
  return Math.max(1, Math.round((size || 32) / 16));
}

export const MOTIF_IDS = Object.keys(MOTIFS) as Motif[];
export const GROUND_IDS = Object.keys(GROUNDS) as Ground[];
export const MOTIF_LABELS = Object.fromEntries(MOTIF_IDS.map((id) => [id, MOTIFS[id].label])) as Record<Motif, string>;
export const GROUND_LABELS = Object.fromEntries(GROUND_IDS.map((id) => [id, GROUNDS[id].label])) as Record<Ground, string>;

export function isIconName(v: unknown): v is IconName {
  return typeof v === 'string' && Object.hasOwn(ICONS, v);
}
export function isMotif(v: unknown): v is Motif {
  return typeof v === 'string' && Object.hasOwn(MOTIFS, v);
}
export function isGround(v: unknown): v is Ground {
  return typeof v === 'string' && Object.hasOwn(GROUNDS, v);
}

/** FNV-1a 32-bit hash. */
export function hash(str: string): number {
  let x = 2166136261;
  for (let i = 0; i < str.length; i++) {
    x ^= str.charCodeAt(i);
    x = Math.imul(x, 16777619);
  }
  return x >>> 0;
}

/** The emblem an instance gets before the player picks one: the same seed (its name) always draws the same pair. */
export function pickIcon(seed?: string | number | null): { motif: Motif; ground: Ground } {
  const n = hash(String(seed == null ? '' : seed));
  return {
    motif: MOTIF_IDS[n % MOTIF_IDS.length],
    ground: GROUND_IDS[Math.floor(n / MOTIF_IDS.length) % GROUND_IDS.length],
  };
}

/**
 * One tab stop, then the arrows: arrow keys, Home and End move the choice inside a tab list or a radio group,
 * and focus follows (`[role="tab"]` or `[role="radio"]` children of the element that has the listener).
 * Left and right swap in right-to-left layouts.
 */
export function rovingKeys(e: KeyboardEvent, count: number, index: number, onMove: (to: number) => void): void {
  const host = e.currentTarget as HTMLElement;
  const rtl = getComputedStyle(host).direction === 'rtl';
  const step: Record<string, number> = { ArrowRight: rtl ? -1 : 1, ArrowLeft: rtl ? 1 : -1, ArrowDown: 1, ArrowUp: -1 };
  const next = step[e.key];
  const to = e.key === 'Home' ? 0 : e.key === 'End' ? count - 1 : next != null ? (index + next + count) % count : null;
  if (to == null) return;
  e.preventDefault();
  onMove(to);
  const els = host.querySelectorAll<HTMLElement>('[role="tab"],[role="radio"]');
  els[to]?.focus();
}
