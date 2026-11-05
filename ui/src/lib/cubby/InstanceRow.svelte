<script lang="ts" module>
  const ROW_HEAD = { name: 'Instance', version: 'Version', loader: 'Loader', mods: 'Mods', played: 'Last played', status: 'Status' };
</script>

<script lang="ts">
  import Content from './Content.svelte';
  import IconButton from './IconButton.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import MetaChip from './MetaChip.svelte';
  import StatusTag from './StatusTag.svelte';
  import type { InstanceRowProps } from './types';
  import { t } from '../i18n.svelte';

  let {
    head,
    labels,
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
    selected,
    onSelect,
    onPlay,
    playLabel,
    group,
    mods,
    lastPlayed,
    class: className,
  }: InstanceRowProps = $props();

  const L = $derived({ ...Object.fromEntries(Object.entries(ROW_HEAD).map(([k, v]) => [k, t(v)])), ...labels } as typeof ROW_HEAD);
  const status = $derived(statusProp || 'ready');
  const running = $derived(status === 'running');
  const playName = $derived(playLabel || t(running ? 'Stop {name}' : 'Play {name}', { name: name ?? '' }));
</script>

{#if head}
  <div class="cb-irow cb-irow--head" aria-hidden="true">
    <div class="cb-irow__main text-label">
      <span></span>
      <span>{L.name}</span>
      <span>{L.version}</span>
      <span>{L.loader}</span>
      <span class="cb-irow__mods">{L.mods}</span>
      <span class="cb-irow__played">{L.played}</span>
      <span>{L.status}</span>
    </div>
    <span class="cb-irow__action"></span>
  </div>
{:else}
  <div class={['cb-irow', selected && 'is-selected', className]}>
    <button type="button" class="cb-irow__main" aria-current={selected ? 'true' : undefined} onclick={onSelect} ondblclick={onPlay}>
      <span class="cb-irow__icon">
        {#if art}<Content value={art} />{:else}<InstanceIcon {motif} {ground} src={iconSrc} seed={name} size={32} />{/if}
      </span>
      <span class="cb-irow__name">
        <span class="cb-irow__title text-body-strong">{name}</span>
        {#if group}<span class="cb-irow__group text-small">{group}</span>{/if}
      </span>
      <span class="cb-irow__cell">{#if version}<MetaChip tone={snapshot ? 'snapshot' : undefined}>{version}</MetaChip>{/if}</span>
      <span class="cb-irow__cell text-mono">{loader || '—'}</span>
      <span class="cb-irow__cell cb-irow__mods text-counter">{mods != null ? mods : '—'}</span>
      <span class="cb-irow__cell cb-irow__played text-small">{lastPlayed || '—'}</span>
      <span class="cb-irow__cell"><StatusTag state={status} detail={statusDetail} /></span>
    </button>
    <span class="cb-irow__action">
      <IconButton
        icon={running ? 'stop' : 'play'}
        variant={running ? 'danger' : 'primary'}
        label={playName}
        onclick={onPlay}
        disabled={status === 'installing'}
      />
    </span>
  </div>
{/if}
