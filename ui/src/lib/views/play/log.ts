// Reading the game log: parsed lines for LogViewer (Running, Crash), and what the game says about the session.

import type { LogLevel, LogLine as ViewLine } from '../../cubby'
import { fold } from '../../format'
import type { LogLine } from '../../types'

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
