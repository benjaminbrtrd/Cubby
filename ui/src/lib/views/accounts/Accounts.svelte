<!--
  Accounts: the Microsoft accounts that can play, the one in use, and the details of the selected one.
  The module script also holds what the first-launch assistant shares with this screen: the Microsoft
  device-code sign-in and the skin head.
-->
<script lang="ts" module>
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import type { Account, DeviceCode } from '../../types'
  import { Avatar, Banner, Button, Icon, IconButton, ProgressBar } from '../../cubby'

  /** The text of a failed backend call. */
  export function messageOf(e: unknown): string {
    if (typeof e === 'string') return e
    if (e instanceof Error) return e.message
    try {
      return JSON.stringify(e)
    } catch {
      return String(e)
    }
  }

  /** "https://www.microsoft.com/link" → "microsoft.com/link" */
  function shortUri(uri: string): string {
    return uri.replace(/^https?:\/\/(www\.)?/, '').replace(/\/$/, '')
  }

  function openUrl(url: string) {
    api.openUrl(url).catch((e) => app.error(e))
  }

  async function copyCode(code: string) {
    try {
      await navigator.clipboard.writeText(code)
      app.toast(t('Code copied.'), 'success')
    } catch {
      app.toast(t('Can’t copy the code. Select it and press Ctrl C.'), 'error')
    }
  }

  /**
   * Microsoft sign-in with a device code: `start()` gets a code, then waits until the player confirms it on
   * Microsoft's page or the code expires. Used by this screen and by the first-launch assistant.
   */
  export class MicrosoftLogin {
    phase = $state<'idle' | 'starting' | 'waiting' | 'failed'>('idle')
    code = $state<DeviceCode | null>(null)
    error = $state('')
    #run = 0

    get busy(): boolean {
      return this.phase === 'starting' || this.phase === 'waiting'
    }

    /** The new account, or null when the sign-in was cancelled or failed (see `error`). */
    async start(): Promise<Account | null> {
      const run = ++this.#run
      this.phase = 'starting'
      this.code = null
      this.error = ''
      try {
        const code = await api.startMicrosoftLogin()
        if (run !== this.#run) {
          void api.cancelMicrosoftLogin().catch(() => {})
          return null
        }
        this.code = code
        this.phase = 'waiting'
        const account = await api.finishMicrosoftLogin()
        if (run !== this.#run) return null
        this.phase = 'idle'
        this.code = null
        await app.refreshAccounts()
        return account
      } catch (e) {
        if (run !== this.#run) return null
        this.phase = 'failed'
        this.code = null
        this.error = messageOf(e)
        return null
      }
    }

    /** Stops waiting for Microsoft and forgets the code. */
    cancel() {
      const wasBusy = this.busy
      this.#run++
      this.phase = 'idle'
      this.code = null
      this.error = ''
      if (wasBusy) void api.cancelMicrosoftLogin().catch(() => {})
    }
  }

  /** Skin url → what it holds once loaded: a whole skin texture, an image that is already a face, or nothing. */
  const loaded = $state<Record<string, 'skin' | 'face' | 'error'>>({})

  function seen(url: string, img: HTMLImageElement) {
    loaded[url] = img.naturalWidth >= 64 ? 'skin' : 'face'
  }

  /** "3f1c9a2e7b4d…" → "3f1c9a2e-7b4d-4e8a-9c61-52d0e8b4a7f3" (Mojang ids are undashed). */
  export function dashedUuid(id: string): string {
    return /^[0-9a-f]{32}$/i.test(id) ? `${id.slice(0, 8)}-${id.slice(8, 12)}-${id.slice(12, 16)}-${id.slice(16, 20)}-${id.slice(20)}` : id
  }

  export { skinHead, deviceCode }
</script>

<script lang="ts">
  import { onMount } from 'svelte'
  import { Dialog, StatusTag } from '../../cubby'
  import { num } from '../../format'

  const uid = $props.id()

  /** The account shown in the side panel (the one in use unless the player picks another). */
  let selectedId = $state<string | null>(null)
  const selected = $derived(app.accounts.find((a) => a.id === selectedId) ?? app.defaultAccount)
  const msaReady = $derived(!!app.info?.msaConfigured)

  onMount(() => {
    void app.refreshAccounts()
  })

  /* Add a Microsoft account */
  const login = new MicrosoftLogin()
  let microsoftOpen = $state(false)

  $effect(() => () => login.cancel())

  async function addMicrosoft() {
    microsoftOpen = true
    if (!msaReady) return
    const account = await login.start()
    if (account) {
      microsoftOpen = false
      selectedId = account.id
      app.toast(t('Signed in as {username}.', { username: account.username }), 'success')
    }
  }

  function closeMicrosoft() {
    login.cancel()
    microsoftOpen = false
  }

  function openAccountSettings() {
    closeMicrosoft()
    app.navigate({ name: 'settings', section: 'accounts' })
  }

  /* Use, sign out, remove */
  async function use(account: Account) {
    try {
      await api.setDefaultAccount(account.id)
      await app.refreshAccounts()
      app.toast(t('Cubby now plays as {username}.', { username: account.username }), 'success')
    } catch (e) {
      app.error(e)
    }
  }

  let removing = $state<Account | null>(null)
  let removeBusy = $state(false)
  /** Who plays after the account in use is removed: the backend picks the first one left. */
  const nextInUse = $derived(removing?.isDefault ? app.accounts.find((a) => a.id !== removing?.id) : undefined)

  async function confirmRemove() {
    const account = removing
    if (!account || removeBusy) return
    removeBusy = true
    try {
      await api.removeAccount(account.id)
      await app.refreshAccounts()
      if (selectedId === account.id) selectedId = null
      removing = null
      app.toast(t('Signed out of {username}.', { username: account.username }))
    } catch (e) {
      app.error(e)
    } finally {
      removeBusy = false
    }
  }

  async function copyUuid(id: string) {
    try {
      await navigator.clipboard.writeText(dashedUuid(id))
      app.toast(t('UUID copied.'), 'success')
    } catch {
      app.toast(t('Can’t copy the UUID. Select it and press Ctrl C.'), 'error')
    }
  }

  const SKIN_PAGE = 'https://www.minecraft.net/msaprofile/mygames/editskin'
</script>

<!-- A skin's face (front of the head with its hat layer), cropped from the skin texture and drawn pixelated.
     `label` empty = decorative. -->
{#snippet skinHead(url: string | null, name: string, size: number, label: string)}
  {#if !url || loaded[url] === 'error'}
    <Avatar {name} {size} alt={label} />
  {:else if loaded[url] === 'face'}
    <Avatar src={url} {name} {size} alt={label} />
  {:else}
    {@const inner = size - 4}
    <span class={['skin', loaded[url] !== 'skin' && 'is-pending']} style:width="{size}px" style:height="{size}px" role={label ? 'img' : undefined} aria-label={label || undefined} aria-hidden={label ? undefined : true}>
      <img
        class="skin__layer"
        src={url}
        alt=""
        style:width="{inner * 8}px"
        style:left="{-inner}px"
        style:top="{-inner}px"
        onload={(e) => seen(url, e.currentTarget as HTMLImageElement)}
        onerror={() => (loaded[url] = 'error')}
      />
      <img class="skin__layer" src={url} alt="" style:width="{inner * 8}px" style:left="{-5 * inner}px" style:top="{-inner}px" />
    </span>
  {/if}
{/snippet}

<!-- The device code: where to go, the code to type, and a calm wait until Microsoft answers. -->
{#snippet deviceCode(login: MicrosoftLogin, retry: () => void)}
  <div class="dc">
    {#if login.phase === 'starting'}
      <ProgressBar label={t('Asking Microsoft for a code')} />
    {:else if login.phase === 'waiting' && login.code}
      {@const code = login.code}
      <ol class="dc__steps">
        <li class="dc__step">
          <span class="dc__n text-body-strong" aria-hidden="true">1</span>
          <div class="dc__main">
            <p class="text-body-strong">{t('Open Microsoft’s sign-in page in your browser')}</p>
            <Button icon="globe" onclick={() => openUrl(code.verificationUri)}>{t('Open')} <span class="text-mono">{shortUri(code.verificationUri)}</span></Button>
          </div>
        </li>
        <li class="dc__step">
          <span class="dc__n text-body-strong" aria-hidden="true">2</span>
          <div class="dc__main">
            <p class="text-body-strong">{t('Enter this code, on this PC or any other device')}</p>
            <div class="dc__code">
              <code class="dc__value text-heading">{code.userCode}</code>
              <IconButton icon="copy" label={t('Copy code')} variant="secondary" onclick={() => copyCode(code.userCode)} />
            </div>
          </div>
        </li>
      </ol>
      <ProgressBar
        label={t('Waiting for you to confirm')}
        detail={t('The code works for {expiresIn} min. This page moves on by itself.', { expiresIn: Math.max(1, Math.round(code.expiresIn / 60)) })}
      />
    {:else if login.phase === 'failed'}
      {#snippet retryAction()}<Button size="sm" icon="update" onclick={retry}>{t('Try again')}</Button>{/snippet}
      <Banner tone="danger" title={t('Sign-in didn’t finish')} actions={retryAction}>{login.error}</Banner>
    {/if}
  </div>
{/snippet}

{#snippet accountRow(a: Account)}
  {@const isSelected = selected?.id === a.id}
  <li>
    <article class={['acc', isSelected && 'is-selected']}>
      <button type="button" class="acc__pick" aria-current={isSelected ? 'true' : undefined} onclick={() => (selectedId = a.id)}>
        {@render skinHead(a.skinUrl, a.username, 48, '')}
        <span class="acc__text">
          <span class="acc__name">
            <span class="text-heading">{a.username}</span>
            {#if a.isDefault}<StatusTag state="ready">{t('In use')}</StatusTag>{/if}
          </span>
          <span class="acc__kind text-body">
            <Icon name="globe" size={18} />
            {t('Microsoft account')}
          </span>
          <span class="text-small muted">
            {t('Owns Minecraft: Java Edition · stays signed in')}
          </span>
        </span>
      </button>
      <div class="acc__actions">
        {#if !a.isDefault}<Button size="sm" onclick={() => use(a)}>{t('Use this account')}</Button>{/if}
        <Button size="sm" variant="ghost" onclick={() => (removing = a)}>{t('Sign out')}</Button>
      </div>
    </article>
  </li>
{/snippet}

<div class="screen">
  <div class="cols">
    <div class="content">
      <header class="head">
        <h1 class="text-title">{t('Accounts')}</h1>
        <span class="grow"></span>
        {#if app.accounts.length}
          <Button variant="primary" icon="plus" onclick={addMicrosoft}>{t('Add Microsoft account')}</Button>
        {/if}
      </header>

      {#if app.accounts.length === 0}
        <div class="empty">
          <Icon name="accounts" size={36} />
          <h2 class="text-heading">{t('No account yet')}</h2>
          <p class="text-body muted">{t('Add a Microsoft account that owns Minecraft: Java Edition to play.')}</p>
          <div class="empty__actions">
            <Button variant="primary" icon="plus" onclick={addMicrosoft}>{t('Add Microsoft account')}</Button>
          </div>
        </div>
      {:else}
        <p class="text-body muted lead">{t('Cubby plays every instance with the account in use. Pick one to see its details.')}</p>

        {#if !app.defaultAccount}
          <Banner tone="info" title={t('Pick the account to play with')}>{t('Choose Use this account on one of your accounts.')}</Banner>
        {/if}

        <section class="group" aria-labelledby="{uid}-ms">
          <h2 id="{uid}-ms" class="text-label muted">{t('Microsoft accounts · {n}', { n: num(app.accounts.length) })}</h2>
          <ul class="list">
            {#each app.accounts as a (a.id)}{@render accountRow(a)}{/each}
          </ul>
        </section>
      {/if}
    </div>

    {#if selected}
      <aside class="aside" aria-label={t('Account details')}>
        {@render skinHead(selected.skinUrl, selected.username, 128, t('Skin of {username}', { username: selected.username }))}

        <div>
          <h2 class="text-title name">{selected.username}</h2>
          <p class="aside__kind text-body">
            <Icon name="globe" size={18} />
            {t('Microsoft account')}
          </p>
          <p class="text-small muted">{t('Owns Minecraft: Java Edition')}</p>
        </div>

        {#if selected.isDefault}
          <div class="aside__use">
            <StatusTag state="ready">{t('In use')}</StatusTag>
            <p class="text-small muted">{t('Cubby plays every instance with this account.')}</p>
          </div>
        {:else}
          <div><Button onclick={() => selected && use(selected)}>{t('Use this account')}</Button></div>
        {/if}

        <div>
          <p class="text-label muted label">UUID</p>
          <div class="uuid">
            <code class="text-mono uuid__value">{dashedUuid(selected.id)}</code>
            <IconButton icon="copy" label={t('Copy UUID')} size="sm" onclick={() => selected && copyUuid(selected.id)} />
          </div>
          <p class="text-small muted note">
            {t('Your player ID on every server.')}
          </p>
        </div>

        <a
          class="link text-body-strong"
          href={SKIN_PAGE}
          onclick={(e) => {
            e.preventDefault()
            openUrl(SKIN_PAGE)
          }}>{t('Change skin and cape on')} <span class="text-mono">minecraft.net</span></a
        >
        <p class="secure text-small muted">
          <Icon name="key" size={18} />
          <span>
            {t('Sign-in tokens are stored in plain text in')} <span class="text-mono">accounts.json</span>{t(', in Cubby’s data folder on this PC. Signing out deletes them. Cubby never sees your password.')}
          </span>
        </p>
      </aside>
    {/if}
  </div>
</div>

{#if microsoftOpen}
  {#snippet microsoftActions()}
    {#if msaReady}
      <Button variant="ghost" onclick={closeMicrosoft}>{t('Cancel')}</Button>
    {:else}
      <Button variant="ghost" onclick={closeMicrosoft}>{t('Close')}</Button>
      <Button variant="primary" icon="settings" onclick={openAccountSettings}>{t('Open Settings')}</Button>
    {/if}
  {/snippet}
  <Dialog title={t('Add a Microsoft account')} icon="accounts" onClose={closeMicrosoft} actions={microsoftActions}>
    {#if msaReady}
      <p>{t('Sign-in happens on Microsoft’s page. Cubby never sees your password.')}</p>
      {@render deviceCode(login, addMicrosoft)}
    {:else}
      <p>{t('Microsoft sign-in needs an application id, and this copy of Cubby doesn’t have one.')}</p>
      <p>{t('Add it in Settings › Accounts. Until then, Cubby only starts the Minecraft demo.')}</p>
    {/if}
  </Dialog>
{/if}

{#if removing}
  {@const r = removing}
  {#snippet removeActions()}
    <Button variant="ghost" onclick={() => (removing = null)}>{t('Cancel')}</Button>
    <Button variant="danger" busy={removeBusy} onclick={confirmRemove}>{t('Sign out')}</Button>
  {/snippet}
  <Dialog
    tone="danger"
    icon="accounts"
    title={t('Sign out of {username}?', { username: r.username })}
    onClose={() => (removing = null)}
    actions={removeActions}
  >
    <p>{t('Cubby deletes this account’s sign-in from this PC. Your Microsoft account, skin and worlds stay as they are, and you can sign in again any time.')}</p>
    {#if r.isDefault}
      <p class="muted">
        {nextInUse ? t('Cubby then plays as {username}.', { username: nextInUse.username }) : t('You’ll need to sign in again to play.')}
      </p>
    {/if}
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .cols {
    flex: 1;
    display: flex;
    align-items: stretch;
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .screen :where(h1, h2, p),
  .dc :where(p) {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .muted {
    color: var(--ink-muted);
  }
  .lead {
    max-width: 640px;
  }

  /* Empty state */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-6);
    color: var(--ink);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .empty__actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }

  /* Account blocks */
  .group {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .acc {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: 14px var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .acc:has(.acc__pick:hover) {
    background: var(--surface-hover);
  }
  /* Selection is an ink frame, never a colour alone */
  .acc.is-selected {
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .acc__pick {
    flex: 1 1 320px;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: 0;
    color: inherit;
    text-align: start;
    font: inherit;
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .acc__pick:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .acc__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .acc__name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    overflow-wrap: anywhere;
  }
  .acc__kind,
  .aside__kind {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .acc__actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  /* Side panel */
  .aside {
    box-sizing: border-box;
    flex: none;
    width: var(--size-inspector);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .aside > .skin,
  .aside > :global(.cb-avatar) {
    box-shadow: var(--shadow-block);
  }
  .name {
    overflow-wrap: anywhere;
  }
  .aside__kind {
    margin-top: var(--space-1);
  }
  .aside__use {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }
  .label {
    margin-bottom: 6px;
  }
  .uuid {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 6px 6px 6px 10px;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .uuid__value {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .note {
    margin-top: 6px;
  }
  .link {
    align-self: flex-start;
    color: var(--link);
  }
  .link:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .secure {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .secure :global(.cb-icon) {
    margin-top: 1px;
  }

  /* Skin head */
  .skin {
    box-sizing: border-box;
    position: relative;
    display: inline-flex;
    flex: none;
    overflow: hidden;
    background: var(--surface-hover);
    border: var(--border-block) solid var(--edge);
  }
  .skin__layer {
    position: absolute;
    display: block;
    max-width: none;
    height: auto;
    image-rendering: pixelated;
  }
  .skin.is-pending .skin__layer {
    visibility: hidden;
  }
  /* Device code */
  .dc {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    margin-top: var(--space-3);
  }
  .dc__steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dc__step {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .dc__n {
    box-sizing: border-box;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: var(--surface-hover);
    border: var(--border-block) solid var(--edge);
  }
  .dc__main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .dc__code {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .dc__value {
    padding: 6px var(--space-3);
    font-family: var(--font-mono);
    font-variant-ligatures: none;
    letter-spacing: 3px;
    color: var(--ink);
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
    user-select: all;
  }
</style>
