<script lang="ts" module>
  const PICK_TEXT = {
    motifs: 'Emblem',
    grounds: 'Background',
    importImage: 'Import image',
    reset: 'Use an emblem',
    custom: 'Imported image',
    importHint: 'Choose an emblem and a colour, or import your own square image. PNG or JPG, ideally 256 px or larger.',
  };
</script>

<script lang="ts">
  import Button from './Button.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import { GROUNDS, GROUND_IDS, GROUND_LABELS, MOTIFS, MOTIF_IDS, MOTIF_LABELS, isGround, isMotif, rovingKeys } from './data';
  import type { IconPickerProps, IconPickerValue } from './types';

  let {
    motif: motifProp = $bindable(),
    ground: groundProp = $bindable(),
    src = $bindable(),
    name,
    onChange,
    onImport,
    labels,
    motifLabels,
    groundLabels,
    class: className,
  }: IconPickerProps = $props();

  const uid = $props.id();
  const motifsId = uid + '-motifs';
  const groundsId = uid + '-grounds';

  const motif = $derived(isMotif(motifProp) ? motifProp : MOTIF_IDS[0]);
  const ground = $derived(isGround(groundProp) ? groundProp : MOTIFS[motif].ground);
  const L = $derived({ ...PICK_TEXT, ...labels });
  const ml = $derived({ ...MOTIF_LABELS, ...motifLabels });
  const gl = $derived({ ...GROUND_LABELS, ...groundLabels });
  const mi = $derived(MOTIF_IDS.indexOf(motif));
  const gi = $derived(GROUND_IDS.indexOf(ground));

  function change(next: Partial<IconPickerValue>) {
    const v: IconPickerValue = { motif, ground, src: null, ...next };
    motifProp = v.motif;
    groundProp = v.ground;
    src = v.src;
    onChange?.(v);
  }
</script>

<div class={['cb-ipick', className]}>
  <div class="cb-ipick__preview">
    <InstanceIcon {motif} {ground} {src} size={96} raised />
    {#if name}<span class="cb-ipick__name text-body-strong">{name}</span>{/if}
    <span class="cb-ipick__desc text-small">{src ? L.custom : ml[motif] + ' · ' + gl[ground]}</span>
  </div>
  <div class="cb-ipick__controls">
    <div id={motifsId} class="cb-label text-body-strong">{L.motifs}</div>
    <!-- Two radio groups, each one tab stop (on the chosen option), then the arrow keys. -->
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div
      class="cb-ipick__grid"
      role="radiogroup"
      aria-labelledby={motifsId}
      onkeydown={(e) => rovingKeys(e, MOTIF_IDS.length, mi, (i) => change({ motif: MOTIF_IDS[i] }))}
    >
      {#each MOTIF_IDS as id (id)}
        {@const on = !src && id === motif}
        <button
          type="button"
          role="radio"
          aria-checked={on}
          aria-label={ml[id]}
          data-tip={ml[id]}
          tabindex={id === motif ? 0 : -1}
          class={['cb-ipick__opt', on && 'is-on']}
          onclick={() => change({ motif: id })}
        >
          <InstanceIcon motif={id} {ground} size={48} framed={false} />
        </button>
      {/each}
    </div>
    <div id={groundsId} class="cb-label text-body-strong">{L.grounds}</div>
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div
      class="cb-ipick__swatches"
      role="radiogroup"
      aria-labelledby={groundsId}
      onkeydown={(e) => rovingKeys(e, GROUND_IDS.length, gi, (i) => change({ ground: GROUND_IDS[i] }))}
    >
      {#each GROUND_IDS as id (id)}
        {@const on = id === ground}
        <button
          type="button"
          role="radio"
          aria-checked={on}
          aria-label={gl[id]}
          data-tip={gl[id]}
          tabindex={on ? 0 : -1}
          class={['cb-ipick__swatch', on && 'is-on']}
          style:background={`linear-gradient(145deg, ${GROUNDS[id].base}, ${GROUNDS[id].dark})`}
          onclick={() => change({ ground: id })}
        ></button>
      {/each}
    </div>
    <div class="cb-ipick__actions">
      <Button size="sm" icon="import" onclick={onImport}>{L.importImage}</Button>
      {#if src}<Button size="sm" variant="ghost" onclick={() => change({ src: null })}>{L.reset}</Button>{/if}
    </div>
    <p class="cb-hint text-small">{L.importHint}</p>
  </div>
</div>
