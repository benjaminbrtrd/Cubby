<!-- Share an instance as a modpack: Cubby lists what it found on Modrinth, you mark what can be skipped,
     and you get a link to paste anywhere. -->
<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, InstanceIcon, MetaChip, TextField } from '../../cubby'
  import { loaderFull, plural } from '../../format'
  import { iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import { encodePack, linkFor } from '../../share'
  import type { ModFile } from '../../types'

  let { id, onClose }: { id: string; onClose: () => void } = $props()

  const inst = $derived(app.instance(id))

  let loading = $state(true)
  let failed = $state<string | null>(null)
  let mods = $state<ModFile[]>([])

  $effect(() => {
    let stale = false
    loading = true
    failed = null
    Promise.all(untrack(() => [app.loadSources(id, true), api.listMods(id)]))
      .then(([, list]) => {
        if (!stale) mods = list
      })
      .catch((e) => {
        if (!stale) failed = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
      })
      .finally(() => {
        if (!stale) loading = false
      })
    return () => {
      stale = true
    }
  })

  // what Modrinth knows: these go in the pack
  const files = $derived(app.sources[id] ?? [])
  const unknown = $derived(Math.max(0, mods.length - files.filter((f) => mods.some((m) => m.fileName === f.fileName)).length))

  let optional = $state<Record<string, boolean>>({})
  let note = $state('')
  let code = $state<string | null>(null)
  const link = $derived(code ? linkFor(code) : null)
  let busy = $state(false)
  const optionalCount = $derived(Object.values(optional).filter(Boolean).length)

  async function create() {
    if (!inst || !files.length) return
    busy = true
    try {
      code = await encodePack({
        name: inst.name,
        minecraft: inst.minecraft,
        loader: inst.loader,
        versions: files.map((f) => f.versionId),
        optional: files.filter((f) => optional[f.versionId]).map((f) => f.versionId),
        note,
        creator: app.defaultAccount ? { name: app.defaultAccount.username, uuid: app.defaultAccount.id.replace(/-/g, '').toLowerCase() } : null,
      })
    } catch (e) {
      app.error(e)
    } finally {
      busy = false
    }
  }

  async function copy(text: string, done: string) {
    try {
      await navigator.clipboard.writeText(text)
      app.toast(done, 'success')
    } catch {
      app.toast(t('Couldn’t copy. Select the link and copy it by hand.'), 'error')
    }
  }
</script>

{#snippet actions()}
  {#if !link}
    <Button variant="ghost" onclick={onClose}>{t('Cancel')}</Button>
    <Button variant="primary" icon="globe" {busy} disabled={busy || loading || !files.length} onclick={create}>{t('Create link')}</Button>
  {:else}
    <Button onclick={onClose}>{t('Done')}</Button>
  {/if}
{/snippet}

<Dialog wide title={link ? t('Your modpack is ready to share') : t('Share {name}', { name: inst?.name ?? '' })} icon="globe" {onClose} {actions}>
  {#if inst}
    <div class="pack">
      <InstanceIcon {...iconOf(inst)} seed={inst.name} size={48} raised />
      <div class="pack__text">
        <span class="text-heading">{inst.name}</span>
        <span class="text-small muted text-mono">{inst.minecraft} · {loaderFull(inst.loader)}</span>
      </div>
      {#if !loading}<MetaChip>{plural(files.length, 'file', 'files')}</MetaChip>{/if}
    </div>

    {#if failed}
      <Banner tone="warning" icon="warning" title={t('Cubby couldn’t read this instance')}>{failed}</Banner>
    {:else if loading}
      <p class="text-body muted">{t('Looking up your mods on Modrinth…')}</p>
    {:else if !link}
      {#if !files.length}
        <Banner tone="info" icon="info" title={t('Nothing to share yet')}>
          {t('Cubby shares what it finds on Modrinth. Add mods from Discover first.')}
        </Banner>
      {:else}
        <section class="col" aria-labelledby="sp-mods">
          <div class="head">
            <h3 id="sp-mods" class="text-label muted grow">{t('Optional files')}</h3>
            <span class="text-small muted">{optionalCount ? t('{n} optional', { n: optionalCount }) : t('All required')}</span>
          </div>
          <ul class="mods">
            {#each files as f (f.versionId)}
              <li class="mod">
                <span class="text-body-strong grow name">{f.title}</span>
                <span class="text-small muted text-mono">{f.versionNumber}</span>
                <button type="button" class={['opt', optional[f.versionId] && 'is-on']} aria-pressed={!!optional[f.versionId]} onclick={() => (optional[f.versionId] = !optional[f.versionId])}>
                  {optional[f.versionId] ? t('Optional') : t('Required')}
                </button>
              </li>
            {/each}
          </ul>
          <TextField label={t('Description')} placeholder={t('Our survival server pack')} maxlength={140} bind:value={note} />
          {#if unknown > 0}
            <p class="text-small muted">{t('{n} files are not on Modrinth, so they are left out.', { n: unknown })}</p>
          {/if}
          <p class="text-small muted">{t('Never shared: your worlds, screenshots, account, settings and options.')}</p>
        </section>
      {/if}
    {:else}
      <div class="done">
        <TextField label={t('Link')} value={link} readonly mono icon="globe" />
        <div class="row">
          <Button variant="primary" icon="copy" onclick={() => link && copy(link, t('Link copied.'))}>{t('Copy link')}</Button>
          <Button icon="copy" onclick={() => link && copy(`${inst.name} · Minecraft ${inst.minecraft}, ${loaderFull(inst.loader)}\n${link}`, t('Copied with a short description.'))}>{t('Copy with description')}</Button>
        </div>
        {#if link.length > 1800}<p class="text-small muted">{t('This link is long. Some chat apps cut very long messages: send it as a file if that happens.')}</p>{/if}
      </div>
    {/if}
  {/if}
</Dialog>

<style>
  .pack {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding-bottom: var(--space-4);
    margin-bottom: var(--space-4);
    border-bottom: var(--border-block) solid var(--line);
  }
  .pack__text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  h3,
  p {
    margin: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .mods {
    list-style: none;
    margin: 0;
    padding: var(--space-2);
    max-height: 240px;
    overflow: auto;
    background: var(--field);
    box-shadow: var(--shadow-well);
    border: var(--border-block) solid var(--edge);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .mod {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: var(--size-control-sm);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .opt {
    font: inherit;
    font-size: 12px;
    padding: 2px 8px;
    color: var(--ink-muted);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .opt.is-on {
    color: var(--on-gold);
    background: var(--gold);
  }
  .done {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
</style>
