<!-- "Install to": picks the instance Discover installs to (app.target), among the player's instances. -->
<script lang="ts">
  import { Icon, InstanceIcon } from '../../cubby'
  import { instanceMeta } from '../../format'
  import { iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Instance } from '../../types'
  import Picker from './Picker.svelte'
  import type { Fit } from './discover'

  interface Props {
    variant?: 'field' | 'block'
    /** Optional word per instance ("Works", "Needs Minecraft 1.20.1"), to help pick one that fits. */
    note?: (instance: Instance) => Fit | null
  }

  let { variant = 'field', note }: Props = $props()

  const target = $derived(app.target)
  const items = $derived(app.recent.map((i) => ({ id: i.id, instance: i })))
  const buttonLabel = $derived(target ? `Install to ${target.name}, Minecraft ${instanceMeta(target)}. Change instance` : 'Choose an instance')
</script>

{#if target}
  <Picker {items} value={target.id} onSelect={(id) => app.setTarget(id)} label="Install to" {buttonLabel} {variant}>
    {#snippet trigger()}
      <InstanceIcon {...iconOf(target)} size={32} />
      {#if variant === 'block'}
        <span class="inline">
          <span class="text-body-strong name" title={target.name}>{target.name}</span>
          <span class="text-mono muted">{instanceMeta(target)}</span>
        </span>
      {:else}
        <span class="stack">
          <span class="text-body-strong name" title={target.name}>{target.name}</span>
          <span class="text-mono muted">{instanceMeta(target)}</span>
        </span>
      {/if}
    {/snippet}
    {#snippet option(item)}
      {@const n = note?.(item.instance)}
      <InstanceIcon {...iconOf(item.instance)} size={32} />
      <span class="stack">
        <span class="text-body-strong name" title={item.instance.name}>{item.instance.name}</span>
        <span class="text-mono muted">{instanceMeta(item.instance)}</span>
        {#if n}
          <span class={['note', 'text-small', n.tone]}><Icon name={n.icon} size={18} />{n.word}</span>
        {/if}
      </span>
    {/snippet}
  </Picker>
{/if}

<style>
  .stack {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .inline {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--space-3);
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--ink-muted);
  }
  .note {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .note.ok {
    color: var(--grass-text);
  }
  .note.warn {
    color: var(--gold-text);
  }
  .note.bad {
    color: var(--redstone-text);
  }
  .note.info,
  .note.have {
    color: var(--ink-muted);
  }
</style>
