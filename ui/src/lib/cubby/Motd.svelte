<!-- A server's message of the day drawn the way the game's server list does: `§` colours and styles, the game's
     gray by default, a drop shadow of a quarter of the colour, and the spaces servers use to centre lines. -->
<script lang="ts" module>
  /** The 16 colours of `§0` to `§f`. */
  const COLORS = ['#000000', '#0000aa', '#00aa00', '#00aaaa', '#aa0000', '#aa00aa', '#ffaa00', '#aaaaaa', '#555555', '#5555ff', '#55ff55', '#55ffff', '#ff5555', '#ff55ff', '#ffff55', '#ffffff'];
  const DEFAULT = '#aaaaaa';

  type Run = { text: string; color: string; bold: boolean; italic: boolean; underline: boolean; strike: boolean; obfuscated: boolean };

  /** Lines of styled runs, from text with `§` codes (`§#rrggbb` for a hex colour). */
  export function parseMotd(source: string): Run[][] {
    const lines: Run[][] = [[]];
    let style: Omit<Run, 'text'> = { color: DEFAULT, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
    let text = '';
    const flush = () => {
      if (text) lines[lines.length - 1].push({ text, ...style });
      text = '';
    };
    for (let i = 0; i < source.length; i++) {
      const c = source[i];
      if (c === '\n') {
        flush();
        lines.push([]);
      } else if (c === '§' && i + 1 < source.length) {
        flush();
        const code = source[i + 1].toLowerCase();
        if (code === '#' && /^[0-9a-f]{6}$/i.test(source.slice(i + 2, i + 8))) {
          style = { color: `#${source.slice(i + 2, i + 8)}`, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
          i += 7;
          continue;
        }
        const n = parseInt(code, 16);
        if (!Number.isNaN(n) && code.length === 1 && /[0-9a-f]/.test(code)) {
          // A colour resets the styles, as in the game.
          style = { color: COLORS[n], bold: false, italic: false, underline: false, strike: false, obfuscated: false };
        } else if (code === 'r') {
          style = { color: DEFAULT, bold: false, italic: false, underline: false, strike: false, obfuscated: false };
        } else if (code === 'l') style = { ...style, bold: true };
        else if (code === 'o') style = { ...style, italic: true };
        else if (code === 'n') style = { ...style, underline: true };
        else if (code === 'm') style = { ...style, strike: true };
        else if (code === 'k') style = { ...style, obfuscated: true };
        i++;
      } else {
        text += c;
      }
    }
    flush();
    return lines;
  }

  /** The game's text shadow: the colour at a quarter of its brightness. */
  function shadow(hex: string): string {
    const v = parseInt(hex.slice(1), 16);
    const q = (s: number) => ((v >> s) & 255) >> 2;
    return `rgb(${q(16)}, ${q(8)}, ${q(0)})`;
  }
</script>

<script lang="ts">
  let { text, class: className }: { text: string; class?: string } = $props();
  const lines = $derived(parseMotd(text).filter((l, i, all) => l.length || i < all.length - 1))
</script>

<div class={['cb-motd', className]}>
  {#each lines as line, i (i)}
    <div class="cb-motd__line">{#each line as run, j (j)}<span
          class={['cb-motd__run', run.bold && 'is-bold', run.italic && 'is-italic', run.obfuscated && 'is-obfuscated']}
          style:color={run.color}
          style:text-shadow="1px 1px 0 {shadow(run.color)}"
          style:text-decoration={[run.underline && 'underline', run.strike && 'line-through'].filter(Boolean).join(' ') || undefined}
        >{run.text}</span>{/each}</div>
  {/each}
</div>
