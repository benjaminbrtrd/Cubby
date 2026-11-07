<!-- Side of the crash screen: what Cubby checked, the crash report and Play again. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, Icon, PlayButton } from '../../cubby'
  import { plural } from '../../format'
  import { app } from '../../state.svelte'
  import type { Diagnosis } from './crash'
  import type { SessionFacts } from './log'
  import { checksFor } from './crashText'

  let {
    id,
    name,
    d,
    facts,
    logLines,
    memoryMb,
    mods,
    enabledMods,
    playDetail,
    onCopyReport,
  }: {
    id: string
    name: string
    d: Diagnosis
    facts: SessionFacts
    /** Lines in the log of the session that crashed. */
    logLines: number
    /** The memory the game had when it crashed. */
    memoryMb: number
    mods: number
    enabledMods: number
    /** Under Play again: whether a fix was applied. */
    playDetail: string | undefined
    onCopyReport: () => void
  } = $props()

  const running = $derived(app.running[id] != null)
  const launch = $derived(app.tasks[`launch:${id}`])
  const checks = $derived(checksFor(d, facts, { memoryMb, mods, enabledMods }))

  function fileNameOf(path: string): string {
    return path.split(/[\\/]/).pop() ?? path
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

<aside class="aside" aria-label={t('Crash details')}>
  <section class="block" aria-labelledby="checked-title">
    <h2 id="checked-title" class="text-label muted">{t('What Cubby checked')}</h2>
    {#if logLines}
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
      <p class="text-small muted">{t('Cubby looked for these in the last {n} of the game log.', { n: plural(d.checkedLines, 'line', 'lines') })}</p>
    {:else}
      <p class="text-small muted">{t('The game wrote nothing to its log, so there was nothing to check.')}</p>
    {/if}
  </section>

  <section class="block" aria-labelledby="report-title">
    <h2 id="report-title" class="text-label muted">{t('Crash report')}</h2>
    {#if d.reportPath}
      <p class="text-mono path">crash-reports/{fileNameOf(d.reportPath)}</p>
    {:else if d.jvmReportPath}
      <p class="text-mono path">{fileNameOf(d.jvmReportPath)}</p>
      <p class="text-small muted">{t('Java wrote this report in the game folder.')}</p>
    {:else}
      <p class="text-small muted">{t('The game didn’t write a crash report. The log has the details.')}</p>
    {/if}
    <div class="stack">
      <Button size="sm" icon="copy" onclick={onCopyReport}>{t('Copy report')}</Button>
      <Button size="sm" icon="folder" onclick={openReports}>{t('Open folder')}</Button>
    </div>
  </section>

  <div class="play">
    {#if running}
      <Button icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>{t('Show the running game')}</Button>
    {:else if launch}
      <PlayButton state="installing" detail={launch.label} progress={launch.total ? (launch.done / launch.total) * 100 : undefined} />
    {:else}
      <PlayButton label={t('Play again')} detail={playDetail} aria-label={t('Play {name} again', { name })} onclick={playAgain} />
    {/if}
  </div>
</aside>

<style>
  h2,
  p,
  ul {
    margin: 0;
  }
  .muted {
    color: var(--ink-muted);
  }
  .aside {
    flex: none;
    min-height: 0;
    overflow: auto;
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
