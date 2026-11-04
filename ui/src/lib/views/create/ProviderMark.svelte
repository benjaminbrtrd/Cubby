<!-- The mark of a modpack provider: its logo (ui/public/providers/<id>.svg, from the project's own site or repository)
     when Cubby has one, else letters on the provider's colour. -->
<script lang="ts" module>
  import type { ProviderId } from '../../types'

  /** Providers whose official logo is in public/providers. */
  const LOGOS: ProviderId[] = ['atlauncher', 'modrinth']

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
  const logo = $derived(LOGOS.includes(id))
</script>

{#if logo}
  <img class="logo" src="/providers/{id}.svg" alt="" width={size} height={size} />
{:else}
  <span class="mark" style:background={m.color} style:color={m.ink} style:width="{size}px" style:height="{size}px" aria-hidden="true">{m.letters}</span>
{/if}

<style>
  .logo {
    flex: none;
    display: block;
    object-fit: contain;
  }
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
