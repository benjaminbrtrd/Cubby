// Game log parsing (Running, Crash) and honest crash heuristics on the tail of the log.

import type { LogLevel, LogLine as ViewLine } from '../../cubby'
import { fold } from '../../format'
import type { LogLine, ModFile } from '../../types'

// Log lines
/** A game log line ready for `LogViewer`, with what the filters need. */
export interface ParsedLine extends ViewLine {
  level: LogLevel
  time: string
  text: string
  raw: string
  /** Folded raw text, for search. */
  search: string
}

/**
 * `[18:30:02] [Render thread/INFO]: message` (vanilla log4j, decoded by the backend), with the variants
 * Forge (`[logger/MARKER]:`), Fabric's own format (`(Logger) message`) and a date before the time.
 */
const HEADER =
  /^\[(?:\d{1,2}[A-Za-z]{3}\d{4} )?(\d{1,2}:\d{2}:\d{2})(?:[.,]\d+)?\]\s*\[([^\]]*?)\/(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|SEVERE)\]\s*(?:\[([^\]]*)\]|\(([^)]*)\))?\s*:?\s?(.*)$/
/** A time and nothing else, as some tools print before plain output. */
const TIME_ONLY = /^\[(\d{1,2}:\d{2}:\d{2})(?:[.,]\d+)?\]\s?(.*)$/
/** Part of the record above: a stack frame, an exception, an indented or bulleted line. */
const CONTINUATION = /^(\s|-\s)|^(Caused by|Suppressed):|^\.\.\. \d+ (more|common frames)|^([\w$]+\.)+[\w$]*(Exception|Error|Throwable)\b/
/** `[Sodium] Using renderer` names its source in the message. */
const TAG = /^\[([A-Za-z][\w .-]{0,23})\]\s+(.*)$/

const LEVELS: Record<string, LogLevel> = {
  TRACE: 'debug',
  DEBUG: 'debug',
  INFO: 'info',
  WARN: 'warn',
  WARNING: 'warn',
  ERROR: 'error',
  FATAL: 'error',
  SEVERE: 'error',
}

function hms(iso: string): string {
  const d = new Date(iso)
  if (isNaN(d.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

/** The text of a raw line without its `[time] [thread/LEVEL]:` header. */
export function messageOf(raw: string): string {
  const h = HEADER.exec(raw)
  if (h) return h[6]
  const t = TIME_ONLY.exec(raw)
  return (t ? t[2] : raw).trim()
}

/**
 * Parses the game log incrementally: only new lines are parsed, and lines dropped at the front
 * (the buffer keeps the last 5,000) are dropped from the cache too.
 */
export class LogParser {
  private out: ParsedLine[] = []
  private keys: string[] = []
  private prev: { level: LogLevel; time: string; stream: string; at: number } | null = null
  private seen = 0
  private found: SessionFacts = emptyFacts()

  /** What the game said about the session so far (kept when old lines leave the buffer). */
  get facts(): SessionFacts {
    return { ...this.found }
  }

  parse(lines: LogLine[]): ParsedLine[] {
    if (!lines.length) {
      this.reset()
      return []
    }
    const first = lines[0].time + '\u0000' + lines[0].line
    if (this.keys.length && this.keys[0] !== first) {
      const shift = this.keys.indexOf(first)
      if (shift > 0) {
        this.out.splice(0, shift)
        this.keys.splice(0, shift)
      } else this.reset()
    }
    if (this.out.length > lines.length) this.reset()
    for (let i = this.out.length; i < lines.length; i++) {
      const l = lines[i]
      this.keys.push(l.time + '\u0000' + l.line)
      this.out.push(this.line(l))
    }
    return this.out.slice()
  }

  private reset() {
    this.out = []
    this.keys = []
    this.prev = null
    this.seen = 0
    this.found = emptyFacts()
  }

  private line(l: LogLine): ParsedLine {
    const raw = l.line
    const at = new Date(l.time).getTime() || 0
    const h = HEADER.exec(raw)
    let parsed: ParsedLine
    if (h) {
      let text = h[6]
      const logger = h[4] || h[5]
      let source = logger ? logger.split('/')[0].split('.').pop() || h[2] : h[2]
      const tag = !logger ? TAG.exec(text) : null
      if (tag) {
        source = tag[1]
        text = tag[2]
      }
      parsed = { time: h[1], level: LEVELS[h[3]] ?? 'info', source, text, raw, search: fold(raw) }
    } else {
      const t = TIME_ONLY.exec(raw)
      const text = t ? t[2] : raw
      const p = this.prev
      // A line without a level: stderr counts as a warning, stdout as info, unless it continues a warning
      // or an error (a stack trace, or more lines of the same record, which arrive together on one stream).
      const together = p && p.stream === l.stream && Math.abs(at - p.at) < 50
      const continues = p && (p.level === 'warn' || p.level === 'error') && (CONTINUATION.test(text) || together)
      const level: LogLevel = continues && p ? p.level : l.stream === 'stderr' ? 'warn' : 'info'
      parsed = { time: t?.[1] ?? (continues && p ? p.time : hms(l.time)), level, text: text.replace(/^\t/, '    '), raw, search: fold(raw) }
    }
    this.prev = { level: parsed.level, time: parsed.time, stream: l.stream, at }
    scanFacts(this.found, parsed.text, this.seen++)
    return parsed
  }
}

// Session facts read from the log
export interface SessionFacts {
  world: string | null
  server: string | null
  user: string | null
  java: string | null
  maxMemoryMb: number | null
}

function emptyFacts(): SessionFacts {
  return { world: null, server: null, user: null, java: null, maxMemoryMb: null }
}

/** What the game says about itself in the log. Only what is written there; nothing guessed. */
function scanFacts(facts: SessionFacts, t: string, index: number) {
  if (!facts.user) {
    const m = /Setting user: (\S+)/.exec(t)
    if (m) facts.user = m[1]
  }
  // Later lines win: the player may switch worlds or servers during a session.
  const w = /Preparing level "([^"]+)"|Joined world (.+)$/.exec(t)
  if (w) {
    facts.world = (w[1] ?? w[2]).trim()
    facts.server = null
  }
  const s = /Connecting to ([^\s,]+)(?:, (\d+))?/.exec(t)
  if (s) {
    facts.server = s[2] && s[2] !== '25565' ? `${s[1]}:${s[2]}` : s[1]
    facts.world = null
  }
  if (index >= 400) return
  if (!facts.java && !/requires|needs|or (newer|later|above)|compiled/i.test(t)) {
    const j = /Java is .*?version (\d[\w.+-]*)/.exec(t) ?? /\bJava (?:version )?(\d+(?:\.\d+){0,3})\b/.exec(t)
    if (j) facts.java = j[1]
  }
  if (!facts.maxMemoryMb) {
    const m = /max(?:imum)? memory:? ([\d,]+) ?(MB|MiB|GB|GiB)\b/i.exec(t) ?? /-Xmx(\d+)([mMgG])\b/.exec(t)
    if (m) {
      const n = Number(m[1].replace(/,/g, ''))
      facts.maxMemoryMb = /^g/i.test(m[2]) ? n * 1024 : n
    }
  }
}

// Crash diagnosis
export type CauseKind = 'memory' | 'java' | 'missing' | 'conflict' | 'mixin' | 'version' | 'driver' | 'jvm-start' | 'jvm-crash' | 'unknown'

/** A mod named by the log: its installed file when Cubby found one, else the id or package the log gives. */
export interface Party {
  /** Display name, when the mod is installed or the log names it; else show `ref` (in mono). */
  name: string | null
  /** The Java package or mod id the log names. */
  ref: string
  mod: ModFile | null
}

export interface Evidence {
  text: string
  culprit: boolean
  /** A gap in the log before this line. */
  gap: boolean
}

/** A dependency the loader didn't find (or found in the wrong version), and the mod that needs it. */
export interface Need {
  dep: Party
  by: Party | null
  /** Installed, but not in a version that fits. */
  wrong: boolean
}

export interface Signals {
  memory: boolean
  java: boolean
  missing: number
  conflict: boolean
  driver: boolean
  jvm: 'start' | 'crash' | null
}

export interface Diagnosis {
  kind: CauseKind
  evidence: Evidence[]
  /** The mod the crash points at (the one to turn off). */
  culprit: Party | null
  /** The other side: the mod it clashes with, or a missing mod. */
  other: Party | null
  /** Missing (or wrong version) dependencies, for `missing`. */
  missing: Need[]
  java: { needed: number | null; running: number | null }
  /** Native library named in a JVM crash ("atio6axx.dll"). */
  frame: string | null
  /** The JVM error that stopped Java from starting. */
  jvmError: string | null
  /** For `version` read from the loader: the game versions the mod is made for ("1.21 to 1.21.1"). */
  madeFor: string | null
  signals: Signals
  /** Crash report file named by the game, if any. */
  reportPath: string | null
  /** JVM error report (hs_err_pid*.log), if any. */
  jvmReportPath: string | null
  checkedLines: number
}

const TAIL = 600

const OOM = /java\.lang\.OutOfMemoryError|OutOfMemoryError: (Java heap space|GC overhead|Metaspace)/
const CLASS_VERSION = /class file version (\d+)(?:\.\d+)?\), this version of the Java Runtime only recognizes class file versions up to (\d+)/
const JAVA_WRONG =
  /UnsupportedClassVersionError|compiled by a more recent version of the Java Runtime|requires (?:version )?[^\n]*?\bof (?:mod )?'?java'?\b|requires Java \d+|Java \d+ (?:or (?:newer|later|above)|is required)|Unsupported Java version|wrong version of Java/i
const RESOLUTION =
  /Mod resolution (?:failed|encountered an incompatible mod set)|Incompatible mods? (?:set|found)|Some of your mods are incompatible|which is missing|Missing or unsupported mandatory dependencies|Mod ID: '[^']+', Requested by: '[^']+'|Missing Mods:/i
const LINKAGE = /java\.lang\.(NoSuchMethodError|NoSuchFieldError|AbstractMethodError|IncompatibleClassChangeError|NoClassDefFoundError|ClassNotFoundException)\b/
const MIXIN = /MixinApplyError|InvalidMixinException|InvalidInjectionException|MixinTransformerError|Mixin apply(?:ing)? (?:for|from) mod [\w.-]+ failed|Critical injection failure|mixin\.injection\.throwables/
const JVM_START =
  /Could not create the Java Virtual Machine|Invalid (?:maximum|initial) heap size|Unrecognized VM option|Unrecognized option:|Could not reserve enough space for|There is insufficient memory for the Java Runtime Environment|Error: Could not find or load main class|Error occurred during initialization of VM/
const JVM_FATAL = /A fatal error has been detected by the Java Runtime Environment|EXCEPTION_ACCESS_VIOLATION|SIGSEGV|EXCEPTION_STACK_OVERFLOW/
const GL_ERROR =
  /Pixel format not accelerated|GLFW error 65542|WGL: The driver does not appear to support OpenGL|Couldn't set pixel format|Failed to create (?:the )?OpenGL context|OpenGL [\d.]+ (?:is )?(?:not supported|required)/i
const DRIVER_LIB = /^(atio6axx|atioglxx|atiumd|amdxc|amdvlk|nvoglv|nvd3dum|nvwgf|ig\d*icd|igxelpicd|igc\d*|ig9icd|libGL|opengl32|lwjgl)/i

/** Packages that are the game, the loader or a library: they never name the mod at fault. */
const IGNORED =
  /^(java|javax|jdk|sun|com\.sun|net\.minecraft|com\.mojang|org\.spongepowered|com\.llamalad7|net\.fabricmc\.(loader|api|impl|tinyremapper|accesswidener|mappingio)|org\.quiltmc\.loader|cpw\.mods|net\.minecraftforge\.(fml|eventbus|bus)|net\.neoforged\.(fml|bus|neoforgespi)|org\.lwjgl|org\.objectweb|com\.google|io\.netty|org\.apache|org\.slf4j|it\.unimi|oshi|kotlin|kotlinx|scala|org\.jetbrains|org\.joml|com\.electronwill|org\.yaml|com\.fasterxml|ca\.weblite)(\.|$)/
const GAME = /^(net\.minecraft|com\.mojang)(\.|$)/
const STOP = new Set(['com', 'net', 'org', 'me', 'dev', 'io', 'de', 'fr', 'uk', 'co', 'xyz', 'github', 'gitlab', 'mod', 'mods', 'client', 'common', 'api', 'impl', 'core', 'lib', 'util', 'utils', 'main', 'render', 'mixin', 'mixins', 'minecraft', 'world', 'compat', 'jar', 'fabric', 'forge', 'neoforge', 'quilt'])

const FQCN = /\b((?:[a-z_][a-z0-9_]*[./]){2,}[A-Z][\w$]*)/g

function packageOf(className: string): string {
  const parts = className.replace(/\//g, '.').split('.')
  return parts.slice(0, -1).join('.')
}

function norm(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]/g, '')
}

/** Tokens a mod is known by: its name, the first word of its name, the start of its file name. */
function modTokens(m: ModFile): { token: string; score: number }[] {
  const base = m.fileName.toLowerCase().replace(/\.disabled$/, '').replace(/\.jar$/, '')
  const fileParts = base.split(/[-_+ ]/).filter(Boolean)
  const out: { token: string; score: number }[] = []
  if (m.name) {
    out.push({ token: norm(m.name), score: 4 })
    const word = norm(m.name.split(/\s+/)[0] ?? '')
    if (word.length >= 4) out.push({ token: word, score: 2 })
  }
  if (fileParts[0]) out.push({ token: norm(fileParts[0]), score: 3 })
  if (fileParts.length > 1) out.push({ token: norm(fileParts[0] + fileParts[1]), score: 3 })
  return out.filter((t) => t.token.length >= 3)
}

/** The installed mod a mod id ("sodium", "fabric-api") or a mod name stands for. */
function findById(id: string, mods: ModFile[]): ModFile | null {
  const key = norm(id)
  if (!key) return null
  let best: { mod: ModFile; score: number } | null = null
  for (const m of mods) {
    for (const t of modTokens(m)) {
      if (t.token === key && (!best || t.score > best.score)) best = { mod: m, score: t.score }
    }
  }
  return best?.mod ?? null
}

/** The installed mod a Java package belongs to, matched segment by segment. */
function findByPackage(pkg: string, mods: ModFile[]): { mod: ModFile; root: string } | null {
  const segs = pkg.split('.')
  if (pkg.startsWith('net.fabricmc.fabric')) {
    const api = mods.find((m) => /fabric[- ]?api/i.test(m.name ?? m.fileName) || /^fabric-api/i.test(m.fileName))
    if (api) return { mod: api, root: 'net.fabricmc.fabric' }
  }
  for (let i = 0; i < segs.length; i++) {
    const s = segs[i].toLowerCase()
    if (STOP.has(s) || s.length < 3) continue
    const bare = s.replace(/\d+$/, '')
    let best: { mod: ModFile; score: number } | null = null
    for (const m of mods) {
      for (const t of modTokens(m)) {
        // Exact, then without a trailing number ("clothconfig2"), then nearly the same ("xaero", "xaeros").
        const score =
          t.token === s || t.token === bare
            ? t.score
            : bare.length >= 5 && Math.abs(t.token.length - bare.length) <= 2 && (t.token.startsWith(bare) || bare.startsWith(t.token))
              ? 1
              : 0
        if (score && (!best || score > best.score)) best = { mod: m, score }
      }
    }
    if (best) return { mod: best.mod, root: segs.slice(0, i + 1).join('.') }
  }
  return null
}

/**
 * The versions in a loader requirement, said simply: "any version between 1.21 (inclusive) and 1.21.1
 * (inclusive)" gives "1.21 to 1.21.1", "version 1.20.1 or later" gives "1.20.1 or later", "1.21.1" stays.
 */
export function gameRange(req: string): string | null {
  const v = '(\\d[\\w.+-]*\\w)'
  const between = new RegExp(`between ${v}[^\\d]*?and ${v}`, 'i').exec(req)
  if (between) return `${between[1]} to ${between[2]}`
  const later = new RegExp(`${v}\\s*(?:\\(inclusive\\))?\\s*or (?:later|newer|above)|>=\\s*${v}`, 'i').exec(req)
  if (later) return `${later[1] ?? later[2]} or later`
  const one = new RegExp(v).exec(req)
  return one ? one[1] : null
}

function rootOf(pkg: string): string {
  return pkg.split('.').slice(0, 3).join('.')
}

function partyFromPackage(pkg: string, mods: ModFile[]): Party {
  const found = findByPackage(pkg, mods)
  if (found) return { name: found.mod.name, ref: found.root, mod: found.mod }
  return { name: null, ref: rootOf(pkg), mod: null }
}

function partyFromId(id: string, name: string | null, mods: ModFile[]): Party {
  const mod = findById(id, mods) ?? (name ? findById(name, mods) : null)
  return { name: mod?.name ?? name, ref: id, mod }
}

function sameParty(a: Party | null, b: Party | null): boolean {
  if (!a || !b) return false
  if (a.mod && b.mod) return a.mod.fileName === b.mod.fileName
  return a.ref === b.ref
}

function isFrame(line: string): boolean {
  return /^\s*at /.test(messageOf(line)) || /^\s+at /.test(line)
}

/** First stack frame after `from` whose class is not the game, the loader or a library. */
function firstModFrame(tail: string[], from: number, skipPkg: string | null): { index: number; pkg: string } | null {
  for (let i = from + 1; i < Math.min(tail.length, from + 80); i++) {
    // "at com.x.Y.m(", "at knot//com.x.Y.m(", "at TRANSFORMER/create@0.5.1/com.x.Y.m("
    const m = /^\s*at (?:[^\s/]+\/)*(?:[^\s/]*\/\/)?([\w$.]+)\.[\w$<>]+\(/.exec(messageOf(tail[i]))
    if (!m) {
      if (i > from + 2 && !/^\s*(Caused by|\.\.\.)/.test(messageOf(tail[i]))) break
      continue
    }
    const pkg = packageOf(m[1])
    if (!pkg || IGNORED.test(pkg)) continue
    if (skipPkg && pkg.startsWith(skipPkg)) continue
    return { index: i, pkg }
  }
  return null
}

function evidence(tail: string[], indices: number[], culprit: number): Evidence[] {
  const sorted = [...new Set(indices.filter((i) => i >= 0 && i < tail.length))].sort((a, b) => a - b).slice(0, 5)
  return sorted.map((i, n) => ({ text: messageOf(tail[i]), culprit: i === culprit, gap: n > 0 && i - sorted[n - 1] > 1 }))
}

function findLast(tail: string[], re: RegExp): number {
  for (let i = tail.length - 1; i >= 0; i--) if (re.test(tail[i])) return i
  return -1
}

function findFirst(tail: string[], re: RegExp, from = 0): number {
  for (let i = from; i < tail.length; i++) if (re.test(tail[i])) return i
  return -1
}

/**
 * Looks at the end of the game log for the usual reasons a game stops: memory, Java version, mods that
 * fail to load, two mods that disagree, the graphics driver or Java itself. Says `unknown` rather than guess.
 */
export function diagnose(lines: string[], mods: ModFile[], exitCode: number | null = null): Diagnosis {
  const tail = lines.slice(-TAIL)
  const text = tail.join('\n')

  const reportPath = /Crash report saved to:?\s*(?:#@!@#\s*)?(\S.*?\.txt)/.exec(text)?.[1]?.trim() ?? null
  const jvmReportPath = /error report file with more information is saved as:\s*#?\s*(\S+\.log)/.exec(text)?.[1] ?? null
  const frameMatch = /Problematic frame:[\s#]*[CVjJv]\s+\[([^\]+\s]+)/.exec(text)
  const frame = frameMatch?.[1] ?? null

  const signals: Signals = {
    memory: OOM.test(text),
    java: JAVA_WRONG.test(text),
    missing: 0,
    conflict: false,
    driver: GL_ERROR.test(text) || (!!frame && DRIVER_LIB.test(frame)),
    jvm: JVM_START.test(text) ? 'start' : JVM_FATAL.test(text) ? 'crash' : null,
  }

  const base: Diagnosis = {
    kind: 'unknown',
    evidence: [],
    culprit: null,
    other: null,
    missing: [],
    java: { needed: null, running: null },
    frame,
    jvmError: null,
    madeFor: null,
    signals,
    reportPath,
    jvmReportPath,
    checkedLines: tail.length,
  }

  // Missing dependencies, read from the loader's own listing.
  const missing: Need[] = []
  const need = (dep: Party, by: Party | null, wrong: boolean) => {
    const known = missing.find((n) => n.dep.ref === dep.ref)
    if (!known) missing.push({ dep, by, wrong })
    else if (!known.by) known.by = by
  }
  // Mods the loader says are made for another version of Minecraft, with the versions they accept.
  const wrongGame: { mod: Party; madeFor: string | null; at: number }[] = []
  let resolutionAt = -1
  if (RESOLUTION.test(text)) {
    resolutionAt = findFirst(tail, RESOLUTION)
    for (let i = Math.max(0, resolutionAt - 2); i < tail.length; i++) {
      const msg = messageOf(tail[i])
      const fab = /Mod '([^']+)' \(([\w.-]+)\)\s*\S*\s+requires\s+(.*?)\bof\s+(?:mod\s+)?(?:'([^']+)'\s*\(([\w.-]+)\)|'?([\w.-]+)'?)/i.exec(msg)
      if (fab) {
        const req = partyFromId(fab[2], fab[1], mods)
        const depId = fab[5] ?? fab[6]
        if (depId === 'minecraft') {
          if (!wrongGame.some((w) => sameParty(w.mod, req))) wrongGame.push({ mod: req, madeFor: gameRange(fab[3]), at: i })
        } else if (depId && depId !== 'java') need(partyFromId(depId, fab[4] ?? null, mods), req, !/missing/i.test(msg))
        continue
      }
      const install = /^\s*-?\s*Install ([\w.-]+),/.exec(msg)
      if (install) {
        need(partyFromId(install[1], null, mods), null, false)
        continue
      }
      const forge = /Mod ID: '([\w.-]+)', Requested by: '([\w.-]+)'/.exec(msg)
      if (forge && forge[1] !== 'minecraft' && forge[1] !== 'forge' && forge[1] !== 'neoforge') {
        need(partyFromId(forge[1], null, mods), partyFromId(forge[2], null, mods), !/\[MISSING\]/.test(msg))
      }
    }
    signals.missing = missing.length
  }

  // Two mods that disagree: a missing method or class between them.
  const linkAt = findFirst(tail, LINKAGE)
  let link: { culprit: Party; other: Party | null; error: number; frame: number; missingClass: boolean; game: boolean } | null = null
  if (linkAt >= 0) {
    const msg = messageOf(tail[linkAt])
    const kind = LINKAGE.exec(msg)?.[1] ?? ''
    const named = [...msg.matchAll(FQCN)].map((m) => packageOf(m[1])).filter((p) => p && !IGNORED.test(p))
    // Only the game's own names ("net.minecraft...", "class_761", "field_1234"): the mod is made for another version.
    const gameOnly = !named.length && ([...msg.matchAll(FQCN)].some((m) => GAME.test(packageOf(m[1]))) || /\b(class|method|field|comp)_\d+\b/.test(msg))
    const ownerPkg = named[named.length - 1] ?? null
    const ownerParty = ownerPkg ? partyFromPackage(ownerPkg, mods) : null
    const callerFrame = firstModFrame(tail, linkAt, ownerParty?.ref ?? null)
    if (callerFrame) {
      const caller = partyFromPackage(callerFrame.pkg, mods)
      // A class nobody provides (or a mod that is turned off): a missing dependency, not a clash.
      const missingClass = /NoClassDefFoundError|ClassNotFoundException/.test(kind) && !!ownerParty && (!ownerParty.mod || !ownerParty.mod.enabled)
      if (!sameParty(caller, ownerParty)) {
        link = { culprit: caller, other: ownerParty, error: linkAt, frame: callerFrame.index, missingClass, game: gameOnly }
        signals.conflict = !missingClass
        if (missingClass) signals.missing = Math.max(signals.missing, 1)
      }
    }
  }

  // A mixin (a change a mod makes to the game when it starts) that failed.
  const mixinAt = findFirst(tail, MIXIN)
  let mixin: { culprit: Party; other: Party | null; at: number } | null = null
  if (mixinAt >= 0) {
    let culprit: Party | null = null
    for (let i = Math.max(0, mixinAt - 1); i < Math.min(tail.length, mixinAt + 40) && !culprit; i++) {
      const msg = messageOf(tail[i])
      const byMod = /(?:from|for) mod ([\w.-]+)/.exec(msg)
      // "create.mixins.json", "sodium-fabric.mixins.json", "mixins.iris.json"
      const byConfig = /(?:^|[\s[:'"(])(?:mixins\.([\w-]+)\.json|([\w-]+?)(?:[._-](?:client|common|fabric|forge|neoforge|quilt))?\.mixins?\.json)/.exec(msg)
      const id = byMod?.[1] ?? byConfig?.[1] ?? byConfig?.[2]
      if (id) culprit = partyFromId(id, null, mods)
    }
    if (!culprit) {
      const frameAt = firstModFrame(tail, mixinAt, null)
      if (frameAt) culprit = partyFromPackage(frameAt.pkg, mods)
    }
    if (culprit) {
      // The other side: a class of another installed mod that the failed change was aimed at.
      let other: Party | null = null
      for (let i = mixinAt; i < Math.min(tail.length, mixinAt + 12) && !other; i++) {
        for (const m of messageOf(tail[i]).matchAll(FQCN)) {
          const pkg = packageOf(m[1])
          if (!pkg || IGNORED.test(pkg)) continue
          const p = partyFromPackage(pkg, mods)
          if (p.mod && !sameParty(p, culprit) && p.mod.fileName !== culprit.mod?.fileName) {
            other = p
            break
          }
        }
      }
      mixin = { culprit, other, at: mixinAt }
      signals.conflict = true
    }
  }

  // Ranked: the first cause found is the one shown. Memory and Java are decisive when present.
  if (signals.memory) {
    const at = findFirst(tail, OOM)
    return { ...base, kind: 'memory', evidence: evidence(tail, [at - 1, at, at + 1, at + 2], at) }
  }

  if (signals.java) {
    const at = findFirst(tail, JAVA_WRONG)
    const cv = CLASS_VERSION.exec(text)
    let needed = cv ? Number(cv[1]) - 44 : null
    const running = cv ? Number(cv[2]) - 44 : Number(/present: (\d+)/.exec(text)?.[1]) || null
    if (!needed) {
      const n = /(?:requires|needs) (?:Java |version )?(?:>=\s*)?(\d+)(?:\.\d+)*(?: or (?:later|newer|above))?(?: of (?:mod )?'?java'?)?/i.exec(messageOf(tail[at]))
      needed = n && Number(n[1]) >= 8 && Number(n[1]) < 60 ? Number(n[1]) : null
    }
    return { ...base, kind: 'java', java: { needed, running }, evidence: evidence(tail, [at - 1, at, at + 1], at) }
  }

  if (missing.length && resolutionAt >= 0) {
    const listAt = findFirst(tail, /requires|Mod ID: '|Install [\w.-]+,/, resolutionAt)
    const culpritLine = listAt >= 0 ? listAt : resolutionAt
    const extra = listAt >= 0 ? [listAt + 1, listAt + 2] : [resolutionAt + 1]
    return {
      ...base,
      kind: 'missing',
      missing,
      culprit: missing.find((n) => n.by)?.by ?? null,
      other: missing[0].dep,
      evidence: evidence(tail, [resolutionAt, culpritLine, ...extra], culpritLine),
    }
  }

  // A mod made for another game version: the loader stops before anything else can go wrong.
  if (wrongGame.length) {
    const first = wrongGame[0]
    return {
      ...base,
      kind: 'version',
      culprit: first.mod,
      madeFor: first.madeFor,
      evidence: evidence(tail, [resolutionAt, ...wrongGame.map((w) => w.at)], first.at),
    }
  }

  if (link?.missingClass && link.other) {
    return {
      ...base,
      kind: 'missing',
      missing: [{ dep: link.other, by: link.culprit, wrong: false }],
      culprit: link.culprit,
      other: link.other,
      evidence: evidence(tail, [link.error - 1, link.error, link.frame, link.frame + 1], link.frame),
    }
  }

  if (link) {
    return {
      ...base,
      kind: link.game ? 'version' : 'conflict',
      culprit: link.culprit,
      other: link.other,
      evidence: evidence(tail, [link.error, link.frame, link.frame + 1, link.error + 1].filter((i) => i === link!.error || isFrame(tail[i] ?? '')), link.frame),
    }
  }

  if (mixin) {
    return { ...base, kind: 'mixin', culprit: mixin.culprit, other: mixin.other, evidence: evidence(tail, [mixin.at - 1, mixin.at, mixin.at + 1, mixin.at + 2], mixin.at) }
  }

  if (signals.driver) {
    const at = frame ? findFirst(tail, /Problematic frame/) : findFirst(tail, GL_ERROR)
    const shown = frame ? [findFirst(tail, JVM_FATAL), at, at + 1] : [at - 1, at, at + 1]
    return { ...base, kind: 'driver', evidence: evidence(tail, shown, frame ? at + 1 : at) }
  }

  if (signals.jvm === 'start') {
    const at = findFirst(tail, JVM_START)
    return { ...base, kind: 'jvm-start', jvmError: messageOf(tail[at]), evidence: evidence(tail, [at - 1, at, at + 1], at) }
  }

  if (signals.jvm === 'crash' || exitCode === -1073741819) {
    const at = findFirst(tail, JVM_FATAL)
    const pf = findFirst(tail, /Problematic frame/)
    return { ...base, kind: 'jvm-crash', signals: { ...signals, jvm: 'crash' }, evidence: at >= 0 ? evidence(tail, [at, at + 1, pf, pf + 1], pf >= 0 ? pf + 1 : at) : [] }
  }

  // Nothing known: show the last errors the game wrote.
  const lastError = findLast(tail, /\/(ERROR|FATAL)\]|Exception|Error:|^\s*at /)
  if (lastError >= 0) {
    const start = findLast(tail.slice(0, lastError + 1), /\/(ERROR|FATAL)\]|Exception|Error:/)
    const first = start >= 0 ? start : lastError
    // The error and the lines that belong to it (its exception and first frames), not what came after.
    const shown = [first]
    for (let i = first + 1; i < tail.length && shown.length < 4; i++) {
      const msg = messageOf(tail[i])
      if (!/^(at |Caused by|\.\.\. \d+)|^([\w$]+\.)+[\w$]*(Exception|Error|Throwable)\b/.test(msg)) break
      shown.push(i)
    }
    return { ...base, evidence: evidence(tail, shown, first) }
  }
  return base
}
