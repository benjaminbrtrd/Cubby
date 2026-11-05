<!--
  One instance on the library shelf: its latest screenshot (or its emblem on its ground), name,
  version and a status tag only when something is going on.
-->
<script lang="ts">
  import { GROUNDS, Icon, IconButton, InstanceIcon, StatusTag } from '../../cubby'
  import { iconOf } from '../../icon'
  import { loaderName, relativeTime } from '../../format'
  import type { Instance } from '../../types'
  import { t } from '../../i18n.svelte'
  import type { Live } from './Library.svelte'

  let {
    instance,
    live,
    cover,
    detail,
    selected,
    onSelect,
    onPlay,
  }: {
    instance: Instance
    live: Live
    cover: string | null
    /** Running time, task progress or update count, next to the status tag. */
    detail: string | undefined
    selected: boolean
    onSelect: () => void
    onPlay: () => void
  } = $props()

  const icon = $derived(iconOf(instance))
  const ground = $derived(icon.ground ? GROUNDS[icon.ground] : null)
  const tech = $derived(instance.loader.kind === 'vanilla' ? instance.minecraft : `${instance.minecraft} · ${loaderName(instance.loader)}`)
  const STATUS_WORD = $derived<Partial<Record<Live['status'], string>>>({
    running: t('playing'),
    installing: t('installing'),
    crashed: t('crashed last time'),
    update: t('updates ready'),
    download: t('not downloaded yet'),
  })
  const tagged = $derived(live.status !== 'ready' && live.status !== 'download')
</script>

<article class={['card', selected && 'is-selected', live.running && 'is-running']}>
  <button type="button" class="card__main" aria-current={selected ? 'true' : undefined} onclick={onSelect}>
    <span class="cb-visually-hidden">{instance.name}, {tech}{STATUS_WORD[live.status] ? `, ${STATUS_WORD[live.status]}` : ''}</span>
  </button>

  <div class="card__cover" aria-hidden="true">
    {#if cover}
      <img src={cover} alt="" loading="lazy" decoding="async" />
    {:else}
      <span class="card__ground" style:background={ground ? `linear-gradient(135deg, ${ground.base}, ${ground.dark})` : undefined}>
        <InstanceIcon motif={icon.motif} ground={icon.ground} src={icon.src} seed={instance.name} size={64} />
      </span>
    {/if}
    {#if live.status === 'installing' && live.progress != null}
      <span class="card__bar"><span style:width="{live.progress}%"></span></span>
    {/if}
  </div>

  <span class="card__emblem" aria-hidden="true">
    <InstanceIcon motif={icon.motif} ground={icon.ground} src={icon.src} seed={instance.name} size={46} raised />
  </span>

  <div class="card__play">
    <IconButton
      icon={live.running ? 'stop' : 'play'}
      variant={live.running ? 'danger' : 'primary'}
      size="lg"
      label={t(live.running ? 'Stop {name}' : 'Play {name}', { name: instance.name })}
      disabled={!!live.task && !live.running}
      onclick={onPlay}
    />
  </div>

  <div class="card__body" aria-hidden="true">
    <span class="card__name text-body-strong">{instance.name}</span>
    <span class="card__tech text-mono">{tech}</span>
    <span class="card__foot text-small">
      {#if tagged}
        <StatusTag state={live.status} detail={detail} />
      {:else if live.status === 'download'}
        <Icon name="download" size={16} />{t('Not downloaded yet')}
      {:else}
        <Icon name="clock" size={16} />{relativeTime(instance.lastPlayed)}
      {/if}
    </span>
  </div>
</article>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
    transition:
      transform var(--duration-fast),
      box-shadow var(--duration-fast);
  }
  .card:hover {
    transform: translate(-2px, -2px);
    box-shadow: var(--shadow-block-lift);
  }
  .card.is-selected {
    border-color: var(--ink);
  }
  /* The whole card selects; the Play button sits above it. */
  .card__main {
    position: absolute;
    inset: 0;
    z-index: 1;
    padding: 0;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .card__main:focus-visible {
    outline: 3px solid var(--focus);
    outline-offset: 2px;
  }
  .card__cover {
    position: relative;
    height: 118px;
    overflow: hidden;
    background: var(--bg-chrome);
    border-bottom: var(--border-block) solid var(--edge);
  }
  .card__cover img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .card__ground {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .card__ground :global(*) {
    opacity: 0.35;
  }
  .card__bar {
    position: absolute;
    inset: auto 0 0;
    height: 6px;
    background: rgb(21 18 15 / 0.8);
  }
  .card__bar span {
    display: block;
    height: 100%;
    background: var(--diamond);
  }
  .card__emblem {
    position: absolute;
    top: 92px;
    inset-inline-start: 10px;
    pointer-events: none;
  }
  .card__play {
    position: absolute;
    z-index: 2;
    top: 66px;
    inset-inline-end: 10px;
    opacity: 0;
    transition: opacity var(--duration-fast);
  }
  .card:hover .card__play,
  .card.is-selected .card__play,
  .card.is-running .card__play,
  .card__play:focus-within {
    opacity: 1;
  }
  .card__body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 26px var(--space-3) var(--space-3);
    pointer-events: none;
  }
  .card__name,
  .card__tech {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card__tech {
    font-size: 13px;
    color: var(--ink-muted);
  }
  .card__foot {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    margin-top: var(--space-2);
    color: var(--ink-muted);
  }
  @media (prefers-reduced-motion: reduce) {
    .card,
    .card__play {
      transition: none;
    }
    .card:hover {
      transform: none;
    }
  }
</style>
