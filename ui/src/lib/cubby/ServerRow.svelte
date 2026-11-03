<script lang="ts">
  import Button from './Button.svelte';
  import Icon from './Icon.svelte';
  import StatusTag from './StatusTag.svelte';
  import type { ServerRowProps } from './types';

  let {
    name,
    address,
    iconSrc,
    motd,
    online: onlineProp,
    offlineText,
    players,
    playersLabel,
    ping,
    pingText,
    instance,
    onJoin,
    joinText,
    joinLabel,
    class: className,
  }: ServerRowProps = $props();

  const online = $derived(onlineProp !== false);
  // Latency in bars: 4 under 80 ms, 3 under 150, 2 under 300, 1 above.
  const bars = $derived(ping == null ? 0 : ping < 80 ? 4 : ping < 150 ? 3 : ping < 300 ? 2 : 1);
</script>

<div class={['cb-server', !online && 'is-offline', className]}>
  <span class="cb-server__icon">{#if iconSrc}<img src={iconSrc} alt="" />{:else}<Icon name="servers" size={27} />{/if}</span>
  <div class="cb-server__body">
    <div class="cb-server__head">
      <span class="cb-server__name text-heading">{name}</span>
      <span class="cb-server__address text-mono">{address}</span>
    </div>
    <p class="cb-server__motd text-mono">{online ? motd : offlineText || 'Can’t reach server'}</p>
  </div>
  <div class="cb-server__stats">
    <StatusTag state={online ? 'online' : 'offline'} />
    {#if online && players}
      <span class="cb-server__players" title={playersLabel}>
        <Icon name="accounts" size={18} />
        <span class="text-counter">{players.online + '/' + players.max}</span>
        <span class="cb-visually-hidden">{playersLabel || 'players online'}</span>
      </span>
    {/if}
    {#if online}
      <span class={['cb-ping', 'cb-ping--' + bars]}>
        <span class="cb-ping__bars" aria-hidden="true">
          {#each { length: 4 }, i (i)}<span class={['cb-ping__bar', i < bars && 'is-on']} style:height="{5 + i * 3}px"></span>{/each}
        </span>
        <span class="cb-ping__ms text-mono">{ping == null ? '—' : pingText || ping + ' ms'}</span>
      </span>
    {/if}
  </div>
  <div class="cb-server__join">
    {#if instance}<span class="cb-server__instance text-small"><Icon name="cube" size={18} />{instance}</span>{/if}
    <Button variant={online ? 'primary' : 'secondary'} icon="play" disabled={!online} onclick={onJoin} aria-label={joinLabel}>{joinText || 'Join'}</Button>
  </div>
</div>
