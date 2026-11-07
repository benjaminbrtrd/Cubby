<!--
  Settings: one long page of blocks, with the section list on the left. Every control saves as it changes
  (app.saveSettings applies theme and font right away).
-->
<script lang="ts">
  import { tick, untrack } from 'svelte'
  import { api } from '../../api'
  import { osName } from '../../platform'
  import { app, type SettingsSection } from '../../state.svelte'
  import { bytes, fold, num, timeOfDay } from '../../format'
  import { switchLanguage, t } from '../../i18n.svelte'
  import type { DataFolder, JavaInstall, Settings } from '../../types'
  import { Button, Dialog, Icon, IconButton, Logo, MetaChip, SegmentedControl, Slider, StatusTag, Switch, TextField } from '../../cubby'

  let { section }: { section: SettingsSection } = $props()

  const uid = $props.id()

  type BlockId = 'language' | 'java' | 'game' | 'downloads' | 'storage' | 'accounts' | 'updates' | 'about'

  /** The blocks, in page order; `words` feed the search field. */
  const BLOCKS: { id: BlockId; label: string; words: string }[] = [
    { id: 'language', label: t('Language'), words: 'english french français langue translation locale' },
    { id: 'java', label: t('Java and memory'), words: 'java runtime automatic memory ram jvm arguments installs detected' },
    { id: 'game', label: t('Game'), words: 'window size width height fullscreen starts launch minimize keep close play time track show' },
    { id: 'downloads', label: t('Downloads'), words: 'parallel concurrency mod updates check modrinth proxy network system manual address' },
    { id: 'storage', label: t('Storage'), words: 'data folder disk space size cache clean up instances libraries assets logs trash open' },
    { id: 'accounts', label: t('Accounts'), words: 'microsoft application client id azure sign in curseforge api key' },
    { id: 'updates', label: t('Updates'), words: 'update version new release download install automatic preview stable channel skip check restart' },
    { id: 'about', label: t('About'), words: 'version license source code github gpl data folder' },
  ]

  /** The block a route points at; 'general' is the top of the page. */
  function blockOf(s: SettingsSection): BlockId | null {
    if (s === 'general' || s === 'appearance' || s === 'keyboard') return null
    if (s === 'privacy') return 'accounts'
    return s
  }

  /** The route value of a block. */
  function routeOf(id: BlockId): SettingsSection {
    return id
  }

  /* Search */
  let query = $state('')
  const shown = $derived.by(() => {
    const words = fold(query).split(/\s+/).filter(Boolean)
    if (!words.length) return BLOCKS
    return BLOCKS.filter((b) => {
      const text = fold(`${b.label} ${b.words}`)
      return words.every((w) => text.includes(w))
    })
  })
  const visible = $derived(new Set(shown.map((b) => b.id)))

  // While searching, the marker stays on a block that is shown.
  $effect(() => {
    const list = shown
    if (list.length && !list.some((b) => b.id === untrack(() => current))) current = list[0].id
  })

  /* Section list and scrolling */
  let root: HTMLElement | undefined = $state()
  let current = $state<BlockId>('language')
  /** Ignore the scroll spy right after a jump, so the block just picked stays marked. */
  let lockUntil = 0

  function scrollParent(el: HTMLElement): HTMLElement | null {
    for (let p = el.parentElement; p; p = p.parentElement) {
      const o = getComputedStyle(p).overflowY
      if (o === 'auto' || o === 'scroll') return p
    }
    return null
  }

  function blockEl(id: BlockId): HTMLElement | null {
    return document.getElementById(`${uid}-${id}`)
  }

  function jump(id: BlockId | null) {
    lockUntil = performance.now() + 250
    const el = id ? blockEl(id) : null
    const box = root ? scrollParent(root) : null
    if (!box) return
    // scroll the content area only: scrollIntoView would also move the window frame when the block is near the end
    const top = el ? box.scrollTop + el.getBoundingClientRect().top - box.getBoundingClientRect().top - 24 : 0
    box.scrollTo({ top: Math.max(0, top) })
    current = id ?? shown[0]?.id ?? 'language'
  }

  function go(e: MouseEvent, id: BlockId) {
    e.preventDefault()
    app.navigate({ name: 'settings', section: routeOf(id) })
    jump(id)
    document.getElementById(`${uid}-h-${id}`)?.focus({ preventScroll: true })
  }

  // A route change (section list, palette, Back) scrolls to its block.
  $effect(() => {
    const target = blockOf(section)
    untrack(() => void tick().then(() => jump(target)))
  })

  // The block at the top of the view is the current one.
  $effect(() => {
    if (!root) return
    const box = scrollParent(root)
    if (!box) return
    const spy = () => {
      if (performance.now() < lockUntil) return
      const list = untrack(() => shown)
      if (!list.length) return
      let pick = list[0].id
      if (box.scrollTop + box.clientHeight >= box.scrollHeight - 2) pick = list[list.length - 1].id
      else {
        const top = box.getBoundingClientRect().top
        for (const b of list) {
          const el = blockEl(b.id)
          if (el && el.getBoundingClientRect().top - top <= 96) pick = b.id
        }
      }
      current = pick
    }
    box.addEventListener('scroll', spy, { passive: true })
    return () => box.removeEventListener('scroll', spy)
  })

  /* Saving */
  const s = $derived(app.settings)
  const save = (patch: Partial<Settings>) => app.saveSettings(patch)
  async function copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path)
      app.toast(t('Path copied.'), 'success')
    } catch {
      app.toast(t('Couldn’t copy. Select the path and copy it by hand.'), 'error')
    }
  }

  const saveJava = (patch: Partial<Settings['java']>) => s && save({ java: { ...s.java, ...patch } })
  const saveGame = (patch: Partial<Settings['game']>) => s && save({ game: { ...s.game, ...patch } })
  const saveProxy = (patch: Partial<Settings['proxy']>) => s && save({ proxy: { ...s.proxy, ...patch } })


  /* Sliders save once the thumb rests, not on every step. */
  function debounced(run: (v: number) => void) {
    let timer: ReturnType<typeof setTimeout> | undefined
    let pending: number | null = null
    return {
      set(v: number) {
        pending = v
        clearTimeout(timer)
        timer = setTimeout(this.flush, 350)
      },
      flush() {
        clearTimeout(timer)
        if (pending != null) run(pending)
        pending = null
      },
    }
  }

  /* Java and memory */
  const totalMb = $derived(app.info?.totalMemoryMb ?? 0)
  const MEM_MIN = 1024
  const memMax = $derived(Math.max(4096, totalMb ? Math.floor(totalMb / 512) * 512 : 16384, s?.java.maxMemoryMb ?? 0))
  let memDraft = $state<number | null>(null)
  const memory = $derived(memDraft ?? s?.java.maxMemoryMb ?? 4096)
  const memSave = debounced((v) => {
    if (s) saveJava({ maxMemoryMb: v, minMemoryMb: Math.min(s.java.minMemoryMb, v) })
    memDraft = null
  })

  function gb(mb: number): string {
    return `${num(Math.round((mb / 1024) * 10) / 10)} GB`
  }

  const memHint = $derived(
    totalMb && memory > totalMb * 0.75
      ? t('That leaves little memory for {os} and other apps. Your computer has {n}.', { n: gb(totalMb), os: osName })
      : totalMb
        ? t('Used by instances that don’t set their own. Your PC has {total}.', { total: gb(totalMb) })
        : t('Used by instances that don’t set their own.'),
  )

  let jvmArgs = $state('')
  const savedJvm = $derived(s?.java.jvmArgs ?? '')
  $effect(() => {
    jvmArgs = savedJvm
  })

  let javas = $state<JavaInstall[] | null>(null)
  let javaBusy = $state(false)
  async function findJava() {
    javaBusy = true
    try {
      javas = (await api.listJava()).sort((a, b) => b.major - a.major)
    } catch (e) {
      javas ??= []
      app.error(e)
    } finally {
      javaBusy = false
    }
  }

  /* Game */
  let width = $state('')
  let height = $state('')
  const savedW = $derived(s?.game.width)
  const savedH = $derived(s?.game.height)
  $effect(() => {
    width = savedW != null ? String(savedW) : ''
  })
  $effect(() => {
    height = savedH != null ? String(savedH) : ''
  })

  function commitSize(which: 'width' | 'height') {
    if (!s) return
    const raw = (which === 'width' ? width : height).trim()
    const n = Math.round(Number(raw))
    const [min, max] = which === 'width' ? [320, 7680] : [240, 4320]
    const v = raw && Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : s.game[which]
    if (which === 'width') width = String(v)
    else height = String(v)
    if (v !== s.game[which]) saveGame({ [which]: v })
  }

  /* Language: each name in its own language, so anyone can find theirs. */
  const LANGUAGES = $derived([
    { value: 'system', label: t('{os} language', { os: osName }) },
    { value: 'en', label: 'English' },
    { value: 'fr', label: 'Français' },
  ])

  const ON_LAUNCH = [
    { value: 'minimize', label: t('Minimize Cubby') },
    { value: 'keep', label: t('Keep it open') },
    { value: 'close', label: t('Close Cubby') },
  ]
  const ON_LAUNCH_HINT: Record<Settings['onLaunch'], string> = {
    minimize: t('Cubby minimizes while you play and comes back when the game closes.'),
    keep: t('Cubby stays open next to the game.'),
    close: t('Cubby hides while you play and quits when the last game closes.'),
  }

  /* Downloads */
  let parDraft = $state<number | null>(null)
  const parallel = $derived(parDraft ?? s?.downloadConcurrency ?? 16)
  const parSave = debounced((v) => {
    save({ downloadConcurrency: v })
    parDraft = null
  })

  const PROXIES = [
    { value: 'system', label: t('Use {os} settings', { os: osName }) },
    { value: 'none', label: t('No proxy') },
    { value: 'manual', label: t('Manual') },
  ]
  let proxyUrl = $state('')
  const savedProxyUrl = $derived(s?.proxy.url ?? '')
  $effect(() => {
    proxyUrl = savedProxyUrl
  })
  const proxyError = $derived(
    proxyUrl.trim() && !/^(https?|socks5h?):\/\/\S+$/i.test(proxyUrl.trim()) ? t('Start the address with http://, https:// or socks5://.') : '',
  )
  function commitProxy() {
    const v = proxyUrl.trim()
    if (proxyError || !s || v === s.proxy.url) return
    saveProxy({ url: v })
  }

  /* Storage */
  const FOLDER_LABEL: Record<DataFolder['key'], string> = {
    instances: t('Instances'),
    libraries: t('Libraries'),
    assets: t('Game assets'),
    java: t('Java runtimes'),
    meta: t('Metadata cache'),
    logs: t('Launcher logs'),
    trash: t('Trash'),
  }
  let folders = $state<DataFolder[] | null>(null)
  const used = $derived(folders?.reduce((n, f) => n + f.sizeBytes, 0) ?? 0)
  const metaSize = $derived(folders?.find((f) => f.key === 'meta')?.sizeBytes ?? null)

  async function loadFolders() {
    try {
      folders = await api.dataFolders()
    } catch (e) {
      folders = []
      app.error(e)
    }
  }

  function openFolder(key: DataFolder['key']) {
    api.openDataFolder(key).catch((e) => app.error(e))
  }

  let cleanAsk = $state(false)
  let cleaning = $state(false)
  async function cleanCache() {
    cleaning = true
    try {
      await api.clearCache()
      cleanAsk = false
      app.toast(t('Cache cleaned up.'), 'success')
      await loadFolders()
    } catch (e) {
      app.error(e)
    } finally {
      cleaning = false
    }
  }

  /* Accounts */
  const msaSource = $derived(app.info?.msaSource ?? null)
  let clientId = $state('')
  const savedClientId = $derived(s?.msaClientId ?? '')
  $effect(() => {
    clientId = savedClientId
  })
  const clientIdError = $derived(
    clientId.trim() && !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(clientId.trim())
      ? t('An application id looks like 00000000-0000-0000-0000-000000000000.')
      : '',
  )
  async function commitClientId() {
    const v = clientId.trim()
    if (clientIdError || v === savedClientId) return
    await save({ msaClientId: v || null })
    // msaConfigured depends on it
    try {
      await app.refreshInfo()
    } catch (e) {
      app.error(e)
    }
  }

  let cfKey = $state('')
  const savedCfKey = $derived(s?.curseforgeApiKey ?? '')
  $effect(() => {
    cfKey = savedCfKey
  })
  const cfSource = $derived(app.info?.curseforgeKey ?? null)
  async function commitCfKey() {
    const v = cfKey.trim()
    if (v === savedCfKey) return
    await save({ curseforgeApiKey: v || null })
    // the key decides whether CurseForge shows up in New instance
    await app.refreshInfo()
    await app.loadProviders()
  }

  /* Updates of Cubby */
  const UPDATE_MODES = [
    { value: 'auto', label: t('Install it for me') },
    { value: 'ask', label: t('Ask me') },
    { value: 'off', label: t('Don’t check') },
  ]
  const UPDATE_MODE_HINT: Record<Settings['updates']['mode'], string> = {
    auto: t('Cubby downloads it in the background and installs it when you close the app. It never interrupts a game.'),
    ask: t('Cubby tells you when a new version is out and waits for your OK before downloading it.'),
    off: t('Cubby never looks by itself. You can still check by hand.'),
  }
  const CHANNELS = [
    { value: 'stable', label: t('Stable') },
    { value: 'preview', label: t('Preview') },
  ]
  const u = $derived(app.update)
  const updating = $derived(u.phase === 'available' || u.phase === 'downloading' || u.phase === 'ready' || u.phase === 'failed')
  const checkedText = $derived(
    u.checkedAt ? t('Last checked {time}.', { time: timeOfDay(new Date(u.checkedAt).toISOString()) }) : t('Not checked yet.'),
  )
  const saveUpdates = (patch: Partial<Settings['updates']>) => s && save({ updates: { ...s.updates, ...patch } })
  async function changeChannel(channel: Settings['updates']['channel']) {
    await saveUpdates({ channel })
    await app.checkForUpdate(true)
  }

  /* About */
  const SOURCE = 'https://github.com/benjaminbrtrd/Cubby'
  const LICENSE = 'https://www.gnu.org/licenses/gpl-3.0.html'
  function openLink(e: MouseEvent, url: string) {
    e.preventDefault()
    api.openUrl(url).catch((err) => app.error(err))
  }

  /* Loading */
  $effect(() => {
    untrack(() => {
      void findJava()
      void loadFolders()
    })
    return () => {
      memSave.flush()
      parSave.flush()
    }
  })
</script>

<!-- A row with its text on the left and a switch on the right; the title is the switch's label. -->
{#snippet switchRow(key: string, title: string, hint: string, checked: boolean, onChange: (v: boolean) => void)}
  <div class="row">
    <div class="row__text">
      <label class="text-body-strong" for="{uid}-{key}">{title}</label>
      <p class="text-small muted">{hint}</p>
    </div>
    <Switch id="{uid}-{key}" label={title} hideLabel {checked} {onChange} />
  </div>
{/snippet}

{#snippet heading(id: BlockId, label: string)}
  <h2 id="{uid}-h-{id}" class="block__title text-heading" tabindex="-1">{label}</h2>
{/snippet}

<div class="screen" bind:this={root}>
  <div class="layout">
    <div class="side">
      <div>
        <h1 class="text-title">{t('Settings')}</h1>
        <p class="text-small muted">{t('Changes save as you go.')}</p>
      </div>
      <TextField label={t('Search settings')} hideLabel icon="search" type="search" placeholder={t('Search settings')} autocomplete="off" bind:value={query} />
      <nav aria-label={t('Settings sections')}>
        <ul class="nav">
          {#each shown as b (b.id)}
            {@const on = current === b.id}
            <li>
              <a href="#{uid}-{b.id}" class={['nav__item', on && 'is-on']} aria-current={on ? 'location' : undefined} onclick={(e) => go(e, b.id)}>
                {#if on}<Icon name="chevron-right" size={18} />{/if}
                <span class={['nav__label', on ? 'text-body-strong' : 'text-body']}>{b.label}</span>
              </a>
            </li>
          {/each}
        </ul>
      </nav>
    </div>

    <div class="blocks">
      {#if !s}
        <p class="text-small muted">{t('Loading your settings…')}</p>
      {:else}
        {#if shown.length === 0}
          <div class="block empty">
            <p class="text-body">{t('No setting matches “{query}”.', { query: query.trim() })}</p>
            <div><Button size="sm" onclick={() => (query = '')}>{t('Clear search')}</Button></div>
          </div>
        {/if}

        {#if visible.has('language')}
          <section id="{uid}-language" class="block" aria-labelledby="{uid}-h-language">
            {@render heading('language', t('Language'))}
            <div class="row row--stack">
              <SegmentedControl label={t('Language')} options={LANGUAGES} value={s.language === 'en' || s.language === 'fr' ? s.language : 'system'} onChange={(v) => void save({ language: v }).then(() => switchLanguage(v))} />
            </div>
          </section>
        {/if}

        {#if visible.has('java')}
          <section id="{uid}-java" class="block" aria-labelledby="{uid}-h-java">
            {@render heading('java', t('Java and memory'))}
            {@render switchRow(
              'java-auto',
              t('Choose Java automatically'),
              s.java.auto
                ? t('Cubby picks the right Java for each version and downloads it when it’s missing.')
                : t('Cubby uses the Java found on this PC. A version with no matching Java won’t start.'),
              s.java.auto,
              (v) => saveJava({ auto: v }),
            )}
            <div class="row row--stack">
              <Slider
                label={t('Default memory')}
                min={MEM_MIN}
                max={memMax}
                step={512}
                value={memory}
                valueText={gb(memory)}
                marks={[gb(MEM_MIN), gb(memMax)]}
                hint={memHint}
                onChange={(v) => {
                  memDraft = v
                  memSave.set(v)
                }}
              />
            </div>
            <div class="row row--stack">
              <TextField
                label={t('Default JVM arguments')}
                mono
                placeholder={t('-XX:+UseZGC')}
                spellcheck={false}
                autocomplete="off"
                hint={t('Used by instances that don’t set their own. Leave it empty unless a mod asks for something.')}
                bind:value={jvmArgs}
                onchange={() => jvmArgs.trim() !== savedJvm && saveJava({ jvmArgs: jvmArgs.trim() })}
              />
            </div>
            <div class="row row--stack">
              <div class="row__head">
                <p class="text-body-strong grow">{t('Java on this PC')}</p>
                <Button size="sm" icon="search" busy={javaBusy} disabled={javaBusy} onclick={findJava}>{t('Search again')}</Button>
              </div>
              {#if javas === null}
                <p class="text-small muted">{t('Looking for Java on this PC…')}</p>
              {:else if javas.length === 0}
                <p class="text-small muted">{t('No Java found. Cubby downloads the right one the first time you play.')}</p>
              {:else}
                <ul class="list javas">
                  {#each javas as j (j.executable)}
                    <li class="java">
                      <span class="java__major text-counter" aria-hidden="true">{j.major}</span>
                      <div class="java__text">
                        <div class="java__title">
                          <span class="text-body-strong">Java {j.major}</span>
                          {#if j.vendor}<span class="text-body muted">{j.vendor}</span>{/if}
                          <MetaChip>{j.version}</MetaChip>
                          <MetaChip>{j.arch}</MetaChip>
                          {#if j.managed}<StatusTag state="ready">{t('Managed by Cubby')}</StatusTag>{/if}
                        </div>
                        <p class="text-mono text-small muted java__path" title={j.executable}>{j.executable}</p>
                      </div>
                      <IconButton icon="copy" size="sm" label={t('Copy the path of Java {major}', { major: j.major })} onclick={() => copyPath(j.executable)} />
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          </section>
        {/if}

        {#if visible.has('game')}
          <section id="{uid}-game" class="block" aria-labelledby="{uid}-h-game">
            {@render heading('game', t('Game'))}
            <div class="row">
              <div class="row__text">
                <p id="{uid}-size" class="text-body-strong">{t('Window size')}</p>
                <p class="text-small muted">{t('When the game isn’t fullscreen. Instances can set their own.')}</p>
              </div>
              <div class="size" role="group" aria-labelledby="{uid}-size">
                <div class="size__field">
                  <TextField label={t('Window width')} hideLabel mono suffix="px" inputmode="numeric" bind:value={width} onchange={() => commitSize('width')} />
                </div>
                <span class="text-body" aria-hidden="true">×</span>
                <div class="size__field">
                  <TextField label={t('Window height')} hideLabel mono suffix="px" inputmode="numeric" bind:value={height} onchange={() => commitSize('height')} />
                </div>
              </div>
            </div>
            {@render switchRow('fullscreen', t('Start in fullscreen'), t('For every instance.'), s.game.fullscreen, (v) => saveGame({ fullscreen: v }))}
            <div class="row row--stack">
              <p class="text-body-strong">{t('When the game starts')}</p>
              <SegmentedControl
                label={t('When the game starts')}
                options={ON_LAUNCH}
                value={s.onLaunch}
                onChange={(v) => save({ onLaunch: v as Settings['onLaunch'] })}
              />
              <p class="text-small muted">{ON_LAUNCH_HINT[s.onLaunch]}</p>
            </div>
            {@render switchRow('track', t('Count play time'), t('Adds each session to the instance’s play time.'), s.trackPlayTime, (v) => save({ trackPlayTime: v }))}
            {@render switchRow('show', t('Show play time'), t('In the library, with each instance.'), s.showPlayTime, (v) => save({ showPlayTime: v }))}
          </section>
        {/if}

        {#if visible.has('downloads')}
          <section id="{uid}-downloads" class="block" aria-labelledby="{uid}-h-downloads">
            {@render heading('downloads', t('Downloads'))}
            <div class="row row--stack">
              <Slider
                label={t('Parallel downloads')}
                min={1}
                max={32}
                step={1}
                value={parallel}
                valueText={t('{n} at a time', { n: num(parallel) })}
                hint={t('More is faster on a good connection. Lower it if downloads stall.')}
                onChange={(v) => {
                  parDraft = v
                  parSave.set(v)
                }}
              />
            </div>
            {@render switchRow(
              'mod-updates',
              t('Check mod updates when I open an instance'),
              t('Looks for new versions on Modrinth. Cubby always asks before updating.'),
              s.checkUpdates,
              (v) => save({ checkUpdates: v }),
            )}
            <div class="row row--stack">
              <p class="text-body-strong">{t('Proxy')}</p>
              <SegmentedControl
                label={t('Proxy')}
                options={PROXIES}
                value={s.proxy.mode}
                onChange={(v) => saveProxy({ mode: v as Settings['proxy']['mode'] })}
              />
              {#if s.proxy.mode === 'manual'}
                <TextField
                  label={t('Proxy address')}
                  mono
                  placeholder="http://proxy.example:8080"
                  spellcheck={false}
                  autocomplete="off"
                  error={proxyError}
                  bind:value={proxyUrl}
                  onchange={commitProxy}
                />
              {/if}
              <p class="text-small muted">{t('Cubby applies this the next time it starts.')}</p>
            </div>
          </section>
        {/if}

        {#if visible.has('storage')}
          <section id="{uid}-storage" class="block" aria-labelledby="{uid}-h-storage">
            {@render heading('storage', t('Storage'))}
            <div class="row row--stack">
              <div>
                <p class="text-body-strong">{t('Data folder')}</p>
                <p class="text-small muted">{t('Instances, Java and shared game files.')}{folders ? t(' {n} used.', { n: bytes(used) }) : ''}</p>
              </div>
              {#if app.info}<span class="well text-mono">{app.info.dataDir}</span>{/if}
            </div>
            <div class="row row--stack">
              {#if folders === null}
                <p class="text-small muted">{t('Measuring folders…')}</p>
              {:else if folders.length === 0}
                <p class="text-small muted">{t('No folder to show.')}</p>
              {:else}
                <ul class="list">
                  {#each folders as f (f.key)}
                    <li class="item">
                      <Icon name="folder" size={18} />
                      <div class="item__text">
                        <p class="text-body-strong">{FOLDER_LABEL[f.key] ?? f.label}</p>
                        <p class="text-mono muted path">{f.path}</p>
                      </div>
                      <span class="text-counter size-text">{bytes(f.sizeBytes)}</span>
                      <Button size="sm" icon="folder" onclick={() => openFolder(f.key)}>{t('Open')}</Button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
            <div class="row">
              <div class="row__text">
                <p class="text-body-strong">{t('Clean up the cache')}</p>
                <p class="text-small muted">
                  {metaSize != null ? t('Deletes saved version lists and loader profiles ({size}). Cubby downloads them again when it needs them.', { size: bytes(metaSize) }) : t('Deletes saved version lists and loader profiles. Cubby downloads them again when it needs them.')}
                </p>
              </div>
              <Button size="sm" icon="trash" onclick={() => (cleanAsk = true)}>{t('Clean up')}</Button>
            </div>
          </section>
        {/if}

        {#if visible.has('accounts')}
          <section id="{uid}-accounts" class="block" aria-labelledby="{uid}-h-accounts">
            {@render heading('accounts', t('Accounts'))}
            <div class="row row--stack">
              <div class="row__head">
                <p class="text-body-strong">{t('Microsoft sign-in')}</p>
                {#if app.info?.msaConfigured}
                  <StatusTag state="ready">{t('Ready')}</StatusTag>
                {:else}
                  <StatusTag state="offline">{t('Not available')}</StatusTag>
                {/if}
              </div>
              <p class="text-small muted">
                {#if msaSource === 'builtIn'}
                  {t('Included in Cubby. Players sign in with their Microsoft account, there is nothing to set up.')}
                {:else if msaSource === 'settings'}
                  {t('Uses your own application id, below, instead of the one included in Cubby.')}
                {:else if msaSource === 'environment'}
                  {t('Uses the application id from the')} <span class="text-mono">CUBBY_MSA_CLIENT_ID</span> variable.
                {:else}
                  {t('This copy of Cubby was built without a Microsoft application id, so it only starts the Minecraft demo. Official builds include one, or add your own below.')}
                {/if}
              </p>
              <details class="advanced" open={msaSource === 'settings' || !app.info?.msaConfigured}>
                <summary class="text-body-strong">{t('Use your own Azure application')}</summary>
                <div class="advanced__body">
                  <TextField
                    label={t('Application id')}
                    mono
                    placeholder="00000000-0000-0000-0000-000000000000"
                    spellcheck={false}
                    autocomplete="off"
                    error={clientIdError}
                    bind:value={clientId}
                    onchange={commitClientId}
                  />
                  <p class="text-small muted">
                    {t('For developers. The application must accept personal Microsoft accounts, allow public client flows, and be approved by Mojang for Minecraft. Leave it empty to use the one included in Cubby.')}
                  </p>
                </div>
              </details>
            </div>
            <div class="row row--stack">
              <div class="row__head">
                <label class="text-body-strong" for="{uid}-cf">{t('CurseForge API key')}</label>
                {#if cfSource}
                  <StatusTag state="ready">{t('Ready')}</StatusTag>
                {:else}
                  <StatusTag state="offline">{t('Not set')}</StatusTag>
                {/if}
              </div>
              <p class="text-small muted">
                {#if cfSource === 'builtIn'}
                  {t('Included in Cubby. CurseForge modpacks work, there is nothing to set up. A key of your own, below, replaces it.')}
                {:else if cfSource === 'settings'}
                  {t('Uses your own key, below, instead of any other.')}
                {:else if cfSource === 'environment'}
                  {t('Uses the key from the')} <span class="text-mono">CUBBY_CURSEFORGE_API_KEY</span> {t('variable. A key of your own, below, replaces it.')}
                {:else}
                  {t('CurseForge modpacks need a key. Get a free one from CurseForge for developers and paste it below.')}
                {/if}
              </p>
              <TextField
                id="{uid}-cf"
                type="password"
                mono
                placeholder={t('Paste your key')}
                spellcheck={false}
                autocomplete="off"
                bind:value={cfKey}
                onchange={commitCfKey}
                hint={t('Saved on this PC. Leave it empty to use the key included in Cubby, if there is one.')}
              />
            </div>
          </section>
        {/if}

        {#if visible.has('updates')}
          <section id="{uid}-updates" class="block" aria-labelledby="{uid}-h-updates">
            {@render heading('updates', t('Updates'))}
            <div class="row">
              <div class="row__text">
                <p class="text-body-strong">Cubby {#if app.info}<span class="text-mono">{app.info.version}</span>{/if}</p>
                <p class="text-small muted">
                  {#if u.phase === 'checking'}{t('Looking for a new version…')}
                  {:else if updating && u.info}{t(u.phase === 'ready' ? '{version} is ready to install.' : u.phase === 'downloading' ? '{version} is downloading.' : '{version} is available.', { version: u.info.version })} {checkedText}
                  {:else if u.error}{u.error}
                  {:else}{checkedText}{/if}
                </p>
              </div>
              {#if u.phase === 'checking'}
                <Button size="sm" busy disabled>{t('Checking…')}</Button>
              {:else if u.phase === 'available'}
                <StatusTag state="update">{t('Update available')}</StatusTag>
                <Button size="sm" variant="primary" icon="download" onclick={() => app.downloadUpdate()}>{t('Download update')}</Button>
              {:else if u.phase === 'downloading'}
                <StatusTag state="download">{t('Downloading')}</StatusTag>
              {:else if u.phase === 'ready'}
                <StatusTag state="ready">{t('Ready')}</StatusTag>
                <Button size="sm" variant="primary" icon="update" onclick={() => (app.updateDialog = 'restart')}>{t('Restart and update')}</Button>
              {:else if u.phase === 'failed'}
                <StatusTag state="crashed">{t('Failed')}</StatusTag>
                <Button size="sm" icon="update" onclick={() => app.downloadUpdate()}>{t('Try again')}</Button>
              {:else}
                {#if u.error}<StatusTag state="offline">{t('Not checked')}</StatusTag>
                {:else if s.updates.mode === 'off'}<StatusTag state="disabled">{t('Off')}</StatusTag>
                {:else if u.checkedAt}<StatusTag state="ready">{t('Up to date')}</StatusTag>{/if}
                <Button size="sm" icon="update" onclick={() => app.checkForUpdate(true)}>{u.error ? t('Try again') : t('Check for updates')}</Button>
              {/if}
            </div>
            <div class="row row--stack">
              <p class="text-body-strong">{t('When an update is available')}</p>
              <SegmentedControl
                label={t('When an update is available')}
                options={UPDATE_MODES}
                value={s.updates.mode}
                onChange={(v) => saveUpdates({ mode: v as Settings['updates']['mode'] })}
              />
              <p class="text-small muted">{UPDATE_MODE_HINT[s.updates.mode]}</p>
            </div>
            <div class="row row--stack">
              <p class="text-body-strong">{t('Version channel')}</p>
              <SegmentedControl label={t('Version channel')} options={CHANNELS} value={s.updates.channel} onChange={(v) => changeChannel(v as Settings['updates']['channel'])} />
              <p class="text-small muted">{t('Preview gets new versions earlier, with rough edges. Stable only gets finished releases.')}</p>
            </div>
            {#if s.updates.skipped}
              <div class="row">
                <div class="row__text">
                  <p class="text-body-strong">{t('Skipped version')}</p>
                  <p class="text-small muted">{t('You chose to skip {skipped}. Cubby stays quiet until a newer one comes out.', { skipped: s.updates.skipped })}</p>
                </div>
                <Button size="sm" onclick={async () => { await saveUpdates({ skipped: null }); await app.checkForUpdate() }}>{t('Stop skipping')}</Button>
              </div>
            {/if}
          </section>
        {/if}

        {#if visible.has('about')}
          <section id="{uid}-about" class="block" aria-labelledby="{uid}-h-about">
            {@render heading('about', t('About'))}
            <div class="about">
              <Logo size={32} />
              <p class="text-body-strong">Cubby {#if app.info}<span class="text-mono">{app.info.version}</span>{/if}</p>
              <p class="text-small muted">{t('A free Minecraft launcher.')}</p>
              {#if app.info}
                <div>
                  <p class="text-label muted label">{t('Data folder')}</p>
                  <span class="well text-mono">{app.info.dataDir}</span>
                </div>
              {/if}
              <p class="links text-small">
                <a class="link" href={SOURCE} onclick={(e) => openLink(e, SOURCE)}>{t('Source code')}</a>
                <a class="link" href={LICENSE} onclick={(e) => openLink(e, LICENSE)}>{t('License')} <span class="text-mono">GPL-3.0</span></a>
              </p>
              <p class="text-small muted">
                {t('Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.')}
                <a class="link" href="mailto:contact@cubbylauncher.com">contact@cubbylauncher.com</a>
              </p>
            </div>
          </section>
        {/if}
      {/if}
    </div>
  </div>
</div>

{#if cleanAsk}
  {#snippet cleanActions()}
    <Button variant="ghost" onclick={() => (cleanAsk = false)}>{t('Cancel')}</Button>
    <Button variant="danger" icon="trash" busy={cleaning} disabled={cleaning} onclick={cleanCache}>{t('Clean up')}</Button>
  {/snippet}
  <Dialog tone="danger" icon="trash" title={t('Clean up the cache?')} onClose={() => (cleanAsk = false)} actions={cleanActions}>
    <p>{t('Cubby deletes its saved version lists and loader profiles, then downloads them again the next time it needs them.')}</p>
    <p>{t('Your instances, worlds, Java and accounts stay as they are.')}</p>
  </Dialog>
{/if}

<style>
  .screen {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    min-height: 100%;
    padding: var(--space-6);
  }
  .layout {
    display: flex;
    align-items: flex-start;
    gap: var(--space-6);
  }
  .screen :where(h1, h2, p) {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
  }

  /* Section list */
  .side {
    position: sticky;
    top: var(--space-6);
    flex: 0 0 240px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .nav__item {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 30px;
    padding: 3px 10px 3px 36px;
    color: var(--ink-muted);
    text-decoration: none;
    border: var(--border-block) solid transparent;
  }
  .nav__item:hover {
    color: var(--ink);
    background: var(--surface);
  }
  .nav__item:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  /* The current section: an ink frame and a chevron */
  .nav__item.is-on {
    padding-inline-start: 8px;
    color: var(--ink);
    background: var(--surface-hover);
    border-color: var(--ink);
  }
  .nav__label {
    flex: 1;
    min-width: 0;
  }

  /* Blocks */
  .blocks {
    flex: 1 1 0;
    min-width: 0;
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }
  .block {
    box-sizing: border-box;
    padding: 14px 18px;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
    scroll-margin-top: var(--space-6);
  }
  .block__title {
    margin-bottom: var(--space-2);
    outline: none;
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) 0;
    border-top: var(--border-hair) solid var(--line);
  }
  .row--stack {
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-2);
  }
  .row--stack > :global(.cb-seg) {
    align-self: flex-start;
  }
  .row__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row__text label {
    cursor: pointer;
  }
  .row__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
  }
  .size {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .size__field {
    width: 112px;
  }

  .list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    background: var(--bg);
    border: var(--border-block) solid var(--edge);
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-top: var(--border-hair) solid var(--line);
  }
  .item:first-child {
    border-top: 0;
  }
  .item__text {
    flex: 1;
    min-width: 0;
  }
  .path {
    overflow-wrap: anywhere;
  }
  .java {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3);
    border-top: var(--border-hair) solid var(--line);
  }
  .java:first-child {
    border-top: 0;
  }
  .java__major {
    flex: none;
    display: grid;
    place-items: center;
    box-sizing: border-box;
    width: 44px;
    height: 44px;
    color: var(--ink);
    background: var(--field);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .java__text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .java__title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }
  .java__path {
    margin: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .size-text {
    flex: none;
    color: var(--ink-muted);
  }
  .well {
    display: block;
    padding: 6px 10px;
    overflow-wrap: anywhere;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }

  .about {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    padding-top: 10px;
    border-top: var(--border-hair) solid var(--line);
  }
  .about > div {
    align-self: stretch;
  }
  .label {
    margin-bottom: 6px;
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-4);
  }
  .link {
    color: var(--link);
  }
  .link:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  /* Advanced: a closed disclosure unless the player needs it */
  .advanced {
    border-top: var(--border-hair) solid var(--line);
    padding-top: var(--space-3);
  }
  .advanced summary {
    width: fit-content;
    color: var(--ink-muted);
    cursor: pointer;
  }
  .advanced summary:hover,
  .advanced[open] summary {
    color: var(--ink);
  }
  .advanced summary:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .advanced__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }
</style>
