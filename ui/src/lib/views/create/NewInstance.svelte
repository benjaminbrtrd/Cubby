<!-- New instance: a rail of sources (Custom, Import, then the modpack providers) and the chosen source on the right. -->
<script lang="ts">
  import { Banner, Button, Icon, type IconName } from '../../cubby'
  import { app } from '../../state.svelte'
  import CustomForm from './CustomForm.svelte'
  import ImportSource from './ImportSource.svelte'
  import ProviderBrowser from './ProviderBrowser.svelte'
  import ProviderMark from './ProviderMark.svelte'

  const source = $derived(app.newSource)
  const providers = $derived(app.providers)
  const provider = $derived(providers?.find((p) => p.id === source))

  $effect(() => {
    if (!app.providers && !app.providersError) void app.loadProviders()
  })

  // a source that doesn't exist (any more) falls back to Custom
  $effect(() => {
    if (providers && source !== 'custom' && source !== 'import' && !provider) app.newSource = 'custom'
  })

  const fixed: { id: 'custom' | 'import'; label: string; icon: IconName }[] = [
    { id: 'custom', label: 'Custom', icon: 'cube' },
    { id: 'import', label: 'Import', icon: 'import' },
  ]
</script>

<div class="screen">
  <nav class="rail" aria-label="Instance sources">
    <div class="rail__head">
      <button type="button" class="back text-body-strong" onclick={() => app.navigate({ name: 'library' })}>
        Library<span class="sep" aria-hidden="true"><Icon name="chevron-right" size={18} /></span>
      </button>
      <h1 class="text-title title">New instance</h1>
    </div>

    <ul class="items">
      {#each fixed as s (s.id)}
        <li>
          <button type="button" class={['item', source === s.id && 'is-on']} aria-current={source === s.id ? 'page' : undefined} onclick={() => (app.newSource = s.id)}>
            <span class="mark mark--line"><Icon name={s.icon} size={18} /></span>
            <span class="text-body-strong">{s.label}</span>
          </button>
        </li>
      {/each}
    </ul>

    <p class="text-label muted group">Modpacks</p>
    {#if providers}
      <ul class="items">
        {#each providers as p (p.id)}
          <li>
            <button type="button" class={['item', source === p.id && 'is-on']} aria-current={source === p.id ? 'page' : undefined} onclick={() => (app.newSource = p.id)}>
              <ProviderMark id={p.id} />
              <span class="text-body-strong">{p.name}</span>
              {#if !p.available}<span class="lock" data-tip="Needs setup"><Icon name="key" size={18} /></span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {:else if app.providersError}
      <div class="rail__error">
        <p class="text-small">Couldn't load the modpack sources.</p>
        <Button size="sm" icon="update" onclick={() => app.loadProviders()}>Try again</Button>
      </div>
    {:else}
      <ul class="items" aria-hidden="true">
        {#each { length: 5 }, i (i)}<li><span class="sk"></span></li>{/each}
      </ul>
      <span class="cb-visually-hidden" role="status">Loading the modpack sources</span>
    {/if}
  </nav>

  <div class="pane">
    {#if source === 'custom'}
      <CustomForm />
    {:else if source === 'import'}
      <ImportSource />
    {:else if provider}
      {#key provider.id}
        <ProviderBrowser {provider} />
      {/key}
    {:else if app.providersError}
      <div class="pane__note">
        <Banner tone="warning" icon="offline" title="Couldn't load the modpack sources">{app.providersError}</Banner>
      </div>
    {/if}
  </div>
</div>

<style>
  .screen {
    display: flex;
    min-height: 100%;
    height: 100%;
  }
  .title {
    white-space: nowrap;
  }
  .rail {
    box-sizing: border-box;
    flex: 0 0 240px;
    width: 240px;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    overflow: auto;
    padding: var(--space-6) var(--space-4);
    background: var(--bg-chrome);
    border-inline-end: var(--border-chrome) solid var(--edge);
  }
  .rail__head {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-1);
    padding-inline: var(--space-2);
  }
  .rail__head h1 {
    margin: 0;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0;
    color: var(--grass-text);
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .back:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .back:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .sep {
    display: inline-flex;
    color: var(--ink-muted);
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    min-height: 40px;
    padding: var(--space-1) var(--space-2);
    color: var(--ink);
    text-align: start;
    background: transparent;
    border: var(--border-block) solid transparent;
    cursor: pointer;
  }
  .item > .text-body-strong {
    flex: 1;
    min-width: 0;
  }
  .item:hover {
    background: var(--surface);
  }
  .item.is-on {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .item:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .mark {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    box-sizing: border-box;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0;
    border: var(--border-block) solid var(--edge);
  }
  .mark--line {
    color: var(--ink-muted);
    background: var(--surface);
  }
  .lock {
    display: inline-flex;
    color: var(--gold-text);
  }
  .group {
    margin: var(--space-3) 0 calc(-1 * var(--space-1));
    padding-inline: var(--space-2);
  }
  .muted {
    color: var(--ink-muted);
  }
  .rail__error {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding-inline: var(--space-2);
  }
  .rail__error p {
    margin: 0;
  }
  .sk {
    display: block;
    height: 36px;
    background: var(--surface);
  }
  .pane {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .pane__note {
    padding: var(--space-6);
  }
</style>
