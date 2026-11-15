<!-- A modpack page as its own page (route `pack`), so Back from one of its mods comes back to it. The summary comes
     from the list it was opened from, else from the provider. -->
<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button } from '../../cubby'
  import { app } from '../../state.svelte'
  import type { PackSummary, ProviderId } from '../../types'
  import PackPage from './PackPage.svelte'

  let { provider: providerId, id }: { provider: ProviderId; id: string } = $props()

  $effect(() => {
    if (!app.providers && !app.providersError) void app.loadProviders()
  })
  const provider = $derived(app.providers?.find((p) => p.id === providerId) ?? null)

  // the page is keyed by provider and id in App: these don't change while it lives
  let summary = $state<PackSummary | null>(untrack(() => app.packSummaries.get(`${providerId}:${id}`) ?? null))
  let error = $state<string | null>(null)
  let tries = $state(0)
  $effect(() => {
    void tries
    if (summary) return
    let stale = false
    error = null
    api
      .packDetails(providerId, id)
      .then((d) => {
        if (stale) return
        summary = d.summary
        app.packSummaries.set(`${providerId}:${id}`, d.summary)
      })
      .catch((e) => {
        if (!stale) error = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
      })
    return () => {
      stale = true
    }
  })
</script>

{#if provider && summary}
  <PackPage {provider} {summary} onBack={() => app.back()} />
{:else if error || app.providersError}
  <div class="note">
    {#snippet actions()}
      <Button size="sm" icon="update" onclick={() => tries++}>{t('Try again')}</Button>
      <Button size="sm" variant="ghost" onclick={() => app.back()}>{t('Back')}</Button>
    {/snippet}
    <Banner tone="warning" title={t('Couldn\'t open this modpack')} {actions}>{error ?? app.providersError}</Banner>
  </div>
{:else}
  <p class="note text-small muted" role="status">{t('Loading the modpack…')}</p>
{/if}

<style>
  .note {
    margin: 0;
    padding: var(--space-6);
  }
  .muted {
    color: var(--ink-muted);
  }
</style>
