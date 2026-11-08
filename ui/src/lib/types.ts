// Data exchanged with the Rust side (src-tauri/src/dto.rs). Keep both files in sync.

export type LoaderKind = 'vanilla' | 'fabric' | 'quilt' | 'neoforge' | 'forge'

export interface Loader {
  kind: LoaderKind
  version?: string
}

export interface Instance {
  id: string
  name: string
  group: string | null
  /** Built-in block icon key ("grass", "copper"...) or an image URL. */
  icon: string | null
  minecraft: string
  loader: Loader
  created: string
  lastPlayed: string | null
  totalPlayedSecs: number
  /** Whether the version files are already downloaded. */
  installed: boolean
  javaOverrides: JavaOverrides
  /** Free text the player keeps about this instance (Overview › Notes). */
  notes: string | null
  /** Modrinth modpack this instance was installed from, if any. */
  modpack: { projectId: string; versionId: string; title: string; versionNumber: string } | null
  /** Files of a modpack still to download by hand (0 normally). */
  blockedFiles: number
  /** The memory "Set memory automatically" gives this instance (MB), from its mods and the computer's memory. */
  recommendedMemoryMb: number
}

export interface JavaOverrides {
  executable: string | null
  minMemoryMb: number | null
  maxMemoryMb: number | null
  jvmArgs: string | null
}

export interface NewInstance {
  name: string
  group: string | null
  icon: string | null
  minecraft: string
  loader: Loader
}

export interface Account {
  id: string
  username: string
  kind: 'microsoft'
  isDefault: boolean
  skinUrl: string | null
}

/** The skin an account wears and the capes it owns (from the Minecraft services). */
export interface SkinProfile {
  skinUrl: string | null
  variant: 'classic' | 'slim'
  capes: CapeInfo[]
}

export interface CapeInfo {
  id: string
  name: string
  url: string
  /** The cape being worn. */
  active: boolean
}

/** A skin of the game, with the PNG of each arm model (bytes). */
export interface DefaultSkin {
  id: string
  name: string
  wide: number[]
  slim: number[]
}

export interface DeviceCode {
  userCode: string
  verificationUri: string
  expiresIn: number
  interval: number
  message: string
}

export interface GameVersion {
  id: string
  kind: 'release' | 'snapshot' | 'old_beta' | 'old_alpha'
  releaseTime: string
}

export interface LoaderVersion {
  version: string
  stable: boolean
}

export interface ModFile {
  fileName: string
  enabled: boolean
  size: number
  name: string | null
  version: string | null
  description: string | null
  authors: string[]
}

export interface World {
  folder: string
  name: string
  lastPlayed: string | null
  sizeBytes: number
  iconUrl: string | null
}

export interface Screenshot {
  fileName: string
  url: string
  taken: string
}

export interface LogLine {
  line: string
  stream: 'stdout' | 'stderr'
  time: string
}

export interface JavaInstall {
  executable: string
  major: number
  version: string
  arch: string
  vendor: string | null
  managed: boolean
}

/** A newer version of Cubby, found on GitHub. */
export interface AppUpdate {
  version: string
  currentVersion: string
  /** Release notes (Markdown). */
  notes: string
  date: string | null
  /** The installer is downloaded and waiting. */
  downloaded: boolean
}

export interface Settings {
  language: string
  /** 'night' | 'day' | 'contrast' | 'system' (older values: 'dark' = night, 'light' = day) */
  theme: string
  /** Replace Rubik and JetBrains Mono with Atkinson Hyperlegible. */
  hyperlegible: boolean
  reduceMotion: boolean
  /** Check installed mods for updates on Modrinth when an instance is opened. */
  checkUpdates: boolean
  accent: string
  frameMode: 'instance' | 'fixed' | 'none'
  frameColor: string
  /** autoMemory: each instance gets `recommendedMemoryMb` instead of `maxMemoryMb`. */
  java: { auto: boolean; autoMemory: boolean; minMemoryMb: number; maxMemoryMb: number; jvmArgs: string }
  game: { width: number; height: number; fullscreen: boolean }
  onLaunch: 'minimize' | 'keep' | 'close'
  downloadConcurrency: number
  msaClientId: string | null
  /** Add play sessions to the instances' play time. */
  trackPlayTime: boolean
  /** Show the play time on the library cards. */
  showPlayTime: boolean
  /** Applied at the next start of Cubby. */
  proxy: { mode: 'system' | 'none' | 'manual'; url: string }
  /** Updates of Cubby itself. */
  updates: { mode: 'ask' | 'auto' | 'off'; channel: 'stable' | 'preview'; skipped: string | null }
  /** User's own CurseForge API key, if any. */
  curseforgeApiKey: string | null
  /** The first-launch assistant was completed or skipped. */
  onboarded: boolean
}

export interface PrismInstance {
  path: string
  name: string
  minecraft: string
  loader: Loader
  group: string | null
}

export interface RunningGame {
  instanceId: string
  startedAt: string
}

export interface AppInfo {
  version: string
  dataDir: string
  msaConfigured: boolean
  /** Where the Microsoft client id comes from; null when there is none. */
  msaSource: 'settings' | 'environment' | 'builtIn' | null
  /** Where the CurseForge API key comes from; null when there is none. */
  curseforgeKey: 'settings' | 'environment' | 'builtIn' | null
  totalMemoryMb: number
  /** Development build: without an account the full game starts offline instead of the demo. */
  devBuild: boolean
}

export type CubbyEvent =
  | { type: 'progress'; task: string; label: string; done: number; total: number }
  | { type: 'taskFinished'; task: string; error: string | null }
  | { type: 'gameLog'; instanceId: string; line: string; stream: 'stdout' | 'stderr' }
  | { type: 'gameState'; instanceId: string; running: boolean; exitCode: number | null }

export type PackKind = 'resource' | 'shader'

/** A resource pack or shader pack of an instance (zip file or folder). */
export interface Pack {
  fileName: string
  name: string
  isDir: boolean
  size: number
  modified: string | null
  /** Resource packs: active in options.txt. Shader packs: the selected shader. */
  enabled: boolean
}

/** An entry of an instance's servers.dat (the game's Multiplayer list). */
export interface Server {
  name: string
  address: string
  /** PNG icon stored by the game, as a data: URL. */
  iconUrl: string | null
  /** Server resource packs: true = accept, false = refuse, null = ask. */
  acceptTextures: boolean | null
}

/** Result of a Server List Ping. */
export interface ServerStatus {
  online: boolean
  motd: string
  playersOnline: number
  playersMax: number
  version: string | null
  pingMs: number | null
  iconUrl: string | null
  /** Whether the server takes the Minecraft version of the instance it was pinged for; null when unknown. */
  compatible?: boolean | null
}

// Discover (Modrinth)
export type ProjectKind = 'mod' | 'modpack' | 'shader' | 'resourcepack'
export type ProjectSort = 'relevance' | 'downloads' | 'follows' | 'newest' | 'updated'
/** Where a project runs, from Modrinth's client_side / server_side. */
export type SideSupport = 'required' | 'optional' | 'unsupported' | 'unknown'

export interface ProjectQuery {
  query: string
  kind: ProjectKind
  /** Only projects with a version for this game version and loader (the target instance). */
  gameVersion: string | null
  loader: LoaderKind | null
  /** Modrinth category slugs ("adventure", "optimization"...). All must match. */
  categories: string[]
  /** 'client' = has client support, 'server' = has server support. */
  environment: 'client' | 'server' | null
  sort: ProjectSort
  offset: number
  limit: number
}

/** A search result. */
export interface ProjectHit {
  id: string
  slug: string
  kind: ProjectKind
  title: string
  author: string
  summary: string
  iconUrl: string | null
  categories: string[]
  downloads: number
  follows: number
  /** ISO dates */
  updated: string
  published: string
  gameVersions: string[]
  loaders: string[]
  clientSide: SideSupport
  serverSide: SideSupport
  license: string | null
  /** First gallery image, for cards. */
  featuredImage: string | null
}

export interface SearchResults {
  hits: ProjectHit[]
  total: number
  offset: number
  limit: number
}

export interface GalleryImage {
  url: string
  title: string | null
  description: string | null
  featured: boolean
}

/** A full project page. */
export interface Project extends ProjectHit {
  /** Long description, Markdown (may contain HTML: sanitize before rendering). */
  body: string
  gallery: GalleryImage[]
  links: { source: string | null; issues: string | null; wiki: string | null; discord: string | null; donation: { platform: string; url: string }[] }
  /** Number of versions. */
  versionCount: number
}

export type DependencyKind = 'required' | 'optional' | 'incompatible' | 'embedded'

export interface Dependency {
  projectId: string | null
  versionId: string | null
  kind: DependencyKind
  /** Filled by the backend when the project is known (one batched lookup). */
  title: string | null
  slug: string | null
  iconUrl: string | null
}

export interface VersionFile {
  url: string
  fileName: string
  size: number
  primary: boolean
  sha1: string
}

export interface ProjectVersion {
  id: string
  projectId: string
  name: string
  versionNumber: string
  channel: 'release' | 'beta' | 'alpha'
  gameVersions: string[]
  loaders: string[]
  published: string
  downloads: number
  /** Markdown */
  changelog: string
  files: VersionFile[]
  dependencies: Dependency[]
}

/** What installing a version into an instance would do, computed before installing. */
export interface InstallPlan {
  instanceId: string
  /** The requested project first, then the dependencies it pulls in. */
  items: InstallItem[]
  /** Required dependencies with no version for this instance's game version and loader. */
  missing: { projectId: string; title: string | null }[]
  /** Projects already in the instance that the requested version or one of its dependencies declares incompatible. */
  conflicts: { projectId: string; title: string | null; fileName: string }[]
  /** Total bytes to download (items that are not already installed). */
  downloadBytes: number
}

/** What a Modrinth version id stands for. */
export interface VersionBrief {
  versionId: string
  projectId: string
  slug: string
  title: string
  iconUrl: string | null
  versionNumber: string
  /** "mod", "shader", "resourcepack"... */
  projectType: string
  size: number
  gameVersions: string[]
  loaders: string[]
}

export interface InstallItem {
  projectId: string
  versionId: string
  title: string
  versionNumber: string
  fileName: string
  size: number
  reason: 'requested' | 'dependency'
  /** This project is already in the instance (same or another version). */
  installed: boolean
  /** Folder it goes to: mods, resourcepacks or shaderpacks. */
  folder: 'mods' | 'resourcepacks' | 'shaderpacks'
}

/** A content file of an instance identified on Modrinth (by its SHA-1). */
export interface ContentSource {
  fileName: string
  projectId: string
  versionId: string
  slug: string
  title: string
  iconUrl: string | null
  versionNumber: string
}

export interface ContentUpdate {
  fileName: string
  projectId: string
  title: string
  currentVersion: string
  latestVersionId: string
  latestVersion: string
  /** Markdown changelog of the new version */
  changelog: string
}

// World backups
export interface WorldBackup {
  /** File name in <instance>/backups/ */
  fileName: string
  /** World folder the backup was made from */
  world: string
  created: string
  sizeBytes: number
}

/** A folder of the data directory, shown in Settings › Outils et dossiers. */
export interface DataFolder {
  key: 'instances' | 'libraries' | 'assets' | 'java' | 'meta' | 'logs' | 'trash'
  label: string
  path: string
  sizeBytes: number
}

// Modpack providers (ATLauncher, CurseForge, FTB, Modrinth, Technic) and file imports
export type ProviderId = 'modrinth' | 'curseforge' | 'ftb' | 'atlauncher' | 'technic'
export type PackLoaderKind = 'fabric' | 'quilt' | 'forge' | 'neoforge'
export type PackSort = 'relevance' | 'downloads' | 'updated' | 'newest' | 'name'

export interface PackProvider {
  id: ProviderId
  name: string
  /** false: CurseForge without an API key. */
  available: boolean
  /** Why it is not available, shown to the player. */
  reason: string | null
  /** Sorts the provider really supports (first = default). */
  sorts: PackSort[]
  /** false when a search returns everything at once. */
  paged: boolean
}

export interface PackSummary {
  id: string
  name: string
  summary: string
  authors: string[]
  iconUrl: string | null
  websiteUrl: string | null
  downloads: number | null
  /** RFC 3339 */
  updated: string | null
  minecraft: string[]
  loaders: PackLoaderKind[]
}

export interface PackPage {
  hits: PackSummary[]
  total: number | null
}

export interface PackDetails {
  summary: PackSummary
  /** Plain text paragraphs separated by blank lines, or Markdown. */
  description: string
  descriptionFormat: 'markdown' | 'text'
  links: { label: string; url: string }[]
  /** Image URLs, may be empty. */
  gallery: string[]
  /** Shown above the version picker when the pack can't be installed at all. */
  warning: string | null
}

export interface PackVersion {
  id: string
  name: string
  channel: 'release' | 'beta' | 'alpha'
  minecraft: string | null
  loader: PackLoaderKind | null
  released: string | null
}

/** ATLauncher only. */
export interface OptionalMod {
  name: string
  description: string
  /** Selected by default. */
  selected: boolean
  recommended: boolean
  /** Mods of the same group exclude each other. */
  group: string | null
  depends: string[]
  warning: string | null
}

/** A file the pack's author doesn't let launchers download: the player gets it from its page. */
export interface BlockedFile {
  fileName: string
  pageUrl: string
  /** Relative to the game folder, forward slashes. */
  path: string
  /** Already in place. */
  found: boolean
}

/** What an imported file or URL contains. */
export interface PackFileInfo {
  format: 'mrpack' | 'curseforge'
  name: string
  version: string | null
  minecraft: string
  loader: { kind: PackLoaderKind; version: string } | null
  fileCount: number
  /** CurseForge zip and no API key: it can't be installed, and this says why. */
  blockedReason: string | null
}

/** A datapack of a world (`saves/<world>/datapacks`; turned off ones wait in `datapacks.disabled`). */
export interface Datapack {
  fileName: string
  /** The file name without `.zip`. */
  name: string
  isDir: boolean
  enabled: boolean
  size: number
  modified: string | null
  description: string | null
}

/** An update that can be undone: its old files are kept in <instance>/backups/<name>/. */
export interface UpdateBackup {
  name: string
  created: string
  /** folder: "mods", "resourcepacks" or "shaderpacks"; old: the file before, new: the file the update put in. */
  files: { folder: string; old: string; new: string }[]
}

/** A top-level file or folder of an instance that an export can include. */
export interface ExportCandidate {
  name: string
  isDir: boolean
  size: number
  /** Usually shipped with a pack (config, options.txt...). */
  suggested: boolean
}

export interface ExportRequest {
  format: 'mrpack' | 'curseforge'
  name: string
  version: string
  /** Summary (Modrinth) or author (CurseForge). */
  note: string
  include: string[]
}

export interface ExportResult {
  path: string
  /** Files the launcher that opens it downloads. */
  listed: number
  /** Files stored inside the pack. */
  stored: number
  /** CurseForge without an API key: every mod is stored. */
  withoutKey: boolean
}

/** An instance of another launcher (Prism, MultiMC, CurseForge, ATLauncher, GDLauncher, Modrinth App, Minecraft Launcher). */
export interface ForeignInstance {
  launcher: 'prism' | 'multiMc' | 'polyMc' | 'curseForge' | 'atLauncher' | 'gdLauncher' | 'modrinthApp' | 'official'
  launcherName: string
  /** What importLauncherInstances takes. */
  path: string
  name: string
  /** null when the launcher doesn't say: it can't be imported. */
  minecraft: string | null
  loader: Loader
  group: string | null
  gameDir: string
}
