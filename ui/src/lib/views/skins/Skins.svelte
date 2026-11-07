<!-- skin and cape of a Microsoft account. Pick a skin and a cape on the left, see the player on the right,
     press Apply: the change goes to Mojang through the backend (see skins.svelte.ts). -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, IconButton, SegmentedControl, StatusTag, TextField } from '../../cubby'
  import { app } from '../../state.svelte'
  import { getSkin, normalizeSkin, studio, type Model } from '../../skins.svelte'
  import CapeThumb from './CapeThumb.svelte'
  import Player3D from './Player3D.svelte'
  import SkinHead from './SkinHead.svelte'

  let { accountId }: { accountId: string } = $props()

  const account = $derived(app.accounts.find((a) => a.id === accountId))

  $effect(() => {
    if (account) void studio.open(account.id, account.username)
  })

  let walking = $state(false)
  const capeOf = (id: string | null) => studio.owned.find((c) => c.id === id) ?? null
  const draftCape = $derived(capeOf(studio.draft.capeId))
  const skin = $derived(studio.selected)

  // add a skin
  let fileInput: HTMLInputElement | undefined = $state()
  let usernameOpen = $state(false)
  let username = $state('')
  let adding = $state(false)
  const usernameOk = $derived(/^\w{3,16}$/.test(username.trim()))

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
  }

  async function addFromFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    const url = URL.createObjectURL(file)
    try {
      const img = new Image()
      img.src = url
      await img.decode()
      normalizeSkin(img)
      studio.addSkin(file.name.replace(/\.png$/i, ''), url)
    } catch (err) {
      URL.revokeObjectURL(url)
      app.toast(message(err) || t('A skin is a 64×64 or 64×32 PNG.'), 'error')
    }
  }

  async function addFromUsername() {
    const name = username.trim()
    if (!usernameOk) return
    adding = true
    try {
      const url = `https://minotar.net/skin/${name}`
      await getSkin(url)
      studio.addSkin(name, url)
      usernameOpen = false
      username = ''
    } catch {
      app.toast(t('No skin found for {name}.', { name }), 'error')
    } finally {
      adding = false
    }
  }

  async function apply() {
    try {
      await studio.apply()
      app.toast(t('Skin and cape saved.'), 'success')
      // the face in the title bar and in Accounts follows
      app.accounts = await api.listAccounts()
    } catch (e) {
      app.error(e)
    }
  }

  const models = $derived([
    { value: 'classic', label: t('Classic') },
    { value: 'slim', label: t('Slim') },
  ])
  const poses = $derived([
    { value: 'still', label: t('Still') },
    { value: 'walking', label: t('Walking') },
  ])
</script>

{#snippet retry()}
  <Button size="sm" icon="update" busy={studio.loading} onclick={() => studio.load()}>{t('Try again')}</Button>
{/snippet}

{#snippet addActions()}
  <Button variant="ghost" onclick={() => (usernameOpen = false)}>{t('Cancel')}</Button>
  <Button variant="primary" icon="plus" busy={adding} disabled={!usernameOk || adding} onclick={addFromUsername}>{t('Add skin')}</Button>
{/snippet}

<div class="screen">
  <div class="layout">
    <div class="content">
      <nav class="crumbs" aria-label={t('Breadcrumb')}>
        <button type="button" class="back text-body-strong" onclick={() => app.navigate({ name: 'accounts' })}>{t('Accounts')}<span class="sep" aria-hidden="true"><Icon name="chevron-right" size={18} /></span></button>
        <span class="text-body muted">{account?.username}</span>
      </nav>
      <h1 class="text-title">{t('Skin and cape')}</h1>
      {#if studio.error}
        <Banner tone="warning" icon="warning" title={t('Cubby couldn’t read your skin and capes')} actions={retry}>{studio.error}</Banner>
      {/if}

      <section class="section" aria-labelledby="sk-skins">
        <h2 id="sk-skins" class="text-label muted">{t('Skins')}</h2>
        <ul class="grid">
          <li class={['card', studio.draft.skinId === 'current' && 'is-shown']}>
            <SkinHead url={studio.current.url} size={48} />
            <div class="card__text">
              <button type="button" class="card__hit text-body-strong" aria-pressed={studio.draft.skinId === 'current'} onclick={() => studio.pickSkin('current')}>{t('Current skin')}</button>
              <span class="text-small muted">{studio.current.model === 'slim' ? t('Slim') : t('Classic')}</span>
            </div>
            <StatusTag state="ready">{t('In use')}</StatusTag>
          </li>
          {#each studio.library as s (s.id)}
            <li class={['card', studio.draft.skinId === s.id && 'is-shown']}>
              <SkinHead url={s.url} size={48} />
              <div class="card__text">
                <button type="button" class="card__hit text-body-strong" aria-pressed={studio.draft.skinId === s.id} onclick={() => studio.pickSkin(s.id)}>{s.name}</button>
                <span class="text-small muted">{s.model === 'slim' ? t('Slim') : t('Classic')}</span>
              </div>
              <span class="card__act"><IconButton icon="trash" size="sm" label={t('Remove {name}', { name: s.name })} onclick={() => studio.removeSkin(s.id)} /></span>
            </li>
          {/each}
          <li class="card card--add">
            <span class="text-body-strong">{t('Add a skin')}</span>
            <span class="card__row">
              <Button size="sm" icon="import" onclick={() => fileInput?.click()}>{t('From a file')}</Button>
              <Button size="sm" icon="accounts" onclick={() => (usernameOpen = true)}>{t('From a name')}</Button>
            </span>
            <input bind:this={fileInput} type="file" accept="image/png" class="file" tabindex="-1" aria-hidden="true" onchange={addFromFile} />
          </li>
        </ul>
      </section>

      <section class="section" aria-labelledby="sk-default">
        <h2 id="sk-default" class="text-label muted">{t('Minecraft skins')}</h2>
        {#if studio.defaults.length}
          <ul class="grid">
            {#each studio.defaults as s (s.id)}
              <li class={['card', studio.draft.skinId === s.id && 'is-shown']}>
                <SkinHead url={s.url} size={48} />
                <div class="card__text">
                  <button type="button" class="card__hit text-body-strong" aria-pressed={studio.draft.skinId === s.id} onclick={() => studio.pickSkin(s.id)}>{s.name}</button>
                </div>
              </li>
            {/each}
          </ul>
        {:else if studio.defaultsError}
          <p class="text-small muted">{studio.defaultsError}</p>
        {:else}
          <p class="text-small muted">{t('Loading…')}</p>
        {/if}
      </section>

      <section class="section" aria-labelledby="sk-capes">
        <h2 id="sk-capes" class="text-label muted">{t('Capes')}</h2>
        <ul class="grid">
          <li class={['card', studio.draft.capeId === null && 'is-shown']}>
            <CapeThumb cape={null} scale={3} />
            <div class="card__text">
              <button type="button" class="card__hit text-body-strong" aria-pressed={studio.draft.capeId === null} onclick={() => (studio.draft = { ...studio.draft, capeId: null })}>{t('No cape')}</button>
            </div>
            {#if studio.worn === null}<StatusTag state="ready">{t('In use')}</StatusTag>{/if}
          </li>
          {#each studio.owned as c (c.id)}
            <li class={['card', studio.draft.capeId === c.id && 'is-shown']}>
              <CapeThumb cape={c} scale={3} />
              <div class="card__text">
                <button type="button" class="card__hit text-body-strong" aria-pressed={studio.draft.capeId === c.id} onclick={() => (studio.draft = { ...studio.draft, capeId: c.id })}>{c.name}</button>
              </div>
              {#if studio.worn === c.id}<StatusTag state="ready">{t('In use')}</StatusTag>{/if}
            </li>
          {/each}
        </ul>
      </section>
    </div>

    <aside class="aside" aria-label={t('Preview')}>
      <div class="stage">
        <Player3D skin={studio.previewUrl} model={studio.draft.model} cape={draftCape?.url ?? null} {walking} width={296} height={380} label={t('{name} wearing {skin}', { name: account?.username ?? '', skin: skin.name })} />
      </div>
      <SegmentedControl label={t('Pose')} options={poses} value={walking ? 'walking' : 'still'} onChange={(v) => (walking = v === 'walking')} />
      <div class="field">
        <span class="text-label muted">{t('Arms')}</span>
        <SegmentedControl label={t('Arms')} options={models} value={studio.draft.model} onChange={(v) => (studio.draft = { ...studio.draft, model: v as Model })} />
      </div>
      <dl class="facts">
        <dt class="text-small muted">{t('Skin')}</dt>
        <dd class="text-body">{studio.draft.skinId === 'current' ? t('Current skin') : skin.name}</dd>
        <dt class="text-small muted">{t('Cape')}</dt>
        <dd class="text-body">{draftCape?.name ?? t('No cape')}</dd>
      </dl>
      <div class="actions">
        <Button variant="primary" size="lg" icon="check" busy={studio.applying} disabled={!studio.changed || studio.applying} onclick={apply} class="wide">{t('Apply')}</Button>
        <Button variant="ghost" disabled={!studio.changed || studio.applying} onclick={() => studio.revert()} class="wide">{t('Revert')}</Button>
      </div>
    </aside>
  </div>
</div>

{#if usernameOpen}
  <Dialog title={t('Add a skin from a name')} icon="accounts" onClose={() => (usernameOpen = false)} actions={addActions}>
    <TextField label={t('Minecraft name')} placeholder="Notch" bind:value={username} maxlength={16} autocomplete="off" spellcheck={false} onkeydown={(e) => e.key === 'Enter' && void addFromUsername()} />
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .layout {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  h1,
  h2,
  dl,
  dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .back {
    display: inline-flex;
    align-items: center;
    padding: 0;
    color: var(--grass-text);
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .back:hover {
    text-decoration: underline;
  }
  .sep {
    display: inline-flex;
    color: var(--ink-muted);
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--space-3);
  }
  .card {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    padding: var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .card:hover {
    background: var(--surface-hover);
  }
  .card.is-shown {
    border-color: var(--ink);
  }
  .card__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .card__hit {
    all: unset;
    cursor: pointer;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .card__hit::after {
    content: '';
    position: absolute;
    inset: 0;
  }
  .card__hit:focus-visible::after {
    box-shadow: var(--focus-ring);
  }
  .card__act {
    position: relative;
    z-index: 1;
  }
  .card--add {
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    background: transparent;
    border-style: dashed;
    box-shadow: none;
  }
  .card--add:hover {
    background: transparent;
  }
  .card__row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .file {
    display: none;
  }

  .aside {
    box-sizing: border-box;
    flex: none;
    width: var(--size-inspector);
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .stage {
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-well);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .facts {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--space-2) var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: auto;
  }
  .actions :global(.wide) {
    width: 100%;
  }
</style>
