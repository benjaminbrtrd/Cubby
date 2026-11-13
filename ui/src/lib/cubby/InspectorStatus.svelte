<!-- One state in a panel's status line: a square of colour and a word. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let { tone = 'ok', children }: { tone?: 'ok' | 'update' | 'off' | 'warn' | 'bad'; children: Snippet } = $props();
</script>

<span class={['cb-ins-st', `cb-ins-st--${tone}`]}>{@render children()}</span>
