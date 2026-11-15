<!-- New instance: a wizard. Start (build your own, a modpack, or import), then the game (version and loader) or the
     modpack, then optional mods, looks and datapacks, then a review that creates the instance. The summary on the
     right follows every choice; what was picked installs once the instance (and its modpack) is ready. -->
<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from '../../../i18n.svelte'
  import { api } from '../../../api'
  import { Button, Icon } from '../../../cubby'
  import { plural } from '../../../format'
  import { app } from '../../../state.svelte'
  import { iconValue } from '../Identity.svelte'
  import ContentStep from './ContentStep.svelte'
  import GameStep from './GameStep.svelte'
  import PackStep from './PackStep.svelte'
  import ReviewStep from './ReviewStep.svelte'
  import StartStep from './StartStep.svelte'
  import Summary from './Summary.svelte'
  import { STEP_ORDER, wizard as w, type StepId } from './state.svelte'

  $effect(() => {
    if (!app.providers && !app.providersError) void app.loadProviders()
  })

  // Opened from elsewhere on a given source (an import, a pack link, a provider): start there once.
  untrack(() => {
    if (app.newSource !== 'quick') {
      w.openOn(app.newSource, app.providers)
      app.newSource = 'quick'
    }
  })

  const steps = $derived(w.steps)
  const index = $derived(Math.max(0, steps.findIndex((s) => s.id === w.step)))
  const current = $derived(steps[index])
  const last = $derived(index === steps.length - 1)

  /** What the step needs before Next. */
  const blocker = $derived.by((): string | null => {
    if (w.step === 'start' && w.path === 'import') return t('Pick a file or paste a link above.')
    if (w.step === 'game' && w.path === 'custom') {
      if (!w.minecraft) return t('Pick a Minecraft version.')
      if (w.loader !== 'vanilla' && !w.loaderVersion) return t('Pick a loader version.')
    }
    if (w.step === 'game' && w.path === 'modpack') {
      if (!w.pack) return t('Pick a modpack.')
      if (!w.packVersionId) return t('Pick a version of the pack.')
    }
    return null
  })

  function go(id: StepId) {
    // a later step needs the game or the pack first
    const gameIndex = STEP_ORDER.indexOf('game')
    if (STEP_ORDER.indexOf(id) > gameIndex && w.step !== id) {
      const ready = w.path === 'modpack' ? !!w.pack && !!w.packVersionId : !!w.minecraft && (w.loader === 'vanilla' || !!w.loaderVersion)
      if (!ready) {
        w.step = 'game'
        return
      }
    }
    w.step = id
    document.querySelector('.wz-body')?.scrollTo?.(0, 0)
  }

  function next() {
    if (blocker || last) return
    go(steps[index + 1].id)
  }

  function back() {
    if (index > 0) go(steps[index - 1].id)
  }

  function cancel() {
    app.navigate({ name: 'library' })
  }

  // create

  let busy = $state<'create' | 'play' | null>(null)
  let nameError = $state<string | undefined>()

  async function create(play: boolean) {
    if (busy) return
    const name = w.shownName.trim()
    if (!name) {
      nameError = t('Give the instance a name.')
      return
    }
    busy = play ? 'play' : 'create'
    const extras = w.picks.map(({ source, projectId, kind, title }) => ({ source, projectId, kind, title }))
    const turnOn = w.turnOn
    const group = w.group.trim() || null
    try {
      let instance
      if (w.path === 'modpack' && w.pack && w.packVersionId) {
        const icon = w.iconTouched ? iconValue(w.src, w.motif, w.ground) : (w.pack.iconUrl ?? null)
        instance = await api.installPack(w.provider, w.pack.id, w.packVersionId, name, group, icon, null)
      } else if (w.minecraft) {
        instance = await api.createInstance({
          name,
          group,
          icon: iconValue(w.src, w.motif, w.ground),
          minecraft: w.minecraft,
          loader: w.loader === 'vanilla' || !w.loaderVersion ? { kind: 'vanilla' } : { kind: w.loader, version: w.loaderVersion },
        })
      } else {
        return
      }
      const id = instance.id
      const fromPack = w.path === 'modpack'
      await app.refreshInstances()
      app.selected = id
      app.openInstance(id)
      w.reset()
      if (extras.length || (play && fromPack)) {
        if (extras.length) app.toast(t('{name} is ready. Cubby is adding {n}.', { name, n: plural(extras.length, 'project', 'projects') }), 'info')
        api
          .installExtras(id, extras, turnOn)
          .then((report) => {
            if (report.missing.length) app.toast(t('Nothing for this instance’s version or loader: {list}.', { list: report.missing.join(', ') }), 'error')
            else if (extras.length) app.toast(t('Everything you picked is in {name}.', { name }), 'success')
            if (play) void app.launch(id)
          })
          .catch((e) => app.error(e))
      } else if (play) {
        void app.launch(id)
      } else {
        app.toast(t('{name} is ready. Game files are downloaded the first time you play.', { name }), 'success')
      }
    } catch (e) {
      app.error(e)
    } finally {
      busy = null
    }
  }

  function onWindowKey(e: KeyboardEvent) {
    if (e.defaultPrevented || app.paletteOpen || app.deletingId || app.prismImportOpen) return
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && w.step === 'review') {
      e.preventDefault()
      void create(true)
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<div class="wz">
  <div class="main">
    <header class="head">
      <div class="titles">
        <p class="text-small muted">
          <button type="button" class="link" onclick={cancel}>{t('Library')}</button>
          <span aria-hidden="true">&nbsp;›&nbsp;</span>{t('New instance')}
        </p>
        <h1 class="text-title">{t('New instance')}</h1>
      </div>
      <Button variant="ghost" size="sm" icon="close" onclick={cancel}>{t('Cancel')}</Button>
    </header>

    {#if steps.length > 1}
      <ol class="steps" aria-label={t('Steps')}>
        {#each steps as s, i (s.id)}
          <li>
            <button type="button" class={['step', i === index && 'is-on', i < index && 'is-done']} aria-current={i === index ? 'step' : undefined} onclick={() => go(s.id)}>
              <span class="num text-counter" aria-hidden="true">{#if i < index}<Icon name="check" size={18} />{:else}{i + 1}{/if}</span>
              <span class="step__txt">
                <span class="text-body-strong">{s.label}</span>
                <span class="sub">{s.sub}</span>
              </span>
            </button>
          </li>
        {/each}
      </ol>
    {/if}

    <section class="wz-body" data-keep-scroll aria-label={current?.label}>
      {#if w.step === 'start'}
        <StartStep />
      {:else if w.step === 'game'}
        {#if w.path === 'modpack'}<PackStep />{:else}<GameStep />{/if}
      {:else if w.step === 'mods'}
        <ContentStep tabs={['mod']} />
      {:else if w.step === 'looks'}
        <ContentStep tabs={['resourcepack', 'shader']} />
      {:else if w.step === 'datapacks'}
        <ContentStep tabs={['datapack']} />
      {:else}
        <ReviewStep {nameError} onName={() => (nameError = undefined)} onChange={go} />
      {/if}
    </section>

    {#if steps.length > 1}
      <footer class="foot">
        <Button variant="ghost" icon="chevron-left" disabled={index === 0} onclick={back}>{t('Back')}</Button>
        <span class="grow"></span>
        {#if blocker}<span class="text-small muted">{blocker}</span>{:else}<span class="text-small muted">{t('Step {n} of {total}', { n: index + 1, total: steps.length })}</span>{/if}
        {#if current?.optional}<Button variant="ghost" onclick={next}>{t('Skip')}</Button>{/if}
        {#if last}
          <Button icon="plus" busy={busy === 'create'} disabled={!!busy} onclick={() => create(false)}>{t('Create instance')}</Button>
          <Button variant="primary" icon="play" busy={busy === 'play'} disabled={!!busy} onclick={() => create(true)}>{t('Create and play')}</Button>
        {:else}
          <Button variant="primary" iconEnd="chevron-right" disabled={!!blocker} onclick={next}>{t('Next: {step}', { step: steps[index + 1]?.label ?? '' })}</Button>
        {/if}
      </footer>
    {/if}
  </div>

  {#if steps.length > 1}<Summary onChange={go} />{/if}
</div>

<style>
  .wz {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  p,
  h1,
  ol {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .grow {
    flex: 1;
  }
  .head {
    display: flex;
    align-items: flex-end;
    gap: var(--space-4);
    padding: var(--space-5) var(--space-6) 0;
  }
  .titles {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
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
  .steps {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    margin: var(--space-3) var(--space-6) 0;
    padding: 0;
    list-style: none;
    border-bottom: var(--border-block) solid var(--line);
  }
  .steps li {
    margin-bottom: calc(-1 * var(--border-block));
  }
  .step {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: var(--size-control-lg);
    padding: 6px 14px 6px 8px;
    font: inherit;
    color: var(--ink-muted);
    text-align: start;
    background: transparent;
    border: 0;
    border-bottom: 3px solid transparent;
    cursor: pointer;
  }
  .step:hover {
    color: var(--ink);
    background: var(--surface);
  }
  .step:focus-visible {
    outline: 2px solid transparent;
    box-shadow: var(--focus-ring);
  }
  .step.is-on {
    color: var(--ink);
    border-bottom-color: var(--grass-text);
  }
  .num {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 24px;
    height: 24px;
    border: var(--border-block) solid var(--line);
  }
  .step.is-on .num {
    border-color: var(--ink);
    color: var(--ink);
  }
  .step.is-done .num {
    color: var(--on-grass);
    background: var(--grass);
    border-color: var(--edge);
  }
  .step__txt {
    display: flex;
    flex-direction: column;
    line-height: 18px;
    min-width: 0;
  }
  .sub {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--ink-muted);
  }
  .wz-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-6);
  }
  .foot {
    flex: none;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-6);
    border-top: var(--border-block) solid var(--edge);
  }
</style>
