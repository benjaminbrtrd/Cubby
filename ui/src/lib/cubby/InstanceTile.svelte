<script lang="ts">
  import Content from './Content.svelte';
  import IconButton from './IconButton.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import MetaChip from './MetaChip.svelte';
  import ProgressBar from './ProgressBar.svelte';
  import StatusTag from './StatusTag.svelte';
  import type { InstanceTileProps } from './types';

  let {
    name,
    motif,
    ground,
    iconSrc,
    art,
    version,
    loader,
    snapshot,
    status: statusProp,
    statusDetail,
    progress,
    selected,
    onSelect,
    onPlay,
    playLabel,
    meta,
    class: className,
  }: InstanceTileProps = $props();

  const status = $derived(statusProp || 'ready');
  const running = $derived(status === 'running');
  const playName = $derived(playLabel || (running ? 'Stop ' : 'Play ') + name);
</script>

<div class={['cb-tile', selected && 'is-selected', 'cb-tile--' + status, className]}>
  <button type="button" class="cb-tile__main" aria-current={selected ? 'true' : undefined} onclick={onSelect} ondblclick={onPlay}>
    <span class="cb-tile__art">
      {#if art}<Content value={art} />{:else}<InstanceIcon {motif} {ground} src={iconSrc} seed={name} size={64} raised />{/if}
    </span>
    <span class="cb-tile__name text-heading">{name}</span>
    {#if meta}<span class="cb-tile__meta text-small"><Content value={meta} /></span>{/if}
  </button>
  {#if status !== 'ready'}
    <div class="cb-tile__status"><StatusTag state={status} detail={status === 'installing' ? null : statusDetail} /></div>
  {/if}
  {#if status === 'installing' && typeof progress === 'number'}
    <ProgressBar value={progress} segments={16} class="cb-tile__progress" ariaValueText={statusDetail} />
  {/if}
  {#if version || loader}
    <div class="cb-tile__tech">
      {#if version}<MetaChip tone={snapshot ? 'snapshot' : undefined}>{version}</MetaChip>{/if}
      {#if loader}<MetaChip>{loader}</MetaChip>{/if}
    </div>
  {/if}
  <div class="cb-tile__play">
    <IconButton
      icon={running ? 'stop' : 'play'}
      variant={running ? 'danger' : 'primary'}
      size="lg"
      label={playName}
      onclick={onPlay}
      disabled={status === 'installing'}
    />
  </div>
</div>
