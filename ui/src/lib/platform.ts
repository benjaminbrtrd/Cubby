// The OS the interface runs on, for the few places it differs: the window buttons and the key names.
const ua = typeof navigator === 'undefined' ? '' : `${navigator.platform} ${navigator.userAgent}`

export const isMac = /Mac/.test(ua)

/** A key name as written on this OS's keyboard: Ctrl is ⌘ on a Mac. */
export function keyName(key: string): string {
  if (!isMac) return key
  return ({ Ctrl: '⌘', Alt: '⌥', Shift: '⇧', Enter: '↩' } as Record<string, string>)[key] ?? key
}

/** "Windows", "macOS" or "Linux", for text like "leave memory for Windows". */
export const osName = isMac ? 'macOS' : /Linux|X11/.test(ua) && !/Windows/.test(ua) ? 'Linux' : 'Windows'
