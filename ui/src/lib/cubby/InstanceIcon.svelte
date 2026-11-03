<script lang="ts">
  import { GROUNDS, MOTIFS, isGround, isMotif, pickIcon } from './data';
  import { renderInstanceArt } from './instance-art';
  import type { InstanceIconProps } from './types';

  let { motif, ground, seed, src, size: sizeProp, framed, raised, label, class: className }: InstanceIconProps = $props();

  const uid = $props.id();
  const size = $derived(sizeProp || 32);
  const cls = $derived(['cb-instance-icon', framed !== false && 'cb-instance-icon--framed', raised && 'cb-instance-icon--raised', className]);
  // Without a known motif, the seed (the instance name) always draws the same motif and ground.
  const auto = $derived(isMotif(motif) ? null : pickIcon(seed));
  const motifId = $derived(auto ? auto.motif : (motif as keyof typeof MOTIFS));
  const m = $derived(MOTIFS[motifId]);
  const g = $derived(isGround(ground) ? GROUNDS[ground] : GROUNDS[auto ? auto.ground : m.ground]);
  const artwork = $derived(renderInstanceArt(motifId, g, uid, size <= 32));
</script>

{#if src}
  <img class={cls} {src} width={size} height={size} alt={label || ''} />
{:else}
  <svg
    class={cls}
    width={size}
    height={size}
    viewBox="0 0 64 64"
    focusable="false"
    role={label ? 'img' : undefined}
    aria-label={label || undefined}
    aria-hidden={label ? undefined : true}
  >
    <!-- Authored local paths only; imported images are rendered separately above. -->
    {@html artwork}
  </svg>
{/if}
