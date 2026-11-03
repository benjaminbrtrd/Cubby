<script lang="ts">
  import { untrack } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, LogViewer, PlayButton, Switch } from '../../cubby'
  import { LOCALE, loaderFull, plural, playTime, timeOfDay } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, ModFile, World } from '../../types'
  import { LogParser, diagnose, type Party } from './crash'

  let { id }: { id: string } = $props()

  const instance = $derived(app.instance(id))
  const name = $derived(instance?.name ?? id)
  const crash = $derived(app.crash?.instanceId === id ? app.crash : null)
  const running = $derived(app.running[id] != null)
  const launch = $derived(app.tasks[`launch:${id}`])

  // Data: the log of the session that crashed, the mods and worlds of the instance
  $effect(() => {
    const lid = id
    untrack(() => {
      if (app.logs[lid]?.length) return
      api
        .gameLog(lid)
        .then((lines) => {
          if (!app.logs[lid]?.length && lines.length) app.logs[lid] = lines
        })
        .catch(() => {})
    })
  })

  let mods = $state<ModFile[]>([])
  let worlds = $state<World[]>([])

  async function loadMods() {
    try {
      mods = await api.listMods(id)
    } catch {
      mods = []
    }
  }

  $effect(() => {
    untrack(() => {
      void loadMods()
      api.listWorlds(id).then(
        (w) => (worlds = w),
        () => (worlds = []),
      )
    })
  })

  const parser = new LogParser()
  const parsed = $derived(parser.parse(app.logs[id] ?? []))
  const facts = $derived.by(() => {
    void parsed
    return parser.facts
  })
  const d = $derived(diagnose(parsed.map((l) => l.raw), mods, crash?.exitCode ?? null))
  const excerpt = $derived(parsed.slice(-8))
  const enabledMods = $derived(mods.filter((m) => m.enabled).length)
  const lastWorld = $derived([...worlds].sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? ''))[0] ?? null)

  // Memory
  const currentMb = $derived(instance?.javaOverrides.maxMemoryMb ?? app.settings?.java.maxMemoryMb ?? 4096)
  const totalMb = $derived(app.info?.totalMemoryMb ?? 0)
  /** 2 GB more, keeping 2 GB for Windows and the other programs. */
  const raisedMb = $derived.by(() => {
    const cap = totalMb ? Math.floor((totalMb - 2048) / 512) * 512 : currentMb + 2048
    return Math.min(currentMb + 2048, cap)
  })
  const canRaise = $derived(raisedMb > currentMb)
  /** The memory the game had when it crashed (the fixes below may change it). */
  const checkedMemoryMb = untrack(() => currentMb)

  function gb(mb: number): string {
    return `${(mb / 1024).toLocaleString(LOCALE, { maximumFractionDigits: 1 })} GB`
  }

  function after(secs: number): string {
    if (secs < 60) return plural(secs, 'second', 'seconds')
    if (secs < 3600) return plural(Math.floor(secs / 60), 'minute', 'minutes')
    return playTime(secs)
  }

  // Text with identifiers: package names, mod ids and versions are shown in mono.
  type Part = string | { mono: string }

  /** A mod in a sentence: its name (and version), else the id or package the log gives. */
  function who(p: Party | null | undefined, fallback: string, version = false): Part[] {
    if (!p) return [fallback]
    if (p.name) return version && p.mod?.version ? [p.name, ' ', { mono: p.mod.version }] : [p.name]
    return [{ mono: p.ref }]
  }

  function plain(parts: Part[]): string {
    return parts.map((p) => (typeof p === 'string' ? p : p.mono)).join('')
  }

  function labelOf(p: Party): string {
    return p.name ?? p.ref
  }

  // What happened, in plain words
  const LIBRARIES =
    /\b(fabric api|quilted fabric api|qsl|fabric language kotlin|architectury|cloth config|yetanotherconfiglib|geckolib|forge config api|balm|puzzles lib|kotlin for forge)\b/i

  const cause = $derived.by((): { title: Part[]; text: Part[]; marks: Part[] } => {
    const a = d.culprit
    const b = d.other
    const lead = 'Most likely cause: '
    switch (d.kind) {
      case 'memory':
        return {
          title: [lead, 'the game ran out of memory'],
          text: [`${name} can use up to ${gb(checkedMemoryMb)}, and the game needed more. Many mods, shader packs or a long render distance need more memory. Your PC is fine.`],
          marks: ['Here marks where Java ran out of memory.'],
        }
      case 'java': {
        const { needed, running: ran } = d.java
        return {
          title: [lead, 'the wrong Java version'],
          text: needed
            ? ['The game or one of its mods needs Java ', { mono: String(needed) }, ' or newer', ...(ran ? [', and it ran on Java ', { mono: String(ran) }] : []), '.']
            : ['The game or one of its mods needs another version of Java than the one it ran on.'],
          marks: ['Here marks the line that says so.'],
        }
      }
      case 'missing': {
        const first = d.missing[0]
        const req = who(first.by, 'a mod')
        const absent = d.missing.filter((n) => !n.wrong)
        const wrong = d.missing.filter((n) => n.wrong)
        const list = (needs: typeof d.missing): Part[] => needs.flatMap((n, i) => [...(i ? [', '] : []), ...who(n.dep, 'a mod')])
        let title: Part[]
        if (d.missing.length > 1) {
          title = !wrong.length
            ? [lead, plural(d.missing.length, 'mod is', 'mods are'), ' missing']
            : !absent.length
              ? [lead, plural(d.missing.length, 'mod needs', 'mods need'), ' another version']
              : [lead, 'some mods are missing or need another version']
        } else if (first.wrong) title = [lead, ...req, ' needs another version of ', ...who(first.dep, 'a mod')]
        else if (first.dep.mod && !first.dep.mod.enabled) title = [lead, ...req, ' needs ', ...who(first.dep, 'a mod'), ', which is turned off']
        else title = [lead, ...req, ' needs ', ...who(first.dep, 'a mod'), ', which isn’t installed']
        return {
          title,
          text: [
            'Before the game starts, the loader checks that every mod has what it needs, and it stopped there.',
            ...(d.missing.length > 1 && absent.length ? [' Missing: ', ...list(absent), '.'] : []),
            ...(d.missing.length > 1 && wrong.length ? [' Another version needed: ', ...list(wrong), '.'] : []),
            ' Your PC is fine.',
          ],
          marks: ['Here marks the line that names it.'],
        }
      }
      case 'conflict':
        return {
          title: [lead, ...who(a, 'a mod', true), ' doesn’t work with ', ...who(b, 'this setup', true)],
          text: b
            ? [...who(a, 'A mod'), ' asks ', ...who(b, 'another mod'), ' for a part that isn’t there, so the game had to stop. Usually one of them is newer than the other expects. Your PC is fine.']
            : [...who(a, 'A mod'), ' asks for a part that isn’t there, so the game had to stop. Your PC is fine.'],
          marks: ['Here marks where ', ...who(a, 'the mod'), ' asks for the missing part.'],
        }
      case 'version':
        return {
          title: [lead, ...who(a, 'a mod', true), ' is made for another version of Minecraft'],
          text: d.madeFor
            ? [...who(a, 'A mod'), ' is made for Minecraft ', { mono: d.madeFor }, ', and this instance runs ', { mono: instance?.minecraft ?? '' }, '. Mods only work with the game versions they’re made for. Your PC is fine.']
            : [...who(a, 'A mod'), ' asks Minecraft ', { mono: instance?.minecraft ?? '' }, ' for a part that isn’t there. Mods only work with the game versions they’re made for.'],
          marks: d.madeFor ? ['Here marks the line that names it.'] : ['Here marks where ', ...who(a, 'the mod'), ' asks for the missing part.'],
        }
      case 'mixin':
        return {
          title: [lead, ...who(a, 'a mod', true), ' couldn’t load', ...(b ? [' with ', ...who(b, 'another mod', true)] : [])],
          text: [
            'When the game starts, ',
            ...who(a, 'the mod'),
            ' changes parts of ',
            ...who(b, 'the game'),
            '. That change failed, which usually means it’s made for another version of ',
            ...(b ? who(b, 'that mod') : ['the game or the loader']),
            '.',
          ],
          marks: ['Here marks the change that failed.'],
        }
      case 'driver':
        return {
          title: [lead, 'the graphics driver crashed'],
          text: d.frame
            ? ['The crash happened inside ', { mono: d.frame }, ', a file of your graphics driver. Updating the driver usually fixes it.']
            : ['The game couldn’t use OpenGL with your graphics driver. Updating the driver usually fixes it.'],
          marks: ['Here marks the driver file.'],
        }
      case 'jvm-start':
        return {
          title: [lead, 'Java couldn’t start'],
          text: [`Java refused the settings it was given. Check the memory and the Java arguments of ${name}.`],
          marks: ['Here marks what Java said.'],
        }
      case 'jvm-crash':
        return {
          title: [lead, 'Java itself crashed'],
          text: [
            'Java stopped with a fatal error',
            ...(d.frame ? [' in ', { mono: d.frame }] : []),
            '. Custom Java arguments, a mod with native code or a graphics driver can cause it.',
          ],
          marks: ['Here marks where Java stopped.'],
        }
      default:
        return {
          title: ['Cubby couldn’t find the cause'],
          text: [
            !parsed.length
              ? 'The game wrote nothing to its log before it closed.'
              : d.evidence.length
                ? 'The log doesn’t show a problem Cubby knows. These are the last errors the game wrote.'
                : 'The game wrote no error before it closed.',
          ],
          marks: ['Here marks where the last error starts.'],
        }
    }
  })

  /** "com.simibubi.create is Create": the packages the log names, and the mods they belong to. */
  const named = $derived.by((): Part[] => {
    const out: Part[] = []
    for (const p of [d.culprit, d.other]) {
      if (!p || !p.ref.includes('.')) continue
      if (out.length) out.push(' and ')
      out.push({ mono: p.ref }, p.mod ? ` is ${p.name ?? p.mod.fileName}` : ' isn’t one of your mods')
    }
    if (out.length) out.push('. ')
    return out
  })

  // Ways to fix it
  type FixId = 'disable' | 'enable' | 'find' | 'memory' | 'java' | 'updates' | 'driver' | 'settings' | 'repair' | 'help'
  interface Fix {
    key: string
    id: FixId
    title: Part[]
    text: Part[]
    /** `true`: can be undone (check and words). `false`: nothing to undo (info and words). */
    undoable: boolean
    undo: string
    party?: Party
  }

  let busy = $state<string | null>(null)
  let applied = $state<Record<string, boolean>>({})
  let memoryBefore = $state<number | null>(null)
  let javaBefore = $state<string | null>(null)
  let disabled = $state<{ label: string; fileName: string; backup: string | null } | null>(null)
  let confirmDisable = $state<Party | null>(null)
  let backupFirst = $state(true)

  const anyApplied = $derived(Object.values(applied).some(Boolean) || !!disabled)

  /** What to type in Discover to find a missing mod. */
  function searchTerm(p: Party): string {
    if (p.name) return p.name
    if (!p.ref.includes('.')) return p.ref
    const skip = ['com', 'net', 'org', 'me', 'dev', 'io', 'github', 'mods', 'mod', 'api', 'client', 'common']
    const segs = p.ref.split('.').filter((s) => !skip.includes(s))
    return (segs[segs.length - 1] ?? p.ref).replace(/\d+$/, '')
  }

  function baseName(mod: ModFile): string {
    return mod.fileName.replace(/\.disabled$/, '')
  }

  function sameMod(a: Party | null, b: Party | null): boolean {
    return !!a?.mod && !!b?.mod && a.mod.fileName === b.mod.fileName
  }

  const fixes = $derived.by((): Fix[] => {
    const out: Fix[] = []
    const n = name
    const repair = (first: boolean): Fix => ({
      key: 'repair',
      id: 'repair',
      title: [first ? `Repair ${n}` : 'If it still crashes'],
      text: ['Downloads the game files again and checks them. Keeps your worlds, mods and settings.'],
      undoable: false,
      undo: 'Nothing to undo',
    })
    const settings = (text: Part[]): Fix => ({
      key: 'settings',
      id: 'settings',
      title: ['Check the instance settings'],
      text,
      undoable: false,
      undo: 'Nothing changes on its own',
    })
    const updates = (parties: (Party | null)[]): Fix => {
      const list = parties.filter((p): p is Party => !!p)
      const names: Part[] = list.flatMap((p, i) => [...(i ? [' and '] : []), ...who(p, 'a mod')])
      return {
        key: 'updates',
        id: 'updates',
        title: list.length ? ['Look for updates of ', ...names] : ['Look at your recent mods'],
        text: [
          list.length
            ? `A newer version of ${list.length > 1 ? 'one of them' : 'it'} often fixes this. Opens the mods of ${n}.`
            : `A mod you added or updated lately may be the cause. Opens the mods of ${n}.`,
        ],
        undoable: false,
        undo: 'Nothing changes until you update',
      }
    }
    const disable = (p: Party | null) => {
      if (!p?.mod || LIBRARIES.test(p.mod.name ?? p.mod.fileName)) return
      if (!p.mod.enabled && disabled?.fileName !== p.mod.fileName) return
      const m = who(p, 'the mod')
      out.push({
        key: 'disable',
        id: 'disable',
        party: p,
        title: ['Play without ', ...m, ' for now'],
        text: lastWorld
          ? [`Cubby can back up ${lastWorld.name} first. Without `, ...m, ', its blocks and items are removed from the places you visit; restore the backup to get them back.']
          : ['Without ', ...m, ', its blocks and items are removed from the places you visit.'],
        undoable: true,
        undo: 'Turn it back on any time',
      })
    }

    switch (d.kind) {
      case 'memory':
        if (canRaise || applied.memory) {
          out.push({
            key: 'memory',
            id: 'memory',
            title: [`Give ${n} more memory`],
            text: [
              applied.memory
                ? `${n} can now use up to ${gb(currentMb)}.`
                : `Raises its memory limit from ${gb(currentMb)} to ${gb(raisedMb)}.${totalMb ? ` Your PC has ${gb(totalMb)}.` : ''}`,
            ],
            undoable: true,
            undo: 'Undo any time',
          })
        } else {
          out.push({
            ...updates([]),
            key: 'memory',
            title: ['Free some memory'],
            text: [`${n} already uses ${gb(currentMb)}${totalMb ? ` of your ${gb(totalMb)}` : ''}. Close other programs, or turn off some mods or shader packs.`],
          })
        }
        out.push(settings([`Set the memory of ${n} yourself.`]))
        out.push(repair(false))
        break
      case 'java':
        if (instance?.javaOverrides.executable || applied.java) {
          out.push({
            key: 'java',
            id: 'java',
            title: ['Let Cubby pick Java'],
            text: [applied.java ? `${n} now uses the Java Cubby picks for the game.` : `${n} uses a Java you chose. Cubby can pick the one that fits the game instead.`],
            undoable: true,
            undo: 'Undo any time',
          })
        }
        out.push(settings(d.java.needed ? ['Choose Java ', { mono: String(d.java.needed) }, ` or newer for ${n}.`] : [`Choose another Java for ${n}.`]))
        out.push(repair(false))
        break
      case 'missing':
        for (const { dep: p, by, wrong } of d.missing.slice(0, 3)) {
          const req = who(by, 'another mod')
          if (p.mod && (!p.mod.enabled || applied[`enable:${baseName(p.mod)}`])) {
            out.push({
              key: `enable:${baseName(p.mod)}`,
              id: 'enable',
              party: p,
              title: ['Turn ', ...who(p, 'it'), ' back on'],
              text: ['It’s installed but turned off, and ', ...req, ' needs it.'],
              undoable: true,
              undo: 'Turn it off again in Mods',
            })
          } else if (p.mod && wrong) {
            out.push({
              ...updates([p]),
              key: `update:${baseName(p.mod)}`,
              title: ['Update ', ...who(p, 'it')],
              text: [...who(by, 'Another mod'), ` needs another version of it. Opens the mods of ${n}.`],
            })
          } else {
            out.push({
              key: `find:${p.ref}`,
              id: 'find',
              party: p,
              title: ['Install ', ...who(p, 'it')],
              text: [...(by ? [...req, ' needs it. '] : []), 'Opens Discover with ', { mono: searchTerm(p) }, `, for ${n}.`],
              undoable: false,
              undo: 'Nothing changes until you install',
            })
          }
        }
        if (d.culprit && !d.missing.some((n) => sameMod(n.dep, d.culprit))) disable(d.culprit)
        break
      case 'conflict':
      case 'version':
      case 'mixin':
        disable(d.culprit)
        out.push(updates(d.kind === 'version' ? [d.culprit] : [d.culprit, d.other]))
        out.push(repair(false))
        break
      case 'driver':
        out.push({
          key: 'driver',
          id: 'driver',
          title: ['Update your graphics driver'],
          text: ['Get the latest driver from NVIDIA, AMD or Intel, then play again.'],
          undoable: false,
          undo: 'Done outside Cubby',
        })
        out.push(settings([`Remove custom Java arguments from ${n}, if you added some.`]))
        out.push(repair(false))
        break
      case 'jvm-start':
        out.push(settings([`Look at the memory and the Java arguments of ${n}.`]))
        out.push(repair(false))
        break
      case 'jvm-crash':
        out.push(settings([`Remove custom Java arguments from ${n}, if you added some.`]))
        out.push({
          key: 'driver',
          id: 'driver',
          title: ['Update your graphics driver'],
          text: ['A crash in Java often comes from the driver. Get the latest one from NVIDIA, AMD or Intel.'],
          undoable: false,
          undo: 'Done outside Cubby',
        })
        out.push(repair(false))
        break
      default:
        out.push(repair(true))
        out.push(updates([]))
        out.push({
          key: 'help',
          id: 'help',
          title: ['Ask for help'],
          text: ['Copy the report and share it with the mod authors or on a forum. It has the end of the game log.'],
          undoable: false,
          undo: 'Nothing changes',
        })
    }
    return out
  })

  // Fix actions
  function snapshot(): Instance | null {
    return instance ? ($state.snapshot(instance) as Instance) : null
  }

  async function setMemory(mb: number | null, remember: boolean) {
    const current = snapshot()
    if (!current) return
    busy = 'memory'
    const before = current.javaOverrides.maxMemoryMb
    const saved = await app.updateInstance({ ...current, javaOverrides: { ...current.javaOverrides, maxMemoryMb: mb } })
    busy = null
    if (!saved) return
    if (remember) {
      memoryBefore = before
      applied.memory = true
      app.toast(`${name} can now use up to ${gb(mb ?? currentMb)}.`, 'success')
    } else {
      applied.memory = false
    }
  }

  async function setJava(executable: string | null, remember: boolean) {
    const current = snapshot()
    if (!current) return
    busy = 'java'
    const before = current.javaOverrides.executable
    const saved = await app.updateInstance({ ...current, javaOverrides: { ...current.javaOverrides, executable } })
    busy = null
    if (!saved) return
    if (remember) {
      javaBefore = before
      applied.java = true
    } else {
      applied.java = false
    }
  }

  async function disableMod(p: Party) {
    if (!p.mod) return
    busy = 'disable'
    const backup = backupFirst && lastWorld ? lastWorld : null
    const label = labelOf(p)
    try {
      let backupName: string | null = null
      if (backup) backupName = (await api.backupWorld(id, backup.folder)).fileName
      await api.setModEnabled(id, p.mod.fileName, false)
      disabled = { label, fileName: `${p.mod.fileName}.disabled`, backup: backupName }
      app.toast(backup ? `${label} is off. ${backup.name} is backed up.` : `${label} is off.`, 'success')
      confirmDisable = null
      await loadMods()
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  async function undoDisable() {
    if (!disabled) return
    busy = 'disable'
    try {
      await api.setModEnabled(id, disabled.fileName, true)
      app.toast(`${disabled.label} is on again.`, 'success')
      disabled = null
      await loadMods()
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  async function setEnabled(p: Party, enabled: boolean) {
    if (!p.mod) return
    const key = `enable:${baseName(p.mod)}`
    busy = key
    try {
      await api.setModEnabled(id, p.mod.fileName, enabled)
      applied[key] = enabled
      await loadMods()
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  function find(p: Party) {
    app.discover.kind = 'mod'
    app.discover.query = searchTerm(p)
    app.discover.categories = []
    app.discover.page = 0
    app.setTarget(id)
    app.navigate({ name: 'discover' })
  }

  const repairTask = $derived(app.tasks[`repair:${id}`])

  async function repair() {
    busy = 'repair'
    try {
      await api.repairInstance(id)
      applied.repair = true
      app.toast(`${name}: game files checked.`, 'success')
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  // What Cubby checked
  const checks = $derived.by(() => {
    const s = d.signals
    const found = [...new Set([d.culprit, d.other, ...d.missing.map((n) => n.dep)].filter((p): p is Party => !!p).map(labelOf))]
    const modsBad = s.missing > 0 || s.conflict
    return [
      { label: `Memory, ${gb(checkedMemoryMb)}`, bad: s.memory, word: s.memory ? 'Ran out' : 'OK', detail: null },
      { label: facts.java ? `Java ${facts.java}` : 'Java version', bad: s.java, word: s.java ? 'Wrong version' : 'OK', detail: null },
      { label: 'Graphics driver', bad: s.driver, word: s.driver ? 'Crashed' : 'OK', detail: null },
      { label: 'Java runtime', bad: !!s.jvm, word: s.jvm === 'start' ? 'Couldn’t start' : s.jvm ? 'Crashed' : 'OK', detail: null },
      {
        label: mods.length ? `Mods, ${enabledMods} on` : 'Mods',
        bad: modsBad,
        word: s.missing > 0 ? `${s.missing} missing` : s.conflict ? '1 conflict' : 'OK',
        detail: modsBad && found.length ? `Found: ${found.join(' and ')}` : null,
      },
    ]
  })

  // Report
  function fileNameOf(path: string): string {
    return path.split(/[\\/]/).pop() ?? path
  }

  async function copyReport() {
    const inst = instance
    const tail = (app.logs[id] ?? []).slice(-300).map((l) => l.line)
    const head = [
      crash ? `${name} crashed after ${after(crash.durationSecs)}, exit code ${crash.exitCode}` : `${name} crashed`,
      inst ? `Minecraft ${inst.minecraft}, ${loaderFull(inst.loader)}` : null,
      `Likely cause: ${plain(cause.title).replace(/^Most likely cause: /, '')}`,
      mods.length ? `Mods: ${enabledMods} on, ${mods.length - enabledMods} off` : null,
      d.reportPath ? `Crash report: ${d.reportPath}` : null,
    ].filter((l): l is string => !!l)
    const text = [...head, '', `Last ${tail.length} lines of the game log:`, ...tail].join('\n')
    try {
      await navigator.clipboard.writeText(text)
      app.toast('Report copied to the clipboard.', 'success')
    } catch {
      app.toast('Couldn’t copy the report. Open the folder and copy the file from there.', 'error')
    }
  }

  function openReports() {
    const sub = !d.reportPath && d.jvmReportPath ? undefined : 'crash-reports'
    api.openFolder(id, sub).catch((e) => app.error(e))
  }

  function playAgain() {
    void app.launch(id)
    // Follow the start on the game screen, unless it didn't start (no account, for one).
    if (app.tasks[`launch:${id}`]) app.navigate({ name: 'running', id })
  }
</script>

{#snippet rich(parts: Part[])}{#each parts as p, i (i)}{#if typeof p === 'string'}{p}{:else}<span class="text-mono">{p.mono}</span>{/if}{/each}{/snippet}

{#snippet bannerText()}
  {#if crash}
    The game closed at {timeOfDay(crash.at)} with exit code <code>{crash.exitCode}</code>.
  {/if}
{/snippet}

{#snippet disableActions()}
  <Button variant="ghost" onclick={() => (confirmDisable = null)}>Cancel</Button>
  <Button variant="primary" icon="mods" busy={busy === 'disable'} disabled={busy === 'disable'} onclick={() => confirmDisable && disableMod(confirmDisable)}>
    {#if busy === 'disable'}
      {backupFirst && lastWorld ? 'Backing up…' : 'Turning off…'}
    {:else}
      Turn off {@render rich(who(confirmDisable, 'the mod'))}
    {/if}
  </Button>
{/snippet}

<div class="screen">
  {#if !crash}
    <div class="content narrow">
      <h1 class="text-title">{name}</h1>
      <Banner tone="success" title="No recent crash">
        {instance ? `${name} hasn’t crashed since Cubby opened.` : 'This instance isn’t here anymore.'}
      </Banner>
      <div class="row">
        {#if instance}
          <Button icon="cube" onclick={() => app.openInstance(id)}>Open {name}</Button>
          <Button icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>Show the game log</Button>
        {:else}
          <Button icon="library" onclick={() => app.navigate({ name: 'library' })}>Back to the library</Button>
        {/if}
      </div>
    </div>
  {:else}
    <div class="layout">
      <div class="content">
        <h1 class="cb-visually-hidden">{name} crash report</h1>
        <Banner tone="danger" title={crash.durationSecs > 0 ? `${name} crashed after ${after(crash.durationSecs)}` : `${name} crashed`}>
          {@render bannerText()}
        </Banner>

        <section class="card what" aria-labelledby="what-title">
          <h2 id="what-title" class="text-label muted">What happened</h2>
          <p class="text-heading">{@render rich(cause.title)}</p>
          <p class="text-body">{@render rich(cause.text)}</p>
          {#if d.evidence.length}
            <div class="evidence" role="group" aria-label="Evidence from the game log">
              {#each d.evidence as e, i (i)}
                {#if e.gap}
                  <div class="ev"><span class="mark" aria-hidden="true"></span><code class="text-log muted">…</code></div>
                {/if}
                <div class={['ev', e.culprit && 'is-culprit']}>
                  {#if e.culprit}
                    <span class="mark text-label">Here<Icon name="chevron-right" size={18} /></span>
                  {:else}
                    <span class="mark" aria-hidden="true"></span>
                  {/if}
                  <code class="text-log">{e.text}</code>
                </div>
              {/each}
            </div>
            <p class="text-small muted">{@render rich([...named, ...cause.marks])}</p>
          {/if}
        </section>

        <section class="fixes" aria-labelledby="fix-title">
          <div class="sec-head">
            <h2 id="fix-title" class="text-label muted">Ways to fix it</h2>
            <span class="text-small muted">Best first.</span>
          </div>
          <ol class="card list">
            {#each fixes as f, i (f.key)}
              {@const variant = i === 0 ? 'primary' : 'secondary'}
              <li class="fix">
                <span class="num text-counter" aria-hidden="true">{i + 1}</span>
                <div class="fix-body">
                  <h3 class="text-body-strong"><span class="cb-visually-hidden">Fix {i + 1}: </span>{@render rich(f.title)}</h3>
                  <p class="text-small muted">{@render rich(f.text)}</p>
                </div>
                <div class="fix-action">
                  {#if f.id === 'disable' && f.party}
                    {#if disabled}
                      <span class="done text-small"><Icon name="check" size={18} />{disabled.backup ? 'Off, world backed up' : 'Off'}</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === 'disable'} onclick={undoDisable}>Turn it back on</Button>
                    {:else}
                      <Button {variant} icon="mods" onclick={() => (confirmDisable = f.party ?? null)}>Turn off {@render rich(who(f.party, 'the mod'))}</Button>
                    {/if}
                  {:else if f.id === 'enable' && f.party?.mod}
                    {#if applied[f.key]}
                      <span class="done text-small"><Icon name="check" size={18} />On</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === f.key} onclick={() => f.party && setEnabled(f.party, false)}>Undo</Button>
                    {:else}
                      <Button {variant} icon="mods" busy={busy === f.key} disabled={busy === f.key} onclick={() => f.party && setEnabled(f.party, true)}>
                        Turn on {@render rich(who(f.party, 'it'))}
                      </Button>
                    {/if}
                  {:else if f.id === 'find' && f.party}
                    <Button {variant} icon="discover" onclick={() => f.party && find(f.party)}>Find {@render rich(who(f.party, 'it'))}</Button>
                  {:else if f.id === 'memory'}
                    {#if applied.memory}
                      <span class="done text-small"><Icon name="check" size={18} />Now {gb(currentMb)}</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === 'memory'} onclick={() => setMemory(memoryBefore, false)}>Undo</Button>
                    {:else}
                      <Button {variant} icon="memory" busy={busy === 'memory'} disabled={busy === 'memory'} onclick={() => setMemory(raisedMb, true)}>Use {gb(raisedMb)}</Button>
                    {/if}
                  {:else if f.id === 'java'}
                    {#if applied.java}
                      <span class="done text-small"><Icon name="check" size={18} />Automatic Java</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === 'java'} onclick={() => setJava(javaBefore, false)}>Undo</Button>
                    {:else}
                      <Button {variant} icon="wrench" busy={busy === 'java'} disabled={busy === 'java'} onclick={() => setJava(null, true)}>Use automatic Java</Button>
                    {/if}
                  {:else if f.id === 'updates'}
                    <Button {variant} icon="mods" onclick={() => app.openInstance(id, 'mods')}>Open mods</Button>
                  {:else if f.id === 'settings'}
                    <Button {variant} icon="settings" onclick={() => app.openInstance(id, 'settings')}>Open instance settings</Button>
                  {:else if f.id === 'repair'}
                    {#if applied.repair && !repairTask}
                      <span class="done text-small"><Icon name="check" size={18} />Game files checked</span>
                    {:else}
                      <Button
                        {variant}
                        icon="wrench"
                        busy={!!repairTask || busy === 'repair'}
                        disabled={!!repairTask || busy === 'repair' || running}
                        onclick={repair}
                      >
                        {repairTask ? `Repairing${repairTask.total ? ` ${Math.round((repairTask.done / repairTask.total) * 100)}%` : '…'}` : 'Repair instance'}
                      </Button>
                    {/if}
                  {:else if f.id === 'help'}
                    <Button {variant} icon="copy" onclick={copyReport}>Copy report</Button>
                  {/if}
                  <span class={['undo text-small', f.undoable ? 'is-undoable' : 'muted']}>
                    <Icon name={f.undoable ? 'check' : 'info'} size={18} />{f.undo}
                  </span>
                </div>
              </li>
            {/each}
          </ol>
        </section>
      </div>

      <aside class="aside" aria-label="Crash details">
        <section class="block" aria-labelledby="checked-title">
          <h2 id="checked-title" class="text-label muted">What Cubby checked</h2>
          {#if parsed.length}
            <ul class="checks">
              {#each checks as c (c.label)}
                <li class="check">
                  <div class="check-row">
                    <span class="text-body check-label">{c.label}</span>
                    <span class={['text-body-strong state', c.bad ? 'is-bad' : 'is-ok']}>
                      <Icon name={c.bad ? 'error' : 'check'} size={18} />{c.word}
                    </span>
                  </div>
                  {#if c.detail}<span class="text-small muted">{c.detail}</span>{/if}
                </li>
              {/each}
            </ul>
            <p class="text-small muted">Cubby looked for these in the last {plural(d.checkedLines, 'line', 'lines')} of the game log.</p>
          {:else}
            <p class="text-small muted">The game wrote nothing to its log, so there was nothing to check.</p>
          {/if}
        </section>

        <section class="block" aria-labelledby="report-title">
          <h2 id="report-title" class="text-label muted">Crash report</h2>
          {#if d.reportPath}
            <p class="text-mono path">crash-reports/{fileNameOf(d.reportPath)}</p>
          {:else if d.jvmReportPath}
            <p class="text-mono path">{fileNameOf(d.jvmReportPath)}</p>
            <p class="text-small muted">Java wrote this report in the game folder.</p>
          {:else}
            <p class="text-small muted">The game didn’t write a crash report. The log has the details.</p>
          {/if}
          <div class="stack">
            <Button size="sm" icon="copy" onclick={copyReport}>Copy report</Button>
            <Button size="sm" icon="folder" onclick={openReports}>Open folder</Button>
          </div>
        </section>

        {#if excerpt.length}
          <section class="block" aria-labelledby="excerpt-title">
            <h2 id="excerpt-title" class="text-label muted">Last {plural(excerpt.length, 'log line', 'log lines')}</h2>
            <LogViewer lines={excerpt} height={184} label="Last lines of the game log" />
          </section>
        {/if}

        <div class="play">
          {#if running}
            <Button icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>Show the running game</Button>
          {:else if launch}
            <PlayButton state="installing" detail={launch.label} progress={launch.total ? (launch.done / launch.total) * 100 : undefined} />
          {:else}
            <PlayButton
              label="Play again"
              detail={fixes.length ? (anyApplied ? 'With your fix' : 'Pick a fix first') : undefined}
              aria-label={`Play ${name} again`}
              onclick={playAgain}
            />
          {/if}
        </div>
      </aside>
    </div>
  {/if}
</div>

{#if confirmDisable}
  {@const mod = plain(who(confirmDisable, 'this mod'))}
  <Dialog title={`Turn off ${mod}?`} icon="mods" tone="warning" onClose={() => (confirmDisable = null)} actions={disableActions}>
    <p>
      Without {@render rich(who(confirmDisable, 'it'))}, its blocks and items are removed from the places you visit, and turning it back on doesn’t
      bring them back.{lastWorld ? ` A backup of ${lastWorld.name} lets you restore them.` : ''}
    </p>
    {#if lastWorld}
      <Switch bind:checked={backupFirst} label={`Back up ${lastWorld.name} first`} hint="Saved with the backups of this instance." />
    {/if}
    <p class="text-small muted">You can turn it back on in Mods.</p>
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .layout {
    flex: 1;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
  }
  .narrow {
    max-width: 720px;
  }
  h1,
  h2,
  h3,
  p,
  ol,
  ul {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .card {
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }

  .what {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4);
  }
  .what > .text-body {
    margin-bottom: var(--space-1);
  }
  .evidence {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-2) 0;
    background: var(--field);
    border: var(--border-block) solid var(--outline);
    box-shadow: var(--shadow-well);
  }
  .ev {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding: 2px var(--space-3) 2px var(--space-2);
    border-inline-start: 4px solid transparent;
  }
  .ev code {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  .ev.is-culprit {
    background: var(--surface);
    border-inline-start-color: var(--redstone);
  }
  .mark {
    flex: none;
    width: 80px;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--redstone-text);
  }

  .fixes {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .sec-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-3);
  }
  .list {
    list-style: none;
    padding: 0;
  }
  .fix {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-3) var(--space-4);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .fix:last-child {
    border-bottom: 0;
  }
  .num {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    background: var(--bg-chrome);
    border: var(--border-block) solid var(--edge);
  }
  .fix-body {
    flex: 1 1 260px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fix-action {
    flex: 0 0 248px;
    margin-left: auto;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    max-width: 100%;
  }
  .undo,
  .done {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .undo.is-undoable,
  .done {
    color: var(--grass-text);
  }
  .done {
    font-weight: 500;
  }

  .aside {
    flex: none;
    width: var(--size-inspector);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .checks {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .check {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .check-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .check-label {
    flex: 1;
    min-width: 0;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    text-align: end;
  }
  .state.is-ok {
    color: var(--grass-text);
  }
  .state.is-bad {
    color: var(--redstone-text);
  }
  .path {
    overflow-wrap: anywhere;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .stack > :global(.cb-btn) {
    width: 100%;
  }
  .play {
    margin-top: auto;
    display: flex;
    flex-direction: column;
  }
  .play > :global(.cb-play),
  .play > :global(.cb-btn) {
    width: 100%;
  }
</style>
