<!-- The Install aside of a project page, also shown in a dialog from a Discover card.
     Mods, shader packs and resource packs go into the target instance after a check (api.planInstall);
     a modpack always becomes a new instance. -->
<script lang="ts">
  import { api } from '../../api'
  import { Banner, Button, Icon, InstanceIcon, Kbd, PlayButton, ProgressBar, Switch, TextField } from '../../cubby'
  import type { IconName } from '../../cubby'
  import { bytes, date, loaderName, plural, relativeTime } from '../../format'
  import { encodeIcon, projectArt } from '../../icon'
  import { app } from '../../state.svelte'
  import type { InstallPlan, Instance, ProjectHit, ProjectVersion } from '../../types'
  import Picker from './Picker.svelte'
  import TargetPicker from './TargetPicker.svelte'
  import {
    CHANNEL,
    KINDS,
    acceptedLoaders,
    bestVersion,
    fits,
    listWords,
    loaderLabel,
    mcRange,
    modLoadersOf,
    newestMc,
    orderVersions,
    primarySize,
    serverHint,
    shaderMods,
    sourceBySlug,
    type Fit,
  } from './discover'

  interface Props {
    project: ProjectHit
    /** Every version of the project, newest first. Leave it out and the panel loads them; null = the page is loading them. */
    versions?: ProjectVersion[] | null
    /** Why the page couldn't load the versions. */
    versionsError?: string | null
    /** Bindable: the version to install. Null = the best one for the target. */
    versionId?: string | null
    /** In a dialog: no title and no "When you install" steps. */
    compact?: boolean
    /** Called when one of the panel's links leaves the current screen (to close a dialog). */
    onLeave?: () => void
    /** Called once an install into an instance has started. */
    onInstall?: (instanceId: string) => void
  }

  let { project, versions: given, versionsError = null, versionId = $bindable(null), compact = false, onLeave, onInstall }: Props = $props()

  const kind = $derived(project.kind)
  const words = $derived(KINDS[kind])
  const isPack = $derived(kind === 'modpack')

  function message(e: unknown): string {
    return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e)
  }

  // versions
  let loaded = $state<ProjectVersion[] | null>(null)
  let loadError = $state<string | null>(null)
  let reloads = $state(0)

  $effect(() => {
    if (given !== undefined) return
    const id = project.id
    void reloads
    let stale = false
    loadError = null
    api
      .listProjectVersions(id, null, null)
      .then((v) => {
        if (!stale) loaded = v
      })
      .catch((e) => {
        if (!stale) loadError = message(e)
      })
    return () => {
      stale = true
    }
  })

  const versions = $derived(given !== undefined ? given : loaded)
  const versionsFailed = $derived(given !== undefined ? versionsError : loadError)

  const target = $derived(isPack ? undefined : app.target)
  const ordered = $derived(versions ? orderVersions(versions, kind, target) : { fit: [], other: [] })
  const best = $derived(versions ? bestVersion(versions, kind, target) : undefined)
  const chosen = $derived(versions?.find((v) => v.id === versionId) ?? best ?? versions?.[0])
  const chosenFits = $derived(chosen ? fits(chosen, kind, target) : false)
  const newestRelease = $derived(versions?.find((v) => v.channel === 'release'))

  const targetId = $derived(target?.id)

  // a new instance gets its own best version
  let lastTarget: string | undefined = undefined
  $effect(() => {
    const id = targetId
    if (id !== lastTarget) {
      if (lastTarget !== undefined) versionId = null
      lastTarget = id
    }
  })

  $effect(() => {
    if (targetId) void app.loadSources(targetId)
  })

  const versionItems = $derived([
    ...ordered.fit.map((v) => ({ id: v.id, v, group: ordered.other.length && target ? `For ${target.name}` : undefined })),
    ...ordered.other.map((v) => ({ id: v.id, v, group: 'Other versions' })),
  ])

  // what's in the instance
  const sources = $derived(target ? app.sources[target.id] : undefined)
  const installed = $derived(target ? app.installedIn(target.id, project.id) : undefined)
  const versionIndex = (id: string | undefined) => (id && versions ? versions.findIndex((v) => v.id === id) : -1)

  // plan
  let plan = $state<InstallPlan | null>(null)
  let planError = $state<string | null>(null)

  $effect(() => {
    // the id, not the instance: the instance list is replaced after every task
    const t = targetId
    const v = chosen?.id
    if (isPack || !t || !v) return
    void sources // plan again once the instance's content changes
    let stale = false
    planError = null
    api
      .planInstall(t, v)
      .then((p) => {
        if (!stale) plan = p
      })
      .catch((e) => {
        if (!stale) {
          plan = null
          planError = message(e)
        }
      })
    return () => {
      stale = true
    }
  })

  /** The plan, only when it is for what is shown now. */
  const current = $derived(plan && target && chosen && plan.instanceId === target.id && plan.items[0]?.versionId === chosen.id ? plan : null)
  const deps = $derived(current ? current.items.filter((i) => i.reason === 'dependency') : [])

  interface Check {
    tone: Fit['tone']
    icon: IconName
    title: string
    mono?: string
    word: string
    detail?: string
    action?: { label: string; run: () => void }
  }

  function openMod(slug: string) {
    app.openProject(slug, 'mod')
    onLeave?.()
  }

  const checks = $derived.by((): Check[] => {
    if (!target || !chosen) return []
    const out: Check[] = []
    const range = mcRange(chosen.gameVersions)
    if (chosen.gameVersions.includes(target.minecraft)) {
      out.push({ tone: 'ok', icon: 'check', title: 'Minecraft', mono: target.minecraft, word: 'Supported', detail: `This version runs on ${range}.` })
    } else {
      out.push({
        tone: kind === 'mod' ? 'bad' : 'warn',
        icon: kind === 'mod' ? 'error' : 'warning',
        title: 'Minecraft',
        mono: target.minecraft,
        word: 'Not listed',
        detail:
          kind === 'resourcepack'
            ? `This version is made for ${range}. It may still load: the game only warns about it.`
            : kind === 'shader'
              ? `This version is made for ${range}. It may still work with Iris.`
              : `This version is made for ${range}. Pick another version or another instance.`,
      })
    }

    if (kind === 'mod') {
      const names = modLoadersOf(chosen.loaders).map(loaderLabel)
      if (target.loader.kind === 'vanilla') {
        out.push({ tone: 'bad', icon: 'error', title: 'Mod loader', word: 'Missing', detail: `${target.name} is Vanilla. Mods need Fabric, Quilt, Forge or NeoForge: create an instance with one of them.` })
      } else if (chosen.loaders.some((l) => acceptedLoaders(target.loader.kind).includes(l))) {
        const others = names.filter((n) => n !== loaderName(target.loader))
        out.push({ tone: 'ok', icon: 'check', title: loaderName(target.loader), word: 'Supported', detail: others.length ? `Also made for ${listWords(others)}.` : undefined })
      } else {
        out.push({ tone: 'bad', icon: 'error', title: loaderName(target.loader), word: 'Not supported', detail: names.length ? `This version is for ${listWords(names, 'or')}.` : undefined })
      }
    }

    if (kind === 'shader') {
      if (target.loader.kind === 'vanilla') {
        out.push({ tone: 'warn', icon: 'warning', title: 'Iris Shaders', word: 'Needs a loader', detail: `Shader packs run through Iris, a mod, and ${target.name} is Vanilla. Create an instance with Fabric or NeoForge, then add Iris.` })
      } else {
        for (const need of shaderMods(target)) {
          const have = sourceBySlug(sources, need.slug)
          if (have) out.push({ tone: 'ok', icon: 'check', title: need.title, mono: have.versionNumber, word: 'Installed', detail: need.why })
          else out.push({ tone: 'warn', icon: 'warning', title: need.title, word: 'Not installed', detail: `${need.why} Install it too, or the pack won't show up.`, action: { label: `Open ${need.title}`, run: () => openMod(need.slug) } })
        }
      }
      const runs = chosen.loaders
      if (runs.length && !runs.includes('iris') && runs.includes('optifine'))
        out.push({ tone: 'warn', icon: 'warning', title: 'Made for OptiFine', word: 'Check', detail: 'Iris runs most OptiFine shader packs, but not every one of them.' })
    }

    for (const d of deps)
      out.push(
        d.installed
          ? { tone: 'ok', icon: 'check', title: d.title, word: 'Already installed', detail: 'Required' }
          : { tone: 'busy', icon: 'download', title: d.title, mono: d.versionNumber, word: 'Will be installed', detail: `Required · ${bytes(d.size)}` },
      )
    if (current) {
      for (const m of current.missing)
        out.push({
          tone: 'bad',
          icon: 'error',
          title: m.title ?? 'A required mod',
          word: 'Missing',
          detail: `It's required, and it has no version for Minecraft ${target.minecraft} with ${loaderName(target.loader)}.`,
        })
      for (const c of current.conflicts)
        out.push({ tone: 'warn', icon: 'warning', title: c.title ?? c.fileName, word: 'Incompatible', detail: `It's in ${target.name} as ${c.fileName}. Turn it off or remove it in Mods first.` })
      if (!current.conflicts.length && kind === 'mod') out.push({ tone: 'ok', icon: 'check', title: 'No known conflicts', word: 'OK', detail: `Checked against what's in ${target.name}.` })
    }
    return out
  })

  const verdict = $derived.by((): Fit | null => {
    if (!target || !chosen) return null
    if (!current && !planError) return { tone: 'info', icon: 'info', word: `Checking with ${target.name}…` }
    if (checks.some((c) => c.tone === 'bad')) return { tone: 'bad', icon: 'error', word: `Can't go into ${target.name} as it is` }
    if (checks.some((c) => c.tone === 'warn')) return { tone: 'warn', icon: 'warning', word: 'Check before installing' }
    return { tone: 'ok', icon: 'check', word: `Works with ${target.name}` }
  })

  const toDownload = $derived(deps.filter((d) => !d.installed))
  const haveDeps = $derived(deps.filter((d) => d.installed))

  // install (mods, shader packs, resource packs)
  interface Job {
    instanceId: string
    versionId: string
    started: number
    /** Shader and resource packs: the file to turn on once it's there. */
    turnOn: string | null
  }

  let job = $state<Job | null>(null)
  let starting = $state(false)
  /** Shader packs and resource packs: select it in the instance after installing. */
  let turnOn = $state(true)
  let turnedOn = $state(false)
  const jobTask = $derived(job ? app.taskFor(`install:${job.instanceId}`) : undefined)
  const jobDone = $derived(job ? app.finished.find((f) => f.id === `install:${job!.instanceId}` && f.at >= job!.started) : undefined)
  const jobRunning = $derived(!!job && !jobDone && job.instanceId === target?.id)

  const handled = new Set<number>()
  $effect(() => {
    const j = job
    const done = jobDone
    if (!j || !done || done.error || handled.has(j.started)) return
    handled.add(j.started)
    void app.loadSources(j.instanceId, true)
    if (j.turnOn) void switchOn(j.instanceId, j.turnOn)
  })

  async function switchOn(instanceId: string, fileName: string) {
    try {
      if (kind === 'shader') await api.setShaderPack(instanceId, fileName)
      else if (kind === 'resourcepack') {
        // list_packs gives the active packs highest priority first: keep their order, below the new one
        const packs = await api.listPacks(instanceId, 'resource')
        const others = packs.filter((p) => p.enabled && p.fileName !== fileName).map((p) => p.fileName)
        await api.setResourcePacks(instanceId, [fileName, ...others])
      }
      turnedOn = true
    } catch (e) {
      app.error(e)
    }
  }

  const justInstalled = $derived(!!job && !!jobDone && !jobDone.error && job.instanceId === target?.id && job.versionId === chosen?.id)
  const isCurrent = $derived(justInstalled || (!!installed && !!chosen && installed.versionId === chosen.id))
  const otherVersion = $derived(!!installed && !!chosen && installed.versionId !== chosen.id && !justInstalled)
  const isUpdate = $derived(otherVersion && versionIndex(installed?.versionId) > versionIndex(chosen?.id))
  const failed = $derived(job && jobDone?.error && job.instanceId === target?.id ? jobDone.error : null)

  const busyElsewhere = $derived(target && !jobRunning ? app.instanceTask(target.id) : undefined)
  const gameRunning = $derived(!!target && !!app.running[target.id])

  const blocked = $derived.by((): string | null => {
    if (!target || !chosen) return null
    if (kind === 'mod' && gameRunning) return `Stop ${target.name} first: mods change when the game starts.`
    if (busyElsewhere) return `Wait for “${busyElsewhere.label}” to finish.`
    if (current?.missing.length) return 'A required mod is missing for this instance. Try another version.'
    if (kind === 'mod' && !chosenFits) return `Pick a version made for ${target.minecraft} and ${loaderName(target.loader)}, or another instance.`
    return null
  })
  const canInstall = $derived(!!target && !!chosen && !!current && !blocked && !starting)

  async function install() {
    if (!target || !chosen || !canInstall) return
    const instanceId = target.id
    const versionIdNow = chosen.id
    starting = true
    turnedOn = false
    const file = current?.items[0]?.fileName ?? null
    job = { instanceId, versionId: versionIdNow, started: Date.now(), turnOn: kind !== 'mod' && turnOn ? file : null }
    onInstall?.(instanceId)
    try {
      await api.installVersion(instanceId, versionIdNow)
    } catch (e) {
      job = null
      app.error(e)
    } finally {
      starting = false
    }
  }

  const playState = $derived(target ? (app.running[target.id] ? 'running' : app.tasks[`launch:${target.id}`] ? 'installing' : 'ready') : 'ready')

  function play() {
    if (!target) return
    if (app.running[target.id]) void app.kill(target.id)
    else void app.launch(target.id)
  }

  function openTab(i: Instance) {
    app.openInstance(i.id, words.tab ?? 'overview')
    onLeave?.()
  }

  /** "Played 2h ago", "Never played" */
  function played(iso: string | null): string {
    if (!iso) return 'Never played'
    const when = relativeTime(iso)
    return `Played ${when.charAt(0).toLowerCase()}${when.slice(1)}`
  }

  function pct(t: { done: number; total: number } | undefined): number | undefined {
    return t && t.total > 0 ? Math.round((t.done / t.total) * 100) : undefined
  }

  // modpacks
  const art = $derived(projectArt(project.slug))
  let name = $state('')
  let group = $state('Modpacks')
  let nameTouched = $state(false)
  $effect(() => {
    if (!nameTouched) name = project.title
  })
  const nameError = $derived(isPack && nameTouched && !name.trim() ? 'Give the instance a name.' : undefined)
  const alreadyHave = $derived(isPack ? app.instances.filter((i) => i.modpack?.projectId === project.id) : [])
  const packMc = $derived(chosen ? newestMc(chosen.gameVersions) : null)
  const packLoaders = $derived(chosen ? modLoadersOf(chosen.loaders).map(loaderLabel) : [])

  let created = $state<{ instance: Instance; started: number } | null>(null)
  let creating = $state(false)
  const packTask = $derived(created ? app.taskFor(`modpack:${created.instance.id}`) : undefined)
  const packDone = $derived(created ? app.finished.find((f) => f.id === `modpack:${created!.instance.id}` && f.at >= created!.started) : undefined)
  const packInstance = $derived(created ? (app.instance(created.instance.id) ?? created.instance) : undefined)

  async function installPack() {
    if (!chosen || creating) return
    nameTouched = true
    const n = name.trim()
    if (!n) return
    creating = true
    try {
      const started = Date.now()
      const icon = project.iconUrl ?? encodeIcon(art.motif, art.ground)
      const inst = await api.installModpack(chosen.id, n, group.trim() || null, icon)
      created = { instance: inst, started }
      void app.refreshInstances()
    } catch (e) {
      app.error(e)
    } finally {
      creating = false
    }
  }

  function openCreated() {
    if (!created) return
    app.openInstance(created.instance.id)
    onLeave?.()
  }

  function goDownloads() {
    app.navigate({ name: 'downloads' })
    onLeave?.()
  }
</script>

{#snippet versionOption(item: { v: ProjectVersion }, _on: boolean)}
  <span class="vopt">
    <span class="text-mono">{item.v.versionNumber}</span>
    <span class="text-small muted">
      {CHANNEL[item.v.channel].word} · <span class="text-mono">{mcRange(item.v.gameVersions)}</span> · {relativeTime(item.v.published, '')}
    </span>
    {#if best && item.v.id === best.id && target}
      <span class="text-small good"><Icon name="check" size={18} />Best for {target.name}</span>
    {/if}
  </span>
{/snippet}

{#snippet versionPicker()}
  <div class="field">
    <span class="text-body-strong">Version</span>
    {#if versions && versions.length && chosen}
      <Picker items={versionItems} value={chosen.id} onSelect={(id) => (versionId = id)} label="Version" buttonLabel="Version {chosen.versionNumber}. Change version" option={versionOption}>
        {#snippet trigger()}
          <span class="stack">
            <span class="text-mono">{chosen!.versionNumber}</span>
            <span class="text-small muted">
              {CHANNEL[chosen!.channel].word}{chosen!.id === newestRelease?.id ? ' · latest release' : ''} · {bytes(primarySize(chosen!))}
            </span>
          </span>
        {/snippet}
      </Picker>
      <span class="text-small muted">Released {relativeTime(chosen.published, '')} · {date(chosen.published)}</span>
      {#if chosen.channel !== 'release'}
        <span class="text-small warn line"><Icon name="warning" size={18} />{CHANNEL[chosen.channel].hint}.</span>
      {/if}
    {:else if versionsFailed}
      <p class="text-small bad line"><Icon name="error" size={18} />Couldn't load the versions. {versionsFailed}</p>
      {#if given === undefined}<Button size="sm" icon="update" onclick={() => reloads++}>Try again</Button>{/if}
    {:else if versions}
      <p class="text-small muted">There's no version to install yet.</p>
    {:else}
      <span class="placeholder" aria-hidden="true"></span>
      <span class="cb-visually-hidden">Loading versions</span>
    {/if}
  </div>
{/snippet}

<section class={['panel', compact && 'compact']} aria-label="Install {project.title}">
  {#if !compact}
    <div class="head">
      <h2 class="text-heading">Install</h2>
    </div>
  {/if}

  {#if isPack}
    <!-- modpack -->
    <div class="note-block">
      <p class="text-body-strong line"><Icon name="library" size={18} />Installs as a new instance</p>
      <p class="text-small muted">Modpacks always get their own instance, with their own Minecraft version, loader and settings. Nothing in your other instances changes.</p>
    </div>

    {#if alreadyHave.length && !created}
      <div class="have">
        <p class="text-small">
          You already have it in {listWords(alreadyHave.map((i) => i.name))}. Installing again makes another instance.
        </p>
        <Button size="sm" iconEnd="chevron-right" onclick={() => (app.openInstance(alreadyHave[0].id), onLeave?.())}>Open {alreadyHave[0].name}</Button>
      </div>
    {/if}

    {#if !created}
      <div class="named">
        <InstanceIcon {...project.iconUrl ? { src: project.iconUrl } : art} size={44} raised />
        <TextField
          label="Instance name"
          bind:value={name}
          oninput={() => (nameTouched = true)}
          hint={nameError ? undefined : 'Shown in your Library'}
          error={nameError}
          maxlength={64}
        />
      </div>
      <TextField label="Group" bind:value={group} hint="Leave empty for no group" maxlength={40} />

      {@render versionPicker()}

      {#if chosen}
        <section class="block" aria-labelledby="{project.id}-req">
          <h3 id="{project.id}-req" class="text-label muted">Comes with the pack</h3>
          <dl class="req">
            <dt class="text-small muted">Minecraft</dt>
            <dd class="text-mono">{packMc ?? '—'}</dd>
            <dt class="text-small muted">Loader</dt>
            <dd class="text-mono">{packLoaders.length ? packLoaders.join(', ') : 'Vanilla'}</dd>
            <dt class="text-small muted">Pack file</dt>
            <dd class="text-mono">{bytes(primarySize(chosen))}</dd>
          </dl>
          <p class="text-small muted">Cubby installs this Minecraft version and loader with the pack. The mods it lists download during the install.</p>
        </section>
      {/if}

      <Button variant="primary" size="lg" icon="download" busy={creating} disabled={!chosen || creating} onclick={installPack} class="wide">Install modpack</Button>
      <p class="text-small muted">You can keep using Cubby while it installs.</p>

      {#if !compact}
        <section class="steps" aria-labelledby="{project.id}-steps">
          <h3 id="{project.id}-steps" class="text-label muted">When you install</h3>
          <ol>
            <li><span class="num text-counter" aria-hidden="true">1</span><span class="text-body">Cubby adds {name.trim() || project.title} to your Library{group.trim() ? `, in ${group.trim()}` : ''}.</span></li>
            <li>
              <span class="num text-counter" aria-hidden="true">2</span>
              <span class="text-body">It downloads Minecraft{packMc ? ` ${packMc}` : ''}{packLoaders[0] ? `, ${packLoaders[0]}` : ''} and every mod of the pack. Follow it in <button type="button" class="link" onclick={goDownloads}>Downloads</button>.</span>
            </li>
            <li><span class="num text-counter" aria-hidden="true">3</span><span class="text-body">Then press Play. Your other instances stay as they are.</span></li>
          </ol>
        </section>
      {/if}
    {:else}
      {#if packDone?.error}
        <Banner tone="danger" title="Couldn't install {created.instance.name}">{packDone.error}</Banner>
        <Button onclick={openCreated} iconEnd="chevron-right">Open {created.instance.name}</Button>
      {:else if packDone}
        <Banner tone="success" title="{created.instance.name} is ready">It's in your Library{created.instance.group ? `, in ${created.instance.group}` : ''}.</Banner>
        <PlayButton
          size="md"
          class="wide"
          state={app.running[created.instance.id] ? 'running' : app.tasks[`launch:${created.instance.id}`] ? 'installing' : 'ready'}
          detail={packInstance ? played(packInstance.lastPlayed) : undefined}
          onclick={() => (app.running[created!.instance.id] ? app.kill(created!.instance.id) : app.launch(created!.instance.id))}
        />
        <Button onclick={openCreated} iconEnd="chevron-right">Open {created.instance.name}</Button>
      {:else}
        <div class="progress" aria-live="polite">
          <ProgressBar tone="diamond" label={packTask?.label ?? 'Getting ready'} value={pct(packTask)} />
          <p class="text-small muted">{created.instance.name} is in your Library. You can keep browsing while it installs.</p>
        </div>
        <div class="row">
          <Button onclick={openCreated} iconEnd="chevron-right">Open {created.instance.name}</Button>
          <Button variant="ghost" onclick={goDownloads}>Downloads</Button>
        </div>
      {/if}
    {/if}
  {:else if !target}
    <!-- no instance -->
    <div class="note-block">
      <p class="text-body-strong">Create an instance first</p>
      <p class="text-small muted">An instance is one Minecraft setup with its own version, loader, {words.many} and worlds. {words.label} go into one of them.</p>
    </div>
    <Button variant="primary" icon="plus" onclick={() => (app.navigate({ name: 'create' }), onLeave?.())}>New instance</Button>
  {:else}
    <!-- mod, shader pack, resource pack -->
    <div class="field">
      <span class="text-body-strong">Install to</span>
      <TargetPicker note={versions ? (i) => (versions!.some((v) => fits(v, kind, i)) ? null : { tone: 'warn', icon: 'warning', word: 'No version for it' }) : undefined} />
    </div>

    {@render versionPicker()}

    {#if chosen}
      <section class="block" aria-labelledby="{project.id}-check" aria-busy={!current && !planError}>
        {#if verdict}
          <h3 id="{project.id}-check" class={['text-body-strong', 'line', verdict.tone]}><Icon name={verdict.icon} size={18} />{verdict.word}</h3>
        {/if}
        {#if planError}
          <p class="text-small bad line"><Icon name="error" size={18} />Couldn't check it: {planError}</p>
        {/if}
        <ul class="checks">
          {#each checks as c, i (i)}
            <li>
              <span class={['ic', c.tone]}><Icon name={c.icon} size={18} /></span>
              <span class="text-body-strong">{c.title}{#if c.mono}{' '}<span class="text-mono">{c.mono}</span>{/if}</span>
              <span class={['text-small', c.tone]}>{c.word}</span>
              {#if c.detail}<span class="text-small muted detail">{c.detail}</span>{/if}
              {#if c.action}<span class="detail"><Button size="sm" iconEnd="chevron-right" onclick={c.action.run}>{c.action.label}</Button></span>{/if}
            </li>
          {/each}
          {#if !current && !planError}
            <li class="loading" aria-hidden="true"><span class="placeholder"></span></li>
          {/if}
        </ul>
      </section>

      {#if current}
        <p class="text-small muted">
          {#if current.downloadBytes > 0}
            <span class="text-body-strong ink">{bytes(current.downloadBytes)} to download</span>{toDownload.length ? `, with ${plural(toDownload.length, 'required mod', 'required mods')}.` : '.'}
          {:else}
            <span class="text-body-strong ink">Nothing to download.</span>
          {/if}
          {#if haveDeps.length && !toDownload.length}
            {listWords(haveDeps.map((d) => d.title))} {haveDeps.length === 1 ? 'is' : 'are'} already in {target.name}.
          {/if}
        </p>
      {/if}
    {/if}

    <!-- the action -->
    {#if jobRunning}
      <div class="progress" aria-live="polite">
        <ProgressBar tone="diamond" label={jobTask?.label ?? 'Getting ready'} value={pct(jobTask)} />
        <p class="text-small muted">Installing to {target.name}. You can keep browsing.</p>
      </div>
    {:else if isCurrent && chosen}
      <Banner tone="success" title="Installed in {target.name}">
        {#if kind === 'mod'}{project.title} {chosen.versionNumber} loads the next time you play.
        {:else if kind === 'shader'}{turnedOn ? "It's turned on for the next time you play." : 'Pick it in the Shaders tab, or in game.'}
        {:else}{turnedOn ? "It's on top of your packs the next time you play." : 'Turn it on in the Resource packs tab, or in game.'}{/if}
      </Banner>
      <PlayButton size="md" class="wide" state={playState} label={playState === 'ready' ? `Play ${target.name}` : undefined} detail={played(target.lastPlayed)} onclick={play} />
      {#if words.tab}<Button onclick={() => openTab(target)} iconEnd="chevron-right">Manage {words.many} in {target.name}</Button>{/if}
    {:else}
      {#if failed}
        <Banner tone="danger" title="Couldn't install {project.title}">{failed}</Banner>
      {/if}
      {#if otherVersion && installed}
        <p class="text-small muted line"><Icon name="update" size={18} />{target.name} has <span class="text-mono">{installed.versionNumber}</span>. Cubby replaces it.</p>
      {/if}
      {#if kind === 'shader' || kind === 'resourcepack'}
        <Switch
          checked={turnOn}
          onChange={(v) => (turnOn = v)}
          label="Turn it on after installing"
          hint={kind === 'shader' ? 'Used the next time you play' : 'On top of your other packs, so its textures win'}
        />
      {/if}
      <Button variant="primary" size="lg" icon={otherVersion ? 'update' : 'download'} busy={starting} disabled={!canInstall} onclick={install} class="wide">
        {#if otherVersion && chosen}{isUpdate ? 'Update' : 'Switch'} to {chosen.versionNumber}{:else}Install to {target.name}{/if}
      </Button>
      {#if blocked}<p class="text-small muted line"><Icon name="info" size={18} />{blocked}</p>{/if}
    {/if}

    {#if kind === 'mod'}
      {@const hint = serverHint(project.clientSide, project.serverSide)}
      <ul class="tips">
        {#if hint}<li class="text-small"><Icon name="servers" size={18} /><span>{hint}</span></li>{/if}
        <li class="text-small">
          <Icon name="info" size={18} />
          <span>Changed your mind? Turn it off or remove it in <button type="button" class="link" onclick={() => openTab(target)}>Mods</button>. Your worlds stay, but what it added disappears from them.</span>
        </li>
      </ul>
    {/if}

    {#if !compact}
      <section class="steps" aria-labelledby="{project.id}-steps">
        <h3 id="{project.id}-steps" class="text-label muted">When you install</h3>
        <ol>
          {#if kind === 'mod'}
            <li><span class="num text-counter" aria-hidden="true">1</span><span class="text-body">Cubby saves {project.title} in the <span class="text-mono">mods</span> folder of {target.name}, with the mods it needs.</span></li>
            <li><span class="num text-counter" aria-hidden="true">2</span><span class="text-body">It loads the next time you start the game.</span></li>
            <li><span class="num text-counter" aria-hidden="true">3</span><span class="text-body">Most mods have settings: look for them in the game's options, or install Mod Menu on Fabric.</span></li>
          {:else if kind === 'shader'}
            <li><span class="num text-counter" aria-hidden="true">1</span><span class="text-body">Cubby adds the pack to the <span class="text-mono">shaderpacks</span> folder of {target.name}.</span></li>
            <li>
              <span class="num text-counter" aria-hidden="true">2</span>
              <span class="text-body">
                {turnOn ? 'It selects the pack for the next time you play. You can change it in the Shaders tab.' : "The pack stays off until you pick it in the Shaders tab, or in game: Options, Video settings, Shader packs."}
              </span>
            </li>
            <li><span class="num text-counter" aria-hidden="true">3</span><span class="text-body">Worlds and mods don't change. With Iris, press <Kbd keys="K" /> in game to turn shaders on or off.</span></li>
          {:else}
            <li><span class="num text-counter" aria-hidden="true">1</span><span class="text-body">Cubby saves the pack in the <span class="text-mono">resourcepacks</span> folder of {target.name}.</span></li>
            <li>
              <span class="num text-counter" aria-hidden="true">2</span>
              <span class="text-body">
                {turnOn ? 'It turns it on above your other packs.' : 'It stays off until you turn it on in the Resource packs tab.'} The order matters: a pack higher in the list wins where two change the same thing.
              </span>
            </li>
            <li><span class="num text-counter" aria-hidden="true">3</span><span class="text-body">Changes apply when the game starts. In game, press <Kbd keys="F3" /> + <Kbd keys="T" /> to reload packs.</span></li>
          {/if}
        </ol>
      </section>
    {/if}
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-1) var(--space-2);
  }
  /* .panel first: wins over the dialog body's paragraph spacing */
  .panel h2,
  .panel h3,
  .panel p,
  .panel ol,
  .panel ul,
  .panel dl,
  .panel dd {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .ink {
    color: var(--ink);
  }
  .good,
  .ok {
    color: var(--grass-text);
  }
  .warn {
    color: var(--gold-text);
  }
  .bad {
    color: var(--redstone-text);
  }
  .busy {
    color: var(--diamond-text);
  }
  .info,
  .have {
    color: var(--ink-muted);
  }
  .line {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }
  .line :global(.cb-icon) {
    margin-top: 2px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .stack {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .vopt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .vopt .good {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .placeholder {
    display: block;
    height: var(--size-control-lg);
    background: var(--surface);
    border: var(--border-block) dashed var(--line);
  }
  .note-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .have {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .named {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .named :global(.cb-tf) {
    flex: 1;
  }
  .named :global(.cb-instance-icon) {
    margin-top: 28px;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .checks {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0;
    list-style: none;
    background: var(--line);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .checks li {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 2px var(--space-3);
    align-items: start;
    padding: 9px var(--space-3);
    background: var(--surface);
  }
  .checks li.loading {
    display: block;
  }
  .checks li.loading .placeholder {
    height: 40px;
    border: 0;
  }
  .ic {
    display: flex;
    padding-top: 2px;
  }
  .detail {
    grid-column: 2 / 4;
  }
  .detail :global(.cb-btn) {
    margin-top: var(--space-1);
  }
  .req {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .panel :global(.wide) {
    width: 100%;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .tips {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0;
    list-style: none;
  }
  .tips li {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    color: var(--ink-muted);
  }
  .tips li :global(.cb-icon) {
    flex: none;
    margin-top: 1px;
    color: var(--diamond-text);
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding-top: var(--space-4);
    border-top: var(--border-hair) solid var(--line);
  }
  .steps ol {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 10px;
    align-items: start;
  }
  .num {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    box-sizing: border-box;
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .link {
    padding: 0;
    font: inherit;
    color: var(--grass-text);
    text-decoration: underline;
    text-underline-offset: 3px;
    background: none;
    border: 0;
    cursor: pointer;
  }
  .link:hover {
    color: var(--ink);
  }
  .link:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
</style>
