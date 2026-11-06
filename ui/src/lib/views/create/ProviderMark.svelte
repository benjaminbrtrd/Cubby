<!-- The mark of a modpack provider: its official logo (ui/public/providers, from the project's own site or repository,
     or Simple Icons for CurseForge), else letters on the provider's colour. -->
<script lang="ts" module>
  import type { ProviderId } from '../../types'

  /** Official logos in public/providers. `crop`: the file is the logo with its name beside it, only the left square is shown. */
  const LOGOS: Partial<Record<ProviderId, { file: string; crop?: boolean }>> = {
    atlauncher: { file: 'atlauncher.svg' },
    curseforge: { file: 'curseforge.svg' },
    ftb: { file: 'ftb.svg' },
    modrinth: { file: 'modrinth.svg' },
    technic: { file: 'technic.png', crop: true },
  }

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
  const logo = $derived(LOGOS[id])
</script>

{#if logo}
  <img class={['logo', logo.crop && 'crop']} src="/providers/{logo.file}" alt="" width={size} height={size} />
{:else}
  <span class="mark" style:background={m.color} style:color={m.ink} style:width="{size}px" style:height="{size}px" aria-hidden="true">{m.letters}</span>
{/if}

<style>
  .logo {
    flex: none;
    display: block;
    object-fit: contain;
  }
  .logo.crop {
    object-fit: cover;
    object-position: left center;
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
