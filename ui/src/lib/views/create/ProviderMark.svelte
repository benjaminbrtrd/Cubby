<!-- The mark of a modpack provider. Letters on the provider's colour for now; one place to put real logos. -->
<script lang="ts" module>
  import type { ProviderId } from '../../types'

  export const MARKS: Record<ProviderId, { letters: string; color: string; ink: string }> = {
    atlauncher: { letters: 'AT', color: 'var(--diamond)', ink: 'var(--edge)' },
    curseforge: { letters: 'CF', color: 'var(--gold)', ink: 'var(--edge)' },
    ftb: { letters: 'FT', color: 'var(--redstone)', ink: '#fff' },
    modrinth: { letters: 'MR', color: 'var(--grass)', ink: 'var(--edge)' },
    technic: { letters: 'TN', color: 'var(--outline)', ink: 'var(--edge)' },
  }
</script>

<script lang="ts">
  let { id, size = 28 }: { id: ProviderId; size?: number } = $props()
  const m = $derived(MARKS[id])
</script>

<span class="mark" style:background={m.color} style:color={m.ink} style:width="{size}px" style:height="{size}px" aria-hidden="true">{m.letters}</span>

<style>
  .mark {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0;
    border: var(--border-block) solid var(--edge);
  }
</style>
