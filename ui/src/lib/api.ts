// Typed wrappers for the Tauri commands; outside Tauri (browser dev) calls go to dev/mock.ts.

import type {
  Account,
  AppInfo,
  AppUpdate,
  ContentSource,
  BlockedFile,
  ContentUpdate,
  DataFolder,
  InstallPlan,
  Project,
  ProjectQuery,
  ProjectVersion,
  SearchResults,
  WorldBackup,
  DeviceCode,
  Pack,
  PackKind,
  Server,
  ServerStatus,
  CubbyEvent,
  GameVersion,
  Instance,
  JavaInstall,
  LoaderKind,
  LoaderVersion,
  LogLine,
  OptionalMod,
  PackDetails,
  PackFileInfo,
  PackPage,
  PackProvider,
  PackSort,
  PackVersion,
  ProviderId,
  ModFile,
  NewInstance,
  PrismInstance,
  RunningGame,
  Screenshot,
  Settings,
  World,
} from './types'

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

type Mock = typeof import('./dev/mock')
let mockModule: Promise<Mock> | null = null
const mock: () => Promise<Mock> = import.meta.env.DEV
  ? () => (mockModule ??= import('./dev/mock'))
  : () => Promise.reject(new Error('Cubby needs its desktop app to run.'))

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) {
    const { invoke } = await import('@tauri-apps/api/core')
    return invoke<T>(command, args)
  }
  return (await mock()).handle(command, args ?? {}) as Promise<T>
}

export const api = {
  appInfo: () => call<AppInfo>('app_info'),

  listInstances: () => call<Instance[]>('list_instances'),
  createInstance: (spec: NewInstance) => call<Instance>('create_instance', { spec }),
  updateInstance: (instance: Instance) => call<Instance>('update_instance', { instance }),
  duplicateInstance: (id: string, name: string) => call<Instance>('duplicate_instance', { id, name }),
  deleteInstance: (id: string) => call<void>('delete_instance', { id }),
  /** Quick Play: `server` is an address to join right away ("play.example.org:25565"), `world` a
   *  world folder to open (Minecraft 1.20 and newer). */
  launchInstance: (id: string, offline = false, server: string | null = null, world: string | null = null) =>
    call<void>('launch_instance', { id, offline, server, world }),
  /** Re-downloads and checks every file of the instance's version (task `repair:<id>`). */
  repairInstance: (id: string) => call<void>('repair_instance', { id }),
  killInstance: (id: string) => call<void>('kill_instance', { id }),
  runningGames: () => call<RunningGame[]>('running_games'),
  gameLog: (id: string) => call<LogLine[]>('game_log', { id }),
  openFolder: (id: string, sub?: string) => call<void>('open_folder', { id, sub: sub ?? null }),

  listVersions: () => call<GameVersion[]>('list_versions'),
  listLoaderVersions: (kind: LoaderKind, game: string) => call<LoaderVersion[]>('list_loader_versions', { kind, game }),

  listMods: (id: string) => call<ModFile[]>('list_mods', { id }),
  setModEnabled: (id: string, fileName: string, enabled: boolean) => call<void>('set_mod_enabled', { id, fileName, enabled }),
  removeMod: (id: string, fileName: string) => call<void>('remove_mod', { id, fileName }),
  listWorlds: (id: string) => call<World[]>('list_worlds', { id }),
  listScreenshots: (id: string) => call<Screenshot[]>('list_screenshots', { id }),
  deleteScreenshot: (id: string, fileName: string) => call<void>('delete_screenshot', { id, fileName }),

  listPacks: (id: string, kind: PackKind) => call<Pack[]>('list_packs', { id, kind }),
  /** Active resource packs, highest priority first (it wins over the ones below). */
  setResourcePacks: (id: string, enabled: string[]) => call<void>('set_resource_packs', { id, enabled }),
  /** `fileName` null turns shaders off. */
  setShaderPack: (id: string, fileName: string | null) => call<void>('set_shader_pack', { id, fileName }),

  listServers: (id: string) => call<Server[]>('list_servers', { id }),
  /** Replaces the whole servers.dat list, in this order. */
  saveServers: (id: string, servers: Server[]) => call<void>('save_servers', { id, servers }),
  pingServer: (address: string) => call<ServerStatus>('ping_server', { address }),

  dataFolders: () => call<DataFolder[]>('data_folders'),
  openDataFolder: (key: DataFolder['key']) => call<void>('open_data_folder', { key }),
  /** Deletes cached metadata (version manifests, loader profiles). Instances are untouched. */
  clearCache: () => call<void>('clear_cache'),

  listAccounts: () => call<Account[]>('list_accounts'),
  startMicrosoftLogin: () => call<DeviceCode>('start_microsoft_login'),
  finishMicrosoftLogin: () => call<Account>('finish_microsoft_login'),
  cancelMicrosoftLogin: () => call<void>('cancel_microsoft_login'),
  removeAccount: (id: string) => call<void>('remove_account', { id }),
  setDefaultAccount: (id: string) => call<void>('set_default_account', { id }),

  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),
  listJava: () => call<JavaInstall[]>('list_java'),

  // Discover (Modrinth)
  /** Mods, shader packs and resource packs from CurseForge, in the shape of searchProjects. */
  curseforgeSearch: (query: ProjectQuery) => call<SearchResults>('curseforge_search', { query }),
  /** The categories CurseForge sorts mods, shader packs or resource packs into (ids are strings). */
  curseforgeCategories: (kind: string) => call<{ id: string; name: string }[]>('curseforge_categories', { kind }),
  searchProjects: (query: ProjectQuery) => call<SearchResults>('search_projects', { query }),
  getProject: (idOrSlug: string) => call<Project>('get_project', { idOrSlug }),
  /** Newest first. `gameVersion` / `loader` null = every version. */
  listProjectVersions: (idOrSlug: string, gameVersion: string | null, loader: LoaderKind | null) =>
    call<ProjectVersion[]>('list_project_versions', { idOrSlug, gameVersion, loader }),
  /** Resolves dependencies for this instance without changing anything. */
  planInstall: (instanceId: string, versionId: string) => call<InstallPlan>('plan_install', { instanceId, versionId }),
  /** Downloads the plan's items that are not installed yet (task `install:<instanceId>`). Replaces older versions of the same projects. */
  installVersion: (instanceId: string, versionId: string) => call<void>('install_version', { instanceId, versionId }),
  /** Creates a new instance from a Modrinth modpack version (.mrpack), task `modpack:<instanceId>`. Returns the new instance right away. */
  installModpack: (versionId: string, name: string, group: string | null, icon: string | null) =>
    call<Instance>('install_modpack', { versionId, name, group, icon }),
  /** Identifies the instance's files on Modrinth by SHA-1 (mods, resource packs and shader packs). */
  contentSources: (instanceId: string) => call<ContentSource[]>('content_sources', { instanceId }),
  /** Looks for a newer Cubby on the settings' channel; null when up to date. */
  checkUpdate: () => call<AppUpdate | null>('check_update'),
  /** Downloads the update found by checkUpdate (task `self-update`). */
  downloadUpdate: () => call<void>('download_update'),
  /** Installs the downloaded update and restarts Cubby. Fails while a game is running. */
  installUpdate: () => call<void>('install_update'),
  checkUpdates: (instanceId: string) => call<ContentUpdate[]>('check_updates', { instanceId }),
  /** Replaces each file by its latest compatible version (task `update:<instanceId>`). Backs up the old files into <instance>/backups/content-<date>/. */
  applyUpdates: (instanceId: string, fileNames: string[]) => call<void>('apply_updates', { instanceId, fileNames }),

  // Worlds
  backupWorld: (instanceId: string, folder: string) => call<WorldBackup>('backup_world', { instanceId, folder }),
  listBackups: (instanceId: string) => call<WorldBackup[]>('list_backups', { instanceId }),
  /** Replaces the world folder with the backup (the current folder is backed up first). */
  restoreBackup: (instanceId: string, fileName: string) => call<void>('restore_backup', { instanceId, fileName }),
  /** Moves the world folder to the trash. */
  deleteWorld: (instanceId: string, folder: string) => call<void>('delete_world', { instanceId, folder }),

  // Modpack providers and file imports
  /** ATLauncher, CurseForge, FTB, Modrinth, Technic (in that order). */
  packProviders: () => call<PackProvider[]>('pack_providers'),
  /** 20 per page; an empty query lists the featured packs. */
  packSearch: (provider: ProviderId, query: string, sort: PackSort, offset: number) => call<PackPage>('pack_search', { provider, query, sort, offset }),
  packDetails: (provider: ProviderId, id: string) => call<PackDetails>('pack_details', { provider, id }),
  packVersions: (provider: ProviderId, id: string) => call<PackVersion[]>('pack_versions', { provider, id }),
  /** Empty for every provider but ATLauncher. */
  packOptionalMods: (provider: ProviderId, id: string, versionId: string) => call<OptionalMod[]>('pack_optional_mods', { provider, id, versionId }),
  /** Creates the instance and returns it right away; the install runs as task `modpack:<instanceId>`. `optionalMods`: names of the ticked optional mods (installed exactly); null = the pack's default selection. */
  installPack: (provider: ProviderId, packId: string, versionId: string, name: string, group: string | null, icon: string | null, optionalMods: string[] | null) =>
    call<Instance>('install_pack', { provider, packId, versionId, name, group, icon, optionalMods }),
  /** Native file dialog for .mrpack and .zip; null when cancelled. */
  pickPackFile: () => call<string | null>('pick_pack_file'),
  inspectPackFile: (path: string) => call<PackFileInfo>('inspect_pack_file', { path }),
  importPackFile: (path: string, name: string, group: string | null) => call<Instance>('import_pack_file', { path, name, group }),
  importPackUrl: (url: string, name: string, group: string | null) => call<Instance>('import_pack_url', { url, name, group }),
  /** Files the player has to download by hand for this instance. */
  blockedFiles: (instanceId: string) => call<BlockedFile[]>('blocked_files', { instanceId }),
  /** Looks in the Downloads folder, moves matches into place and returns the updated list. */
  findBlockedFiles: (instanceId: string) => call<BlockedFile[]>('find_blocked_files', { instanceId }),

  listPrismInstances: () => call<PrismInstance[]>('list_prism_instances'),
  importPrism: (paths: string[]) => call<Instance[]>('import_prism', { paths }),
  openUrl: (url: string) => call<void>('open_url', { url }),
}

export async function onEvent(handler: (event: CubbyEvent) => void): Promise<() => void> {
  if (inTauri) {
    const { listen } = await import('@tauri-apps/api/event')
    return listen<CubbyEvent>('cubby://event', (e) => handler(e.payload))
  }
  return (await mock()).subscribe(handler)
}

export async function windowAction(action: 'minimize' | 'maximize' | 'close' | 'drag') {
  if (!inTauri) return
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const win = getCurrentWindow()
  if (action === 'minimize') await win.minimize()
  else if (action === 'maximize') await win.toggleMaximize()
  else if (action === 'close') await win.close()
  else await win.startDragging()
}
