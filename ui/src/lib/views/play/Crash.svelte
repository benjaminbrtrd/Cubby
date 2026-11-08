<!-- After a crash: what happened in plain words, the ways to fix it (best first, most can be undone) and,
     on the side, what Cubby checked. The words come from crashText.ts, the diagnosis from crash.ts. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { untrack } from 'svelte'
  import { api } from '../../api'
  import { Banner, Button, Dialog, Icon, LogViewer, PlayButton, Switch } from '../../cubby'
  import { loaderFull, plural, timeOfDay } from '../../format'
  import { app } from '../../state.svelte'
  import type { Instance, ModFile, World } from '../../types'
  import CrashAside from './CrashAside.svelte'
  import Rich from './Rich.svelte'
  import { diagnose, type Party } from './crash'
  import { LogParser } from './log'
  import { after, baseName, causeOf, fixesFor, gb, labelOf, namedParts, plain, searchTerm, who } from './crashText'

  let { id }: { id: string } = $props()

  const instance = $derived(app.instance(id))
  const name = $derived(instance?.name ?? id)
  const crash = $derived(app.crash?.instanceId === id ? app.crash : null)
  const running = $derived(app.running[id] != null)

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
  const enabledMods = $derived(mods.filter((m) => m.enabled).length)
  const lastWorld = $derived([...worlds].sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? ''))[0] ?? null)

  // Memory
  const currentMb = $derived(instance ? app.memoryOf(instance) : (app.settings?.java.maxMemoryMb ?? 4096))
  const totalMb = $derived(app.info?.totalMemoryMb ?? 0)
  /** 2 GB more, keeping 2 GB for Windows and the other programs. */
  const raisedMb = $derived.by(() => {
    const cap = totalMb ? Math.floor((totalMb - 2048) / 512) * 512 : currentMb + 2048
    return Math.min(currentMb + 2048, cap)
  })
  /** The memory the game had when it crashed (the fixes below may change it). */
  const checkedMemoryMb = untrack(() => currentMb)

  const cause = $derived(causeOf(d, { name, memoryMb: checkedMemoryMb, minecraft: instance?.minecraft ?? '', logLines: parsed.length }))
  const named = $derived(namedParts(d))

  // Ways to fix it
  let busy = $state<string | null>(null)
  let applied = $state<Record<string, boolean>>({})
  let memoryBefore = $state<number | null>(null)
  let javaBefore = $state<string | null>(null)
  let disabled = $state<{ label: string; fileName: string; backup: string | null } | null>(null)
  let confirmDisable = $state<Party | null>(null)
  let backupFirst = $state(true)

  const anyApplied = $derived(Object.values(applied).some(Boolean) || !!disabled)
  const fixes = $derived(
    fixesFor(d, {
      name,
      lastWorld,
      disabledFile: disabled?.fileName ?? null,
      applied,
      currentMb,
      raisedMb,
      totalMb,
      customJava: !!instance?.javaOverrides.executable,
    }),
  )

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
      app.toast(t('{name} can now use up to {n}.', { name, n: gb(mb ?? currentMb) }), 'success')
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
      app.toast(backup ? t('{label} is off. {name} is backed up.', { label, name: backup.name }) : t('{label} is off.', { label }), 'success')
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
      app.toast(t('{label} is on again.', { label: disabled.label }), 'success')
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
      app.toast(t('{name}: game files checked.', { name }), 'success')
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  // Report
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
      app.toast(t('Report copied to the clipboard.'), 'success')
    } catch {
      app.toast(t('Couldn’t copy the report. Open the folder and copy the file from there.'), 'error')
    }
  }
</script>

{#snippet bannerText()}
  {#if crash}
    {t('The game closed at {at} with exit code', { at: timeOfDay(crash.at) })} <code>{crash.exitCode}</code>.
  {/if}
{/snippet}

{#snippet disableActions()}
  <Button variant="ghost" onclick={() => (confirmDisable = null)}>{t('Cancel')}</Button>
  <Button variant="primary" icon="mods" busy={busy === 'disable'} disabled={busy === 'disable'} onclick={() => confirmDisable && disableMod(confirmDisable)}>
    {#if busy === 'disable'}
      {backupFirst && lastWorld ? t('Backing up…') : t('Turning off…')}
    {:else}
      {t('Turn off')} <Rich parts={who(confirmDisable, 'the mod')} />
    {/if}
  </Button>
{/snippet}

<div class="screen">
  {#if !crash}
    <div class="content narrow">
      <h1 class="text-title">{name}</h1>
      <Banner tone="success" title={t('No recent crash')}>
        {instance ? t('{name} hasn’t crashed since Cubby opened.', { name }) : t('This instance isn’t here anymore.')}
      </Banner>
      <div class="row">
        {#if instance}
          <Button icon="cube" onclick={() => app.openInstance(id)}>{t('Open {name}', { name })}</Button>
          <Button icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>{t('Show the game log')}</Button>
        {:else}
          <Button icon="library" onclick={() => app.navigate({ name: 'library' })}>{t('Back to the library')}</Button>
        {/if}
      </div>
    </div>
  {:else}
    <div class="layout">
      <div class="content">
        <h1 class="cb-visually-hidden">{t('{name} crash report', { name })}</h1>
        <Banner tone="danger" title={crash.durationSecs > 0 ? `${name} crashed after ${after(crash.durationSecs)}` : `${name} crashed`}>
          {@render bannerText()}
        </Banner>

        <section class="card what" aria-labelledby="what-title">
          <h2 id="what-title" class="text-label muted">{t('What happened')}</h2>
          <p class="text-heading"><Rich parts={cause.title} /></p>
          <p class="text-body"><Rich parts={cause.text} /></p>
          {#if d.evidence.length}
            <div class="evidence" role="group" aria-label={t('Evidence from the game log')}>
              {#each d.evidence as e, i (i)}
                {#if e.gap}
                  <div class="ev"><span class="mark" aria-hidden="true"></span><code class="text-log muted">…</code></div>
                {/if}
                <div class={['ev', e.culprit && 'is-culprit']}>
                  {#if e.culprit}
                    <span class="mark text-label">{t('Here')}<Icon name="chevron-right" size={18} /></span>
                  {:else}
                    <span class="mark" aria-hidden="true"></span>
                  {/if}
                  <code class="text-log">{e.text}</code>
                </div>
              {/each}
            </div>
            <p class="text-small muted"><Rich parts={[...named, ...cause.marks]} /></p>
          {/if}
        </section>

        <section class="fixes" aria-labelledby="fix-title">
          <div class="sec-head">
            <h2 id="fix-title" class="text-label muted">{t('Ways to fix it')}</h2>
            <span class="text-small muted">{t('Best first.')}</span>
          </div>
          <ol class="card list">
            {#each fixes as f, i (f.key)}
              {@const variant = i === 0 ? 'primary' : 'secondary'}
              <li class="fix">
                <span class="num text-counter" aria-hidden="true">{i + 1}</span>
                <div class="fix-body">
                  <h3 class="text-body-strong"><span class="cb-visually-hidden">{t('Fix {i}:', { i: i + 1 })} </span><Rich parts={f.title} /></h3>
                  <p class="text-small muted"><Rich parts={f.text} /></p>
                </div>
                <div class="fix-action">
                  {#if f.id === 'disable' && f.party}
                    {#if disabled}
                      <span class="done text-small"><Icon name="check" size={18} />{disabled.backup ? t('Off, world backed up') : t('Off')}</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === 'disable'} onclick={undoDisable}>{t('Turn it back on')}</Button>
                    {:else}
                      <Button {variant} icon="mods" onclick={() => (confirmDisable = f.party ?? null)}>{t('Turn off')} <Rich parts={who(f.party, 'the mod')} /></Button>
                    {/if}
                  {:else if f.id === 'enable' && f.party?.mod}
                    {#if applied[f.key]}
                      <span class="done text-small"><Icon name="check" size={18} />{t('On')}</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === f.key} onclick={() => f.party && setEnabled(f.party, false)}>{t('Undo')}</Button>
                    {:else}
                      <Button {variant} icon="mods" busy={busy === f.key} disabled={busy === f.key} onclick={() => f.party && setEnabled(f.party, true)}>
                        {t('Turn on')} <Rich parts={who(f.party, 'it')} />
                      </Button>
                    {/if}
                  {:else if f.id === 'find' && f.party}
                    <Button {variant} icon="discover" onclick={() => f.party && find(f.party)}>{t('Find')} <Rich parts={who(f.party, 'it')} /></Button>
                  {:else if f.id === 'memory'}
                    {#if applied.memory}
                      <span class="done text-small"><Icon name="check" size={18} />{t('Now {n}', { n: gb(currentMb) })}</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === 'memory'} onclick={() => setMemory(memoryBefore, false)}>{t('Undo')}</Button>
                    {:else}
                      <Button {variant} icon="memory" busy={busy === 'memory'} disabled={busy === 'memory'} onclick={() => setMemory(raisedMb, true)}>{t('Use {n}', { n: gb(raisedMb) })}</Button>
                    {/if}
                  {:else if f.id === 'java'}
                    {#if applied.java}
                      <span class="done text-small"><Icon name="check" size={18} />{t('Automatic Java')}</span>
                      <Button size="sm" variant="ghost" icon="update" disabled={busy === 'java'} onclick={() => setJava(javaBefore, false)}>{t('Undo')}</Button>
                    {:else}
                      <Button {variant} icon="wrench" busy={busy === 'java'} disabled={busy === 'java'} onclick={() => setJava(null, true)}>{t('Use automatic Java')}</Button>
                    {/if}
                  {:else if f.id === 'updates'}
                    <Button {variant} icon="mods" onclick={() => app.openInstance(id, 'mods')}>{t('Open mods')}</Button>
                  {:else if f.id === 'settings'}
                    <Button {variant} icon="settings" onclick={() => app.openInstance(id, 'settings')}>{t('Open instance settings')}</Button>
                  {:else if f.id === 'repair'}
                    {#if applied.repair && !repairTask}
                      <span class="done text-small"><Icon name="check" size={18} />{t('Game files checked')}</span>
                    {:else}
                      <Button
                        {variant}
                        icon="wrench"
                        busy={!!repairTask || busy === 'repair'}
                        disabled={!!repairTask || busy === 'repair' || running}
                        onclick={repair}
                      >
                        {repairTask ? (repairTask.total ? t('Repairing {percent}%', { percent: Math.round((repairTask.done / repairTask.total) * 100) }) : t('Repairing…')) : t('Repair instance')}
                      </Button>
                    {/if}
                  {:else if f.id === 'help'}
                    <Button {variant} icon="copy" onclick={copyReport}>{t('Copy report')}</Button>
                  {/if}
                  <span class={['undo text-small', f.undoable ? 'is-undoable' : 'muted']}>
                    <Icon name={f.undoable ? 'check' : 'info'} size={18} />{f.undo}
                  </span>
                </div>
              </li>
            {/each}
          </ol>
        </section>

        {#if parsed.length}
          <section class="excerpt" aria-labelledby="excerpt-title">
            <h2 id="excerpt-title" class="text-label muted">{t('Last {n}', { n: plural(Math.min(8, parsed.length), 'log line', 'log lines') })}</h2>
            <LogViewer lines={parsed.slice(-8)} height={240} label={t('Last lines of the game log')} />
          </section>
        {/if}
      </div>

      <CrashAside
        {id}
        {name}
        {d}
        {facts}
        logLines={parsed.length}
        memoryMb={checkedMemoryMb}
        mods={mods.length}
        {enabledMods}
        playDetail={fixes.length ? (anyApplied ? t('With your fix') : t('Pick a fix first')) : undefined}
        onCopyReport={copyReport}
      />
    </div>
  {/if}
</div>

{#if confirmDisable}
  {@const mod = plain(who(confirmDisable, 'this mod'))}
  <Dialog title={t('Turn off {mod}?', { mod })} icon="mods" tone="warning" onClose={() => (confirmDisable = null)} actions={disableActions}>
    <p>
      {t('Without')} <Rich parts={who(confirmDisable, t('it'))} />{t(', its blocks and items are removed from the places you visit, and turning it back on doesn’t bring them back.')}{lastWorld ? t(' A backup of {name} lets you restore them.', { name: lastWorld.name }) : ''}
    </p>
    {#if lastWorld}
      <Switch bind:checked={backupFirst} label={t('Back up {name} first', { name: lastWorld.name })} hint={t('Saved with the backups of this instance.')} />
    {/if}
    <p class="text-small muted">{t('You can turn it back on in Mods.')}</p>
  </Dialog>
{/if}

<style>
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .layout {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
  }
  .content {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: auto;
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
  ol {
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

  .excerpt {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
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
</style>
