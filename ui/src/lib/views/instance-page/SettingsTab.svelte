<script lang="ts" module>
  import type { GameVersion } from '../../types'

  /** Minecraft versions, fetched once per app run. */
  let versionCache: GameVersion[] | null = null

  /** The Java a release of Minecraft needs (null for snapshots and very old versions we can't read). */
  function javaNeeded(mc: string): number | null {
    const m = /^1\.(\d+)(?:\.(\d+))?$/.exec(mc)
    if (!m) return null
    const minor = Number(m[1])
    const patch = Number(m[2] ?? 0)
    if (minor > 20 || (minor === 20 && patch >= 5)) return 21
    if (minor >= 18) return 17
    if (minor === 17) return 16
    return 8
  }
</script>

<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import { api } from '../../api'
  import {
    Button,
    Dialog,
    Icon,
    IconPicker,
    InstanceIcon,
    MetaChip,
    ProgressBar,
    SegmentedControl,
    Slider,
    Switch,
    TextField,
    rovingKeys,
    type Ground,
    type Motif,
  } from '../../cubby'
  import { loaderFull, loaderName, megabytes, num } from '../../format'
  import { encodeIcon, iconOf } from '../../icon'
  import { app } from '../../state.svelte'
  import type { Instance, JavaInstall, JavaOverrides, LoaderKind, LoaderVersion } from '../../types'
  import { gigabytes, type InstanceData } from './InstancePage.svelte'

  let { instance, data }: { instance: Instance; data: InstanceData } = $props()

  const id = $derived(instance.id)
  const running = $derived(!!app.running[id])
  const task = $derived(app.instanceTask(id))
  const repairTask = $derived(app.tasks[`repair:${id}`])

  /** Saves a change on top of the latest saved instance. */
  async function save(patch: Partial<Instance>) {
    const current = app.instance(id)
    if (!current) return
    return app.updateInstance({ ...($state.snapshot(current) as Instance), ...patch })
  }

  // General
  let name = $state(untrack(() => instance.name))
  let nameError = $state('')
  let group = $state(untrack(() => instance.group ?? ''))

  function commitName() {
    const next = name.trim()
    if (!next) {
      nameError = 'Give the instance a name.'
      return
    }
    nameError = ''
    if (next !== instance.name) void save({ name: next })
  }

  function commitGroup() {
    const next = group.trim() || null
    if (next !== instance.group) void save({ group: next })
  }

  // Icon
  let iconOpen = $state(false)
  let motif = $state<Motif>('block')
  let ground = $state<Ground>('moss')
  let src = $state<string | null>(null)
  let fileInput: HTMLInputElement | undefined = $state()

  function openIcon() {
    const now = iconOf(instance)
    if (now.src !== undefined) src = now.src
    else {
      motif = now.motif
      ground = now.ground
      src = null
    }
    iconOpen = true
  }

  /** Keep a 256 px square for sharp previews and high-DPI screens. */
  async function importImage(e: Event & { currentTarget: HTMLInputElement }) {
    const file = e.currentTarget.files?.[0]
    e.currentTarget.value = ''
    if (!file) return
    const url = URL.createObjectURL(file)
    try {
      const img = new Image()
      img.src = url
      await img.decode()
      const side = Math.min(img.naturalWidth, img.naturalHeight)
      const canvas = document.createElement('canvas')
      canvas.width = 256
      canvas.height = 256
      const ctx = canvas.getContext('2d')
      if (!ctx || !side) throw new Error('This image can’t be read.')
      ctx.imageSmoothingEnabled = true
      ctx.imageSmoothingQuality = 'high'
      ctx.drawImage(img, (img.naturalWidth - side) / 2, (img.naturalHeight - side) / 2, side, side, 0, 0, 256, 256)
      src = canvas.toDataURL('image/png')
    } catch {
      app.toast('Cubby can’t read this image. Try a PNG or JPG file.', 'error')
    } finally {
      URL.revokeObjectURL(url)
    }
  }

  async function saveIcon() {
    await save({ icon: src ?? encodeIcon(motif, ground) })
    iconOpen = false
  }

  // Java and memory
  const o = $derived(instance.javaOverrides)
  const g = $derived(app.settings?.java ?? { auto: true, minMemoryMb: 1024, maxMemoryMb: 4096, jvmArgs: '' })
  const usesGlobal = $derived(o.executable == null && o.minMemoryMb == null && o.maxMemoryMb == null && o.jvmArgs == null)
  const total = $derived(app.info?.totalMemoryMb ?? 8192)
  const memMax = $derived(Math.max(2048, Math.floor(total / 512) * 512))
  const memMarks = $derived(['1 GB', gigabytes(Math.round((1024 + memMax) / 2 / 512) * 512), gigabytes(memMax)])

  function overrides(patch: Partial<JavaOverrides>) {
    return save({ javaOverrides: { ...$state.snapshot(o), ...patch } })
  }

  function setGlobal(on: boolean) {
    if (on) void save({ javaOverrides: { executable: null, minMemoryMb: null, maxMemoryMb: null, jvmArgs: null } })
    else void overrides({ minMemoryMb: g.minMemoryMb, maxMemoryMb: g.maxMemoryMb })
  }

  let memDraft = $state<number | null>(null)
  const mem = $derived(memDraft ?? o.maxMemoryMb ?? g.maxMemoryMb)
  let memTimer: ReturnType<typeof setTimeout> | undefined

  function setMemory(v: number) {
    memDraft = v
    clearTimeout(memTimer)
    memTimer = setTimeout(saveMemory, 400)
  }

  async function saveMemory() {
    clearTimeout(memTimer)
    memTimer = undefined
    const v = memDraft
    if (v == null) return
    await overrides({ maxMemoryMb: v, minMemoryMb: Math.min(o.minMemoryMb ?? g.minMemoryMb, v) })
    if (memDraft === v) memDraft = null
  }

  onDestroy(() => {
    if (memTimer) void saveMemory()
  })

  let jvm = $state(untrack(() => instance.javaOverrides.jvmArgs ?? ''))
  function commitJvm() {
    const next = jvm.trim() || null
    if (next !== o.jvmArgs) void overrides({ jvmArgs: next })
  }

  // Java installs found on this PC
  let javas = $state<JavaInstall[] | null>(null)
  api
    .listJava()
    .then((j) => (javas = j))
    .catch(() => (javas = []))
  const needed = $derived(javaNeeded(instance.minecraft))
  const chosenJava = $derived(o.executable ? javas?.find((j) => j.executable === o.executable) : undefined)

  let javaOpen = $state(false)
  let javaPick = $state<string | null>(null)
  const javaOptions = $derived<(JavaInstall | null)[]>([null, ...(javas ?? [])])
  const javaIndex = $derived(Math.max(0, javaOptions.findIndex((j) => (j?.executable ?? null) === javaPick)))

  function openJava() {
    javaPick = o.executable
    javaOpen = true
  }

  async function saveJava() {
    await overrides({ executable: javaPick })
    javaOpen = false
  }

  // Version and loader
  type Channel = 'release' | 'snapshot' | 'old'
  const channelOf = (v: GameVersion): Channel => (v.kind === 'release' ? 'release' : v.kind === 'snapshot' ? 'snapshot' : 'old')
  /** What Cubby can install; an instance imported with Forge or NeoForge keeps it listed. */
  const SUPPORTED: LoaderKind[] = ['vanilla', 'fabric', 'quilt']
  const LOADERS = $derived(SUPPORTED.includes(instance.loader.kind) ? SUPPORTED : [...SUPPORTED, instance.loader.kind])

  let versionOpen = $state(false)
  let versions = $state<GameVersion[] | null>(versionCache)
  let channel = $state<Channel>('release')
  let target = $state('')
  let kind = $state<LoaderKind>('vanilla')
  let loaderVersions = $state<LoaderVersion[] | null>(null)
  let loaderVersion = $state('')
  let applying = $state(false)
  let token = 0

  async function openVersion() {
    target = instance.minecraft
    kind = instance.loader.kind
    loaderVersion = instance.loader.version ?? ''
    versionOpen = true
    if (!versions) {
      try {
        versions = versionCache = await api.listVersions()
      } catch (e) {
        versions = []
        app.error(e)
      }
    }
    const cur = versions.find((v) => v.id === instance.minecraft)
    channel = cur ? channelOf(cur) : 'release'
  }

  const shownVersions = $derived((versions ?? []).filter((v) => channelOf(v) === channel))
  const versionIndex = $derived(Math.max(0, shownVersions.findIndex((v) => v.id === target)))

  // Loader versions for the chosen game version
  $effect(() => {
    if (!versionOpen) return
    const k = kind
    const game = target
    if (k === 'vanilla' || !SUPPORTED.includes(k) || !game) {
      loaderVersions = []
      return
    }
    const mine = ++token
    loaderVersions = null
    api
      .listLoaderVersions(k, game)
      .then((list) => {
        if (mine !== token) return
        const sorted = [...list.filter((v) => v.stable), ...list.filter((v) => !v.stable)]
        loaderVersions = sorted
        const keep = untrack(() => loaderVersion)
        loaderVersion = sorted.some((v) => v.version === keep) ? keep : (sorted[0]?.version ?? '')
      })
      .catch(() => {
        if (mine === token) loaderVersions = []
      })
  })

  const loaderIndex = $derived(Math.max(0, (loaderVersions ?? []).findIndex((v) => v.version === loaderVersion)))
  const unsupported = $derived(!SUPPORTED.includes(kind))
  const loaderMissing = $derived(kind !== 'vanilla' && !unsupported && loaderVersions !== null && loaderVersions.length === 0)
  const changed = $derived(
    target !== instance.minecraft || kind !== instance.loader.kind || (kind !== 'vanilla' && loaderVersion !== (instance.loader.version ?? '')),
  )
  const canApply = $derived(
    !applying && !running && !!target && changed && !unsupported && (kind === 'vanilla' || (loaderVersions !== null && !!loaderVersion)),
  )

  async function applyVersion() {
    if (!canApply) return
    applying = true
    const loader = kind === 'vanilla' ? { kind } : { kind, version: loaderVersion }
    const saved = await save({ minecraft: target, loader })
    applying = false
    if (saved) {
      versionOpen = false
      app.toast(`${saved.name} now uses Minecraft ${saved.minecraft} with ${loaderFull(saved.loader)}. Cubby downloads the files the next time you play.`, 'success')
    }
  }

  function backUpFirst() {
    versionOpen = false
    app.openInstance(id, 'worlds')
  }

  const game = $derived(app.settings?.game)
</script>

<div class="ip-body" role="tabpanel" id="instance-panel" aria-label="Settings">
  <div class="cols">
    <div class="col">
      <section class="block" aria-labelledby="is-general">
        <div class="block__head"><h2 id="is-general" class="text-heading">General</h2></div>
        <div class="setting">
          <div>
            <div class="text-body-strong">Name</div>
            <div class="text-small muted">Shown in your library.</div>
          </div>
          <TextField label="Name" hideLabel bind:value={name} error={nameError || undefined} onchange={commitName} autocomplete="off" />
        </div>
        <div class="setting">
          <div>
            <div class="text-body-strong">Group</div>
            <div class="text-small muted">The library tab it appears in. Leave empty for none.</div>
          </div>
          <TextField label="Group" hideLabel icon="folder" bind:value={group} onchange={commitGroup} list="instance-groups" autocomplete="off" placeholder="No group" />
          <datalist id="instance-groups">
            {#each app.groups as gr (gr.name)}<option value={gr.name}></option>{/each}
          </datalist>
        </div>
        <div class="setting">
          <div>
            <div class="text-body-strong">Icon</div>
            <div class="text-small muted">Pick a picture or import one.</div>
          </div>
          <div class="inline">
            <InstanceIcon {...iconOf(instance)} size={32} raised />
            <Button size="sm" icon="captures" onclick={openIcon}>Change icon</Button>
          </div>
        </div>
      </section>

      <section class="block" aria-labelledby="is-java">
        <div class="block__head">
          <h2 id="is-java" class="text-heading grow">Java and memory</h2>
          <Switch checked={usesGlobal} onChange={setGlobal} label="Use global settings" />
        </div>
        {#if usesGlobal}
          <div class="setting setting--wide">
            <div>
              <div class="text-body">Memory {megabytes(g.maxMemoryMb)} · Java picked automatically{g.jvmArgs.trim() ? ' · global JVM arguments' : ''}</div>
              <div class="text-small muted">The same as every instance that uses the global settings.</div>
            </div>
            <Button size="sm" variant="ghost" icon="settings" onclick={() => app.navigate({ name: 'settings', section: 'java' })}>Global settings</Button>
          </div>
        {:else}
          <div class="ruled">
            <Slider
              label="Memory"
              min={1024}
              max={memMax}
              step={512}
              value={Math.min(mem, memMax)}
              valueText={megabytes(mem)}
              marks={memMarks}
              hint={`Your PC has ${gigabytes(total)}. Leave at least a quarter of it for Windows and other apps.`}
              onChange={setMemory}
            />
            {#if mem > total * 0.75}
              <p class="text-small warn flag"><Icon name="warning" size={18} label="Warning" /><span>That leaves little memory for Windows. The game may stutter or close.</span></p>
            {/if}
          </div>
          <div class="setting setting--wide">
            <div class="min0">
              <div class="text-body-strong">Java</div>
              {#if o.executable}
                <div class="text-body">{chosenJava ? `Java ${chosenJava.version}${chosenJava.vendor ? ` · ${chosenJava.vendor}` : ''}` : 'Your own Java'}</div>
                <div class="text-mono muted wrap">{o.executable}</div>
              {:else}
                <div class="text-body">Picked automatically{needed ? ` (Java ${needed})` : ''}</div>
                <div class="text-small muted">Cubby uses the Java this version needs and downloads it when it’s missing.</div>
              {/if}
            </div>
            <Button size="sm" icon="folder" onclick={openJava}>Choose Java</Button>
          </div>
          <div class="setting">
            <div>
              <div class="text-body-strong">JVM arguments</div>
              <div class="text-small muted">Replace the global ones. For experts: leave empty if unsure.</div>
            </div>
            <TextField label="JVM arguments" hideLabel mono bind:value={jvm} onchange={commitJvm} placeholder={g.jvmArgs.trim() || '-XX:+UseG1GC'} autocomplete="off" spellcheck={false} />
          </div>
        {/if}
      </section>

      <section class="block" aria-labelledby="is-version">
        <div class="block__head"><h2 id="is-version" class="text-heading">Version and loader</h2></div>
        <div class="setting setting--wide">
          <div>
            <div class="inline">
              <MetaChip>{instance.minecraft}</MetaChip>
              <MetaChip>{loaderFull(instance.loader)}</MetaChip>
            </div>
            <div class="text-small muted gap">The game version and the loader that runs your mods.</div>
          </div>
          <Button size="sm" icon="cube" disabled={running} onclick={openVersion}>Change version</Button>
        </div>
        <p class="text-small warn flag"><Icon name="warning" size={18} label="Warning" /><span>Changing version can break mods and worlds. Back up first.</span></p>
        {#if running}<p class="text-small muted">Close the game to change the version.</p>{/if}
      </section>
    </div>

    <div class="col">
      <section class="block" aria-labelledby="is-window">
        <div class="block__head"><h2 id="is-window" class="text-heading">Game window</h2></div>
        <div class="setting setting--wide">
          <div>
            <div class="text-body">{game ? `${num(game.width)} × ${num(game.height)} · ${game.fullscreen ? 'fullscreen' : 'windowed'}` : 'Global window size'}</div>
            <div class="text-small muted">Window size and fullscreen are the same for every instance.</div>
          </div>
          <Button size="sm" variant="ghost" icon="settings" onclick={() => app.navigate({ name: 'settings', section: 'game' })}>Game settings</Button>
        </div>
      </section>

      <section class="block" aria-labelledby="is-maint">
        <div class="block__head"><h2 id="is-maint" class="text-heading">Maintenance</h2></div>
        <div class="setting setting--wide">
          <div>
            <div class="text-body-strong">Repair</div>
            <div class="text-small muted">Downloads missing or broken game files again. Keeps worlds, mods and settings.</div>
          </div>
          <Button size="sm" icon="wrench" busy={!!repairTask} disabled={running || !!task} onclick={() => data.repair()}>Repair</Button>
        </div>
        {#if repairTask}
          <ProgressBar tone="diamond" label={repairTask.label} value={repairTask.total ? (repairTask.done / repairTask.total) * 100 : undefined} />
        {/if}
        <div class="setting setting--wide">
          <div>
            <div class="text-body-strong">Duplicate</div>
            <div class="text-small muted">Copies the whole instance, worlds included, as “{instance.name} copy”.</div>
          </div>
          <Button size="sm" icon="copy" busy={data.duplicating} disabled={data.duplicating} onclick={() => data.duplicate(instance)}>Duplicate</Button>
        </div>
        <div class="setting setting--wide">
          <div>
            <div class="text-body-strong">Folder</div>
            <div class="text-small muted">Where the game keeps this instance’s files.</div>
          </div>
          <Button size="sm" icon="folder" onclick={() => data.openFolder()}>Open folder</Button>
        </div>
        <div class="setting setting--wide danger">
          <div>
            <div class="text-body-strong danger__title">Delete instance</div>
            <div class="text-small muted">Moves it to the trash, with its worlds, mods and settings.</div>
          </div>
          <Button size="sm" variant="danger" icon="trash" disabled={running} onclick={() => (app.deletingId = id)}>Delete instance</Button>
        </div>
        {#if running}<p class="text-small muted">Close the game to delete this instance.</p>{/if}
      </section>
    </div>
  </div>
</div>

<input class="cb-visually-hidden" type="file" accept="image/png,image/jpeg,image/gif,image/webp" tabindex="-1" aria-hidden="true" bind:this={fileInput} onchange={importImage} />

{#if iconOpen}
  {#snippet iconActions()}
    <Button variant="ghost" onclick={() => (iconOpen = false)}>Cancel</Button>
    <Button variant="primary" icon="check" onclick={saveIcon}>Use this icon</Button>
  {/snippet}
  <Dialog wide title="Change icon" onClose={() => (iconOpen = false)} actions={iconActions}>
    <IconPicker bind:motif bind:ground bind:src name={instance.name} onImport={() => fileInput?.click()} />
  </Dialog>
{/if}

{#if javaOpen}
  {#snippet javaActions()}
    <Button variant="ghost" onclick={() => (javaOpen = false)}>Cancel</Button>
    <Button variant="primary" icon="check" onclick={saveJava}>Use this Java</Button>
  {/snippet}
  <Dialog wide title="Choose Java" onClose={() => (javaOpen = false)} actions={javaActions}>
    {#if needed}<p>Minecraft {instance.minecraft} needs Java {needed} or newer.</p>{/if}
    {#if javas === null}
      <p class="muted">Looking for Java on this PC…</p>
    {:else}
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div
        class="choices"
        role="radiogroup"
        aria-label="Java"
        onkeydown={(e) => rovingKeys(e, javaOptions.length, javaIndex, (i) => (javaPick = javaOptions[i]?.executable ?? null))}
      >
        {#each javaOptions as j, i (j?.executable ?? 'auto')}
          {@const on = i === javaIndex}
          {@const old = !!j && !!needed && j.major < needed}
          <button type="button" role="radio" aria-checked={on} tabindex={on ? 0 : -1} class={['choice', on && 'is-on']} onclick={() => (javaPick = j?.executable ?? null)}>
            <span class="choice__mark" aria-hidden="true">{#if on}<Icon name="check" size={18} />{/if}</span>
            <span class="choice__text">
              {#if j}
                <span class="text-body-strong">Java {j.version}{j.vendor ? ` · ${j.vendor}` : ''}{j.managed ? ' · downloaded by Cubby' : ''}</span>
                <span class="text-mono muted wrap">{j.executable}</span>
                {#if old}<span class="text-small warn flag"><Icon name="warning" size={18} />Too old for {instance.minecraft}</span>{/if}
              {:else}
                <span class="text-body-strong">Automatic (recommended)</span>
                <span class="text-small muted">Cubby picks the right Java for each version and downloads it when it’s missing.</span>
              {/if}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </Dialog>
{/if}

{#if versionOpen}
  {#snippet versionActions()}
    <Button variant="ghost" onclick={() => (versionOpen = false)}>Cancel</Button>
    <Button variant="primary" icon="check" busy={applying} disabled={!canApply} onclick={applyVersion}>Apply</Button>
  {/snippet}
  <Dialog wide title="Change version" onClose={() => (versionOpen = false)} actions={versionActions}>
    <div class="warnbox">
      <span class="warn icon"><Icon name="warning" size={18} label="Warning" /></span>
      <div class="min0">
        <p class="text-body-strong">Changing version can break mods and worlds.</p>
        <p class="text-small muted">A world opened in a newer version can’t go back. Mods made for another version may not load.</p>
      </div>
      <Button size="sm" icon="copy" onclick={backUpFirst}>Back up worlds</Button>
    </div>
    <div class="vgrid">
      <div class="vcol">
        <p class="text-body-strong">Minecraft version</p>
        <SegmentedControl
          label="Kind of version"
          options={[
            { value: 'release', label: 'Releases' },
            { value: 'snapshot', label: 'Snapshots' },
            { value: 'old', label: 'Old' },
          ]}
          value={channel}
          onChange={(v) => (channel = v as Channel)}
        />
        {#if versions === null}
          <p class="muted text-small">Loading versions…</p>
        {:else if !shownVersions.length}
          <p class="muted text-small">No versions of this kind.</p>
        {:else}
          <!-- svelte-ignore a11y_interactive_supports_focus -->
          <div
            class="choices choices--scroll"
            role="radiogroup"
            aria-label="Minecraft version"
            onkeydown={(e) => rovingKeys(e, shownVersions.length, versionIndex, (i) => (target = shownVersions[i].id))}
          >
            {#each shownVersions as v, i (v.id)}
              {@const on = i === versionIndex && v.id === target}
              <button type="button" role="radio" aria-checked={on} tabindex={i === versionIndex ? 0 : -1} class={['choice choice--row', on && 'is-on']} onclick={() => (target = v.id)}>
                <span class="choice__mark" aria-hidden="true">{#if on}<Icon name="check" size={18} />{/if}</span>
                <span class="text-mono grow">{v.id}</span>
                {#if v.id === instance.minecraft}<span class="text-small muted">Now</span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <div class="vcol">
        <p class="text-body-strong">Loader</p>
        <SegmentedControl label="Loader" options={LOADERS.map((k) => ({ value: k, label: loaderName(k) }))} value={kind} onChange={(v) => (kind = v as LoaderKind)} />
        {#if unsupported}
          <p class="text-small warn flag"><Icon name="warning" size={18} label="Warning" /><span>Cubby can’t install {loaderName(kind)} yet, so it can’t change its version. Pick Vanilla, Fabric or Quilt to change the version.</span></p>
        {:else}
          <p class="text-small muted">Cubby can’t install NeoForge or Forge yet.</p>
        {/if}
        {#if unsupported}
          <!-- nothing to pick -->
        {:else if kind === 'vanilla'}
          <p class="text-body">Vanilla doesn’t load mods. Your mod files stay in the folder.</p>
        {:else if loaderVersions === null}
          <p class="muted text-small">Loading {loaderName(kind)} versions…</p>
        {:else if loaderMissing}
          <p class="text-small warn flag"><Icon name="warning" size={18} label="Warning" /><span>{loaderName(kind)} isn’t out for Minecraft {target} yet.</span></p>
        {:else}
          <!-- svelte-ignore a11y_interactive_supports_focus -->
          <div
            class="choices choices--scroll"
            role="radiogroup"
            aria-label={`${loaderName(kind)} version`}
            onkeydown={(e) => rovingKeys(e, loaderVersions?.length ?? 0, loaderIndex, (i) => (loaderVersion = loaderVersions?.[i]?.version ?? ''))}
          >
            {#each loaderVersions as v, i (v.version)}
              {@const on = v.version === loaderVersion}
              <button type="button" role="radio" aria-checked={on} tabindex={i === loaderIndex ? 0 : -1} class={['choice choice--row', on && 'is-on']} onclick={() => (loaderVersion = v.version)}>
                <span class="choice__mark" aria-hidden="true">{#if on}<Icon name="check" size={18} />{/if}</span>
                <span class="text-mono grow">{v.version}</span>
                {#if !v.stable}<span class="text-small muted">Beta</span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
    {#if changed && target}
      <p class="summary text-body">
        {instance.name} will use Minecraft <span class="text-mono">{target}</span>{kind === 'vanilla' ? ' without a loader' : loaderVersion ? ` with ${loaderName(kind)} ` : ''}{#if kind !== 'vanilla' && loaderVersion}<span class="text-mono">{loaderVersion}</span>{/if}. Cubby downloads the files the next time you play.
      </p>
    {/if}
  </Dialog>
{/if}

<style>
  p {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .warn {
    color: var(--gold-text);
  }
  .icon {
    display: inline-flex;
    flex: none;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .min0 {
    min-width: 0;
  }
  .wrap {
    overflow-wrap: anywhere;
  }
  .gap {
    margin-top: var(--space-1);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: var(--space-4);
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: var(--space-3) 14px;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .block h2 {
    margin: 0;
  }
  .block__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    min-height: 28px;
  }
  .setting {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 232px;
    align-items: center;
    gap: var(--space-1) var(--space-4);
    padding-top: 6px;
    border-top: var(--border-hair) solid var(--line);
  }
  .setting--wide {
    grid-template-columns: minmax(0, 1fr) auto;
  }
  .ruled {
    padding-top: 6px;
    border-top: var(--border-hair) solid var(--line);
  }
  .inline {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .flag {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .danger {
    margin-top: 6px;
    padding: 10px var(--space-3);
    background: var(--bg);
    border: var(--border-block) solid var(--redstone);
  }
  .danger__title {
    color: var(--redstone-text);
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }
  .choices--scroll {
    gap: 0;
    max-height: 280px;
    overflow-y: auto;
    margin-top: 0;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    min-height: var(--size-control);
    padding: var(--space-2) var(--space-3);
    color: var(--ink);
    text-align: start;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    cursor: pointer;
  }
  .choice:hover {
    background: var(--surface-hover);
  }
  .choice.is-on {
    border-color: var(--ink);
    background: var(--surface-hover);
  }
  .choice:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .choice--row {
    align-items: center;
    min-height: 32px;
    padding: 4px var(--space-3);
    background: transparent;
    border-width: 0 0 var(--border-hair);
    border-color: var(--line);
  }
  .choice--row.is-on {
    border-color: var(--line);
    box-shadow: inset 0 0 0 2px var(--ink);
  }
  .choice--row:focus-visible {
    box-shadow: inset 0 0 0 3px var(--focus);
  }
  .choice__mark {
    display: inline-flex;
    flex: none;
    width: 18px;
    color: var(--grass-text);
  }
  .choice__text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .warnbox {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
    padding: var(--space-3);
    background: var(--bg);
    border: var(--border-block) solid var(--gold);
  }
  .warnbox .min0 {
    flex: 1 1 260px;
  }
  .vgrid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: var(--space-5);
  }
  .vcol {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .summary {
    margin-top: var(--space-4);
  }
</style>
