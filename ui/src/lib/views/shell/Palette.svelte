<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { CommandPalette, Icon, type CommandGroup, type CommandItem, type IconName } from '../../cubby'
  import { api } from '../../api'
  import { app } from '../../state.svelte'
  import { fold, loaderFull, plural, relativeTime } from '../../format'
  import type { Instance, ProjectKind, Server } from '../../types'
  import { liveState } from '../library/Library.svelte'

  let { onClose }: { onClose: () => void } = $props()

  interface Action {
    id: string
    label: string
    detail?: string
    icon: IconName
    kbd?: string[]
    /** Extra words the filter matches. */
    words?: string
    run: () => void
  }

  const KINDS: { kind: ProjectKind; label: string; plural: string }[] = [
    { kind: 'mod', label: 'mods', plural: t('Mods') },
    { kind: 'modpack', label: 'modpacks', plural: t('Modpacks') },
    { kind: 'shader', label: t('shader packs'), plural: t('Shader packs') },
    { kind: 'resourcepack', label: t('resource packs'), plural: t('Resource packs') },
  ]
  let query = $state('')
  let activeId = $state<string | undefined>()
  let servers = $state<{ instance: Instance; server: Server }[]>([])
  let root: HTMLElement | undefined = $state()

  // Every instance's servers, read once when the palette opens.
  $effect(() => {
    let stale = false
    const list = app.recent
    void Promise.allSettled(list.map((i) => api.listServers(i.id))).then((results) => {
      if (stale) return
      servers = results.flatMap((r, n) => (r.status === 'fulfilled' ? r.value.map((server) => ({ instance: list[n], server })) : []))
    })
    return () => {
      stale = true
    }
  })

  // Focus goes to the field; it comes back where it was when the palette closes.
  $effect(() => {
    const back = document.activeElement instanceof HTMLElement ? document.activeElement : null
    root?.querySelector<HTMLInputElement>('.cb-cmd__input')?.focus()
    return () => {
      if (back?.isConnected) back.focus()
    }
  })

  let closed = false

  function go(run: () => void) {
    closed = true
    onClose()
    run()
  }

  function discover(kind: ProjectKind, q = '') {
    app.discover.kind = kind
    app.discover.query = q
    app.discover.page = 0
    app.navigate({ name: 'discover' })
  }

  const actions = $derived.by((): Action[] => {
    const tasks = Object.values(app.tasks)
    const known = tasks.filter((t) => t.total > 0)
    const percent = known.length ? Math.round((known.reduce((s, t) => s + t.done / t.total, 0) / known.length) * 100) : null
    return [
      { id: 'a:new', label: t('New instance'), detail: t('Pick a version and a loader'), icon: 'plus', words: 'create add', run: () => app.navigate({ name: 'create' }) },
      {
        id: 'a:import',
        label: t('Import from another launcher'),
        detail: t('Copies instances, the other launcher stays as it is'),
        icon: 'import',
        words: 'import prism multimc curseforge atlauncher gdlauncher modrinth bring',
        run: () => (app.prismImportOpen = true),
      },
      {
        id: 'a:downloads',
        label: t('Open downloads'),
        detail: tasks.length ? t(percent != null ? '{tasks} running · {percent}%' : '{tasks} running', { tasks: plural(tasks.length, 'task', 'tasks'), percent: percent ?? '' }) : t('Nothing running'),
        icon: 'download',
        words: 'downloads queue progress',
        run: () => app.navigate({ name: 'downloads' }),
      },
      ...KINDS.map((k) => ({
        id: `a:discover:${k.kind}`,
        label: t('Discover {label}', { label: k.label }),
        detail: t('Browse Modrinth'),
        icon: 'discover' as const,
        words: 'discover modrinth browse',
        run: () => discover(k.kind),
      })),
      { id: 'a:accounts', label: t('Accounts'), detail: t('Microsoft accounts'), icon: 'accounts', words: 'account sign in skin', run: () => app.navigate({ name: 'accounts' }) },
      {
        id: 'a:settings',
        label: t('Settings'),
        detail: t('Appearance, Java, game window'),
        icon: 'settings',
        words: 'settings preferences options',
        run: () => app.navigate({ name: 'settings', section: 'general' }),
      },
    ]
  })

  const q = $derived(fold(query.trim()))
  const has = (text: string) => fold(text).includes(q)

  function instanceItem(i: Instance): CommandItem {
    const live = liveState(i)
    const meta = i.loader.kind === 'vanilla' ? i.minecraft : `${i.minecraft} · ${loaderFull(i.loader)}`
    const hint = live.running ? t('Show game') : live.task ? t('Show progress') : live.status === 'download' ? t('Download and play') : 'Play'
    return {
      id: `i:${i.id}`,
      label: i.name,
      detail: `${meta} · ${relativeTime(i.lastPlayed)}`,
      icon: live.running ? 'stop' : 'play',
      status: live.status,
      hint,
    }
  }

  const groups = $derived.by((): CommandGroup[] => {
    const out: CommandGroup[] = []
    const instances = q ? app.recent.filter((i) => has(i.name) || has(i.minecraft) || has(loaderFull(i.loader)) || has(i.group ?? '')) : app.recent.slice(0, 5)
    if (instances.length) out.push({ label: t('Instances'), items: instances.map(instanceItem) })

    const joinable = (q ? servers.filter((s) => has(s.server.name) || has(s.server.address) || has(s.instance.name)) : servers.slice(0, 3)).map(
      ({ instance, server }, n): CommandItem => ({
        id: `s:${instance.id}:${n}:${server.address}`,
        label: server.name || server.address,
        detail: t('Join with {name} · {address}', { name: instance.name, address: server.address }),
        icon: 'servers',
        hint: t('Join'),
      }),
    )
    if (joinable.length) out.push({ label: t('Servers'), items: joinable })

    const acts = q ? actions.filter((a) => has(a.label) || has(a.words ?? '')) : actions
    if (acts.length) out.push({ label: t('Actions'), items: acts.map(({ id, label, detail, icon, kbd }) => ({ id, label, detail, icon, kbd })) })

    if (q) {
      const kind = KINDS.find((k) => k.kind === app.discover.kind) ?? KINDS[0]
      out.push({
        label: t('Discover'),
        items: [
          {
            id: 'q:search',
            label: t('Search Modrinth for “{query}”', { query: query.trim() }),
            detail: kind.kind !== 'modpack' && app.target ? `${kind.plural} for ${app.target.name}` : kind.plural,
            icon: 'discover',
            hint: t('Search'),
          },
        ],
      })
    }
    return out
  })

  const flat = $derived(groups.flatMap((g) => g.items))
  const active = $derived(flat.some((it) => it.id === activeId) ? activeId : flat[0]?.id)

  function serverOf(id: string) {
    const [, instanceId, , ...rest] = id.split(':')
    return { instanceId, address: rest.join(':') }
  }

  function select(item: CommandItem, { offline }: { offline: boolean }) {
    if (item.id.startsWith('i:')) {
      const id = item.id.slice(2)
      if (app.running[id]) go(() => app.navigate({ name: 'running', id }))
      else if (app.instanceTask(id)) go(() => app.openInstance(id))
      else go(() => void app.launch(id, offline))
    } else if (item.id.startsWith('s:')) {
      const { instanceId, address } = serverOf(item.id)
      if (app.running[instanceId]) {
        const name = app.instance(instanceId)?.name ?? t('This instance')
        go(() => app.toast(t('{name} is already running. Stop it to join another server.', { name }), 'info'))
      } else go(() => void app.launch(instanceId, offline, address))
    } else if (item.id === 'q:search') {
      const text = query.trim()
      go(() => discover(app.discover.kind, text))
    } else {
      const action = actions.find((a) => a.id === item.id)
      if (action) go(action.run)
    }
  }

  // Tab: the instance page of the active instance or server.
  function more(item: CommandItem) {
    if (item.id.startsWith('i:')) {
      const id = item.id.slice(2)
      go(() => app.openInstance(id))
    } else if (item.id.startsWith('s:')) {
      const { instanceId } = serverOf(item.id)
      go(() => app.openInstance(instanceId, 'servers'))
    }
  }

  // Esc anywhere in the palette closes it; Tab stays inside.
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    } else if (e.key === 'Tab' && root && !closed) {
      const items = [...root.querySelectorAll<HTMLElement>('.cb-cmd__input')]
      if (!items.length) return
      const i = items.indexOf(document.activeElement as HTMLElement)
      const to = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : i < 0 || i === items.length - 1 ? 0 : i + 1
      e.preventDefault()
      items[to].focus()
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" bind:this={root} onclick={(e) => e.target === e.currentTarget && onClose()} {onkeydown}>
  <div class="palette">
    <CommandPalette
      label={t('Search and launch')}
      placeholder={t('Search instances, servers, actions…')}
      {groups}
      bind:query
      activeId={active}
      onActiveChange={(id) => (activeId = id)}
      onSelect={select}
      onMore={more}
      {onClose}
      footer={[]}
      closeKey=""
      modal
      idBase="palette"
    />
    {#if !flat.length}
      <p class="none text-small">{t('Nothing matches “{query}”.', { query: query.trim() })}</p>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: var(--layer-dialog);
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: flex-start;
    align-content: flex-start;
    gap: var(--space-6);
    padding: 88px var(--space-6) var(--space-6);
    overflow: auto;
    background: var(--scrim);
  }
  .palette {
    flex: 0 1 640px;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .none {
    margin: 0;
    padding: var(--space-3) var(--space-4);
    color: var(--ink-muted);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    border-top: 0;
  }
</style>
