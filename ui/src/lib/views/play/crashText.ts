// The words of the crash screen: what happened, the ways to fix it and what Cubby checked, built from a
// diagnosis. Pure functions; the screen (Crash.svelte) owns the state and the actions.
import { locale, plural, playTime } from '../../format'
import type { ModFile, World } from '../../types'
import type { Diagnosis, Party } from './crash'
import type { SessionFacts } from './log'
import { t } from '../../i18n.svelte'

/** Text with identifiers: package names, mod ids and versions are shown in mono. */
export type Part = string | { mono: string }

export function gb(mb: number): string {
  return `${(mb / 1024).toLocaleString(locale(), { maximumFractionDigits: 1 })} GB`
}

/** How long the game ran, in words. */
export function after(secs: number): string {
  if (secs < 60) return plural(secs, 'second', 'seconds')
  if (secs < 3600) return plural(Math.floor(secs / 60), 'minute', 'minutes')
  return playTime(secs)
}

/** A mod in a sentence: its name (and version), else the id or package the log gives. */
export function who(p: Party | null | undefined, fallback: string, version = false): Part[] {
  if (!p) return [fallback]
  if (p.name) return version && p.mod?.version ? [p.name, ' ', { mono: p.mod.version }] : [p.name]
  return [{ mono: p.ref }]
}

export function plain(parts: Part[]): string {
  return parts.map((p) => (typeof p === 'string' ? p : p.mono)).join('')
}

export function labelOf(p: Party): string {
  return p.name ?? p.ref
}

/** What to type in Discover to find a missing mod. */
export function searchTerm(p: Party): string {
  if (p.name) return p.name
  if (!p.ref.includes('.')) return p.ref
  const skip = ['com', 'net', 'org', 'me', 'dev', 'io', 'github', 'mods', 'mod', 'api', 'client', 'common']
  const segs = p.ref.split('.').filter((s) => !skip.includes(s))
  return (segs[segs.length - 1] ?? p.ref).replace(/\d+$/, '')
}

export function baseName(mod: ModFile): string {
  return mod.fileName.replace(/\.disabled$/, '')
}

function sameMod(a: Party | null, b: Party | null): boolean {
  return !!a?.mod && !!b?.mod && a.mod.fileName === b.mod.fileName
}

/** Libraries other mods need: turning one off only moves the crash. */
const LIBRARIES =
  /\b(fabric api|quilted fabric api|qsl|fabric language kotlin|architectury|cloth config|yetanotherconfiglib|geckolib|forge config api|balm|puzzles lib|kotlin for forge)\b/i

// What happened, in plain words

export interface Cause {
  title: Part[]
  text: Part[]
  /** What the "Here" mark in the evidence points at. */
  marks: Part[]
}

export function causeOf(d: Diagnosis, ctx: { name: string; memoryMb: number; minecraft: string; logLines: number }): Cause {
  const { name } = ctx
  const a = d.culprit
  const b = d.other
  const lead = t('Most likely cause: ')
  switch (d.kind) {
    case 'memory':
      return {
        title: [lead, t('the game ran out of memory')],
        text: [t('{name} can use up to {n}, and the game needed more. Many mods, shader packs or a long render distance need more memory. Your PC is fine.', { name, n: gb(ctx.memoryMb) })],
        marks: [t('Here marks where Java ran out of memory.')],
      }
    case 'java': {
      const { needed, running: ran } = d.java
      return {
        title: [lead, t('the wrong Java version')],
        text: needed
          ? [t('The game or one of its mods needs Java '), { mono: String(needed) }, t(' or newer'), ...(ran ? [t(', and it ran on Java '), { mono: String(ran) }] : []), '.']
          : [t('The game or one of its mods needs another version of Java than the one it ran on.')],
        marks: [t('Here marks the line that says so.')],
      }
    }
    case 'missing': {
      const first = d.missing[0]
      const req = who(first.by, t('a mod'))
      const absent = d.missing.filter((n) => !n.wrong)
      const wrong = d.missing.filter((n) => n.wrong)
      const list = (needs: typeof d.missing): Part[] => needs.flatMap((n, i) => [...(i ? [', '] : []), ...who(n.dep, t('a mod'))])
      let title: Part[]
      if (d.missing.length > 1) {
        title = !wrong.length
          ? [lead, plural(d.missing.length, 'mod is', 'mods are'), t(' missing')]
          : !absent.length
            ? [lead, plural(d.missing.length, 'mod needs', 'mods need'), t(' another version')]
            : [lead, t('some mods are missing or need another version')]
      } else if (first.wrong) title = [lead, ...req, t(' needs another version of '), ...who(first.dep, t('a mod'))]
      else if (first.dep.mod && !first.dep.mod.enabled) title = [lead, ...req, t(' needs '), ...who(first.dep, t('a mod')), t(', which is turned off')]
      else title = [lead, ...req, t(' needs '), ...who(first.dep, t('a mod')), t(', which isn’t installed')]
      return {
        title,
        text: [
          t('Before the game starts, the loader checks that every mod has what it needs, and it stopped there.'),
          ...(d.missing.length > 1 && absent.length ? [t(' Missing: '), ...list(absent), '.'] : []),
          ...(d.missing.length > 1 && wrong.length ? [t(' Another version needed: '), ...list(wrong), '.'] : []),
          t(' Your PC is fine.'),
        ],
        marks: [t('Here marks the line that names it.')],
      }
    }
    case 'conflict':
      return {
        title: [lead, ...who(a, t('a mod'), true), t(' doesn’t work with '), ...who(b, t('this setup'), true)],
        text: b
          ? [...who(a, t('A mod')), t(' asks '), ...who(b, t('another mod')), t(' for a part that isn’t there, so the game had to stop. Usually one of them is newer than the other expects. Your PC is fine.')]
          : [...who(a, t('A mod')), t(' asks for a part that isn’t there, so the game had to stop. Your PC is fine.')],
        marks: [t('Here marks where '), ...who(a, t('the mod')), t(' asks for the missing part.')],
      }
    case 'version':
      return {
        title: [lead, ...who(a, t('a mod'), true), t(' is made for another version of Minecraft')],
        text: d.madeFor
          ? [...who(a, t('A mod')), t(' is made for Minecraft '), { mono: d.madeFor }, t(', and this instance runs '), { mono: ctx.minecraft }, '. Mods only work with the game versions they’re made for. Your PC is fine.']
          : [...who(a, t('A mod')), t(' asks Minecraft '), { mono: ctx.minecraft }, t(' for a part that isn’t there. Mods only work with the game versions they’re made for.')],
        marks: d.madeFor ? [t('Here marks the line that names it.')] : [t('Here marks where '), ...who(a, t('the mod')), t(' asks for the missing part.')],
      }
    case 'mixin':
      return {
        title: [lead, ...who(a, t('a mod'), true), t(' couldn’t load'), ...(b ? [t(' with '), ...who(b, t('another mod'), true)] : [])],
        text: [
          t('When the game starts, '),
          ...who(a, t('the mod')),
          t(' changes parts of '),
          ...who(b, t('the game')),
          '. That change failed, which usually means it’s made for another version of ',
          ...(b ? who(b, t('that mod')) : [t('the game or the loader')]),
          '.',
        ],
        marks: [t('Here marks the change that failed.')],
      }
    case 'driver':
      return {
        title: [lead, t('the graphics driver crashed')],
        text: d.frame
          ? [t('The crash happened inside '), { mono: d.frame }, t(', a file of your graphics driver. Updating the driver usually fixes it.')]
          : [t('The game couldn’t use OpenGL with your graphics driver. Updating the driver usually fixes it.')],
        marks: [t('Here marks the driver file.')],
      }
    case 'jvm-start':
      return {
        title: [lead, t('Java couldn’t start')],
        text: [t('Java refused the settings it was given. Check the memory and the Java arguments of {name}.', { name })],
        marks: [t('Here marks what Java said.')],
      }
    case 'jvm-crash':
      return {
        title: [lead, t('Java itself crashed')],
        text: [t('Java stopped with a fatal error'), ...(d.frame ? [t(' in '), { mono: d.frame }] : []), '. Custom Java arguments, a mod with native code or a graphics driver can cause it.'],
        marks: [t('Here marks where Java stopped.')],
      }
    default:
      return {
        title: [t('Cubby couldn’t find the cause')],
        text: [
          !ctx.logLines
            ? t('The game wrote nothing to its log before it closed.')
            : d.evidence.length
              ? t('The log doesn’t show a problem Cubby knows. These are the last errors the game wrote.')
              : t('The game wrote no error before it closed.'),
        ],
        marks: [t('Here marks where the last error starts.')],
      }
  }
}

/** "com.simibubi.create is Create": the packages the log names, and the mods they belong to. */
export function namedParts(d: Diagnosis): Part[] {
  const out: Part[] = []
  for (const p of [d.culprit, d.other]) {
    if (!p || !p.ref.includes('.')) continue
    if (out.length) out.push(t(' and '))
    out.push({ mono: p.ref }, p.mod ? t(' is {fileName}', { fileName: p.name ?? p.mod.fileName }) : t(' isn’t one of your mods'))
  }
  if (out.length) out.push('. ')
  return out
}

// Ways to fix it

export type FixId = 'disable' | 'enable' | 'find' | 'memory' | 'java' | 'updates' | 'driver' | 'settings' | 'repair' | 'help'

export interface Fix {
  key: string
  id: FixId
  title: Part[]
  text: Part[]
  /** `true`: can be undone (check and words). `false`: nothing to undo (info and words). */
  undoable: boolean
  undo: string
  party?: Party
}

export interface FixContext {
  name: string
  lastWorld: World | null
  /** File name of the mod the player turned off from this screen, if any. */
  disabledFile: string | null
  /** Fixes already applied from this screen, by key. */
  applied: Record<string, boolean>
  currentMb: number
  raisedMb: number
  totalMb: number
  /** The instance runs a Java the player chose. */
  customJava: boolean
}

/** The ways to fix a crash, best first. */
export function fixesFor(d: Diagnosis, ctx: FixContext): Fix[] {
  const out: Fix[] = []
  const n = ctx.name
  const { applied, lastWorld } = ctx
  const repair = (first: boolean): Fix => ({
    key: 'repair',
    id: 'repair',
    title: [first ? t('Repair {n}', { n }) : t('If it still crashes')],
    text: [t('Downloads the game files again and checks them. Keeps your worlds, mods and settings.')],
    undoable: false,
    undo: t('Nothing to undo'),
  })
  const settings = (text: Part[]): Fix => ({
    key: 'settings',
    id: 'settings',
    title: [t('Check the instance settings')],
    text,
    undoable: false,
    undo: t('Nothing changes on its own'),
  })
  const updates = (parties: (Party | null)[]): Fix => {
    const list = parties.filter((p): p is Party => !!p)
    const names: Part[] = list.flatMap((p, i) => [...(i ? [t(' and ')] : []), ...who(p, t('a mod'))])
    return {
      key: 'updates',
      id: 'updates',
      title: list.length ? [t('Look for updates of '), ...names] : [t('Look at your recent mods')],
      text: [
        list.length
          ? list.length > 1
            ? t('A newer version of one of them often fixes this. Opens the mods of {name}.', { name: n })
            : t('A newer version of it often fixes this. Opens the mods of {name}.', { name: n })
          : t('A mod you added or updated lately may be the cause. Opens the mods of {n}.', { n }),
      ],
      undoable: false,
      undo: t('Nothing changes until you update'),
    }
  }
  const disable = (p: Party | null) => {
    if (!p?.mod || LIBRARIES.test(p.mod.name ?? p.mod.fileName)) return
    if (!p.mod.enabled && ctx.disabledFile !== p.mod.fileName) return
    const m = who(p, t('the mod'))
    out.push({
      key: 'disable',
      id: 'disable',
      party: p,
      title: [t('Play without '), ...m, t(' for now')],
      text: lastWorld
        ? [t('Cubby can back up {name} first. Without ', { name: lastWorld.name }), ...m, t(', its blocks and items are removed from the places you visit; restore the backup to get them back.')]
        : [t('Without '), ...m, t(', its blocks and items are removed from the places you visit.')],
      undoable: true,
      undo: t('Turn it back on any time'),
    })
  }
  const driver = (text: string): Fix => ({ key: 'driver', id: 'driver', title: [t('Update your graphics driver')], text: [text], undoable: false, undo: t('Done outside Cubby') })

  switch (d.kind) {
    case 'memory':
      if (ctx.raisedMb > ctx.currentMb || applied.memory) {
        out.push({
          key: 'memory',
          id: 'memory',
          title: [t('Give {n} more memory', { n })],
          text: [
            applied.memory
              ? t('{n} can now use up to {n2}.', { n, n2: gb(ctx.currentMb) })
              : ctx.totalMb
                ? t('Raises its memory limit from {from} to {to}. Your PC has {total}.', { from: gb(ctx.currentMb), to: gb(ctx.raisedMb), total: gb(ctx.totalMb) })
                : t('Raises its memory limit from {from} to {to}.', { from: gb(ctx.currentMb), to: gb(ctx.raisedMb) }),
          ],
          undoable: true,
          undo: t('Undo any time'),
        })
      } else {
        out.push({
          ...updates([]),
          key: 'memory',
          title: [t('Free some memory')],
          text: [
            ctx.totalMb
              ? t('{name} already uses {used} of your {total}. Close other programs, or turn off some mods or shader packs.', { name: n, used: gb(ctx.currentMb), total: gb(ctx.totalMb) })
              : t('{name} already uses {used}. Close other programs, or turn off some mods or shader packs.', { name: n, used: gb(ctx.currentMb) }),
          ],
        })
      }
      out.push(settings([t('Set the memory of {n} yourself.', { n })]))
      out.push(repair(false))
      break
    case 'java':
      if (ctx.customJava || applied.java) {
        out.push({
          key: 'java',
          id: 'java',
          title: [t('Let Cubby pick Java')],
          text: [applied.java ? t('{n} now uses the Java Cubby picks for the game.', { n }) : t('{n} uses a Java you chose. Cubby can pick the one that fits the game instead.', { n })],
          undoable: true,
          undo: t('Undo any time'),
        })
      }
      out.push(settings(d.java.needed ? [t('Choose Java '), { mono: String(d.java.needed) }, t(' or newer for {n}.', { n })] : [t('Choose another Java for {n}.', { n })]))
      out.push(repair(false))
      break
    case 'missing':
      for (const { dep: p, by, wrong } of d.missing.slice(0, 3)) {
        const req = who(by, t('another mod'))
        if (p.mod && (!p.mod.enabled || applied[`enable:${baseName(p.mod)}`])) {
          out.push({
            key: `enable:${baseName(p.mod)}`,
            id: 'enable',
            party: p,
            title: [t('Turn '), ...who(p, 'it'), t(' back on')],
            text: [t('It’s installed but turned off, and '), ...req, t(' needs it.')],
            undoable: true,
            undo: t('Turn it off again in Mods'),
          })
        } else if (p.mod && wrong) {
          out.push({
            ...updates([p]),
            key: `update:${baseName(p.mod)}`,
            title: [t('Update '), ...who(p, 'it')],
            text: [...who(by, t('Another mod')), t(' needs another version of it. Opens the mods of {n}.', { n })],
          })
        } else {
          out.push({
            key: `find:${p.ref}`,
            id: 'find',
            party: p,
            title: [t('Install '), ...who(p, 'it')],
            text: [...(by ? [...req, t(' needs it. ')] : []), t('Opens Discover with '), { mono: searchTerm(p) }, t(', for {n}.', { n })],
            undoable: false,
            undo: t('Nothing changes until you install'),
          })
        }
      }
      if (d.culprit && !d.missing.some((m) => sameMod(m.dep, d.culprit))) disable(d.culprit)
      break
    case 'conflict':
    case 'version':
    case 'mixin':
      disable(d.culprit)
      out.push(updates(d.kind === 'version' ? [d.culprit] : [d.culprit, d.other]))
      out.push(repair(false))
      break
    case 'driver':
      out.push(driver(t('Get the latest driver from NVIDIA, AMD or Intel, then play again.')))
      out.push(settings([t('Remove custom Java arguments from {n}, if you added some.', { n })]))
      out.push(repair(false))
      break
    case 'jvm-start':
      out.push(settings([t('Look at the memory and the Java arguments of {n}.', { n })]))
      out.push(repair(false))
      break
    case 'jvm-crash':
      out.push(settings([t('Remove custom Java arguments from {n}, if you added some.', { n })]))
      out.push(driver(t('A crash in Java often comes from the driver. Get the latest one from NVIDIA, AMD or Intel.')))
      out.push(repair(false))
      break
    default:
      out.push(repair(true))
      out.push(updates([]))
      out.push({
        key: 'help',
        id: 'help',
        title: [t('Ask for help')],
        text: [t('Copy the report and share it with the mod authors or on a forum. It has the end of the game log.')],
        undoable: false,
        undo: t('Nothing changes'),
      })
  }
  return out
}

// What Cubby checked

export interface Check {
  label: string
  bad: boolean
  word: string
  detail: string | null
}

export function checksFor(d: Diagnosis, facts: SessionFacts, ctx: { memoryMb: number; mods: number; enabledMods: number }): Check[] {
  const s = d.signals
  const found = [...new Set([d.culprit, d.other, ...d.missing.map((n) => n.dep)].filter((p): p is Party => !!p).map(labelOf))]
  const modsBad = s.missing > 0 || s.conflict
  return [
    { label: t('Memory, {n}', { n: gb(ctx.memoryMb) }), bad: s.memory, word: s.memory ? t('Ran out') : 'OK', detail: null },
    { label: facts.java ? t('Java {java}', { java: facts.java }) : t('Java version'), bad: s.java, word: s.java ? t('Wrong version') : 'OK', detail: null },
    { label: t('Graphics driver'), bad: s.driver, word: s.driver ? t('Crashed') : 'OK', detail: null },
    { label: t('Java runtime'), bad: !!s.jvm, word: s.jvm === 'start' ? t('Couldn’t start') : s.jvm ? t('Crashed') : 'OK', detail: null },
    {
      label: ctx.mods ? t('Mods, {enabledMods} on', { enabledMods: ctx.enabledMods }) : t('Mods'),
      bad: modsBad,
      word: s.missing > 0 ? t('{missing} missing', { missing: s.missing }) : s.conflict ? t('1 conflict') : 'OK',
      detail: modsBad && found.length ? t('Found: {mods}', { mods: found.join(t(' and ')) }) : null,
    },
  ]
}
