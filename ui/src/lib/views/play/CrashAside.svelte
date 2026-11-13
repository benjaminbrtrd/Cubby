<!-- Side of the crash screen: what Cubby checked, the crash report and Play again. -->
<script lang="ts">
  import { t } from '../../i18n.svelte'
  import { api } from '../../api'
  import { Button, Icon, Inspector, InspectorHead, InspectorPath, InspectorSection, InspectorStatus, PlayButton } from '../../cubby'
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
  <Inspector>
    <InspectorHead eyebrow={t('Crash')} title={name} sub={d.kind === 'unknown' ? t('Cubby couldn’t find the cause') : t('Cubby found what most likely went wrong')}>
      {#snippet meta()}
        {#if d.kind === 'unknown'}<InspectorStatus tone="warn">{t('Cause not found')}</InspectorStatus>{:else}<InspectorStatus tone="bad">{t('Cause found')}</InspectorStatus>{/if}
      {/snippet}
    </InspectorHead>
    <div class="cb-ins-acts">
      {#if running}
        <Button class="wide" icon="terminal" onclick={() => app.navigate({ name: 'running', id })}>{t('Show the running game')}</Button>
      {:else if launch}
        <PlayButton state="installing" detail={launch.label} progress={launch.total ? (launch.done / launch.total) * 100 : undefined} />
      {:else}
        <PlayButton label={t('Play again')} detail={playDetail} aria-label={t('Play {name} again', { name })} onclick={playAgain} />
      {/if}
    </div>
    <InspectorSection title={t('What Cubby checked')}>
      {#if logLines}
        <ul class="cb-ins-rows">
          {#each [...checks].sort((a, b) => Number(b.bad) - Number(a.bad)) as c (c.label)}
            <li class="cb-ins-row">
              <div class="cb-ins-row__txt">
                <span class="cb-ins-row__t">{c.label}</span>
                {#if c.detail}<span class="cb-ins-row__s" title={c.detail}>{c.detail}</span>{/if}
              </div>
              <span class={['state', c.bad ? 'is-bad' : 'is-ok']}><Icon name={c.bad ? 'error' : 'check'} size={18} />{c.word}</span>
            </li>
          {/each}
        </ul>
        <p class="cb-ins-hint">{t('Cubby looked for these in the last {n} of the game log.', { n: plural(d.checkedLines, 'line', 'lines') })}</p>
      {:else}
        <p class="cb-ins-hint">{t('The game wrote nothing to its log, so there was nothing to check.')}</p>
      {/if}
    </InspectorSection>
    <InspectorSection title={t('Crash report')}>
      {#if d.reportPath || d.jvmReportPath}
        <dl class="cb-ins-facts">
          <InspectorPath path={d.reportPath ? `crash-reports/${fileNameOf(d.reportPath)}` : fileNameOf(d.jvmReportPath ?? '')} openLabel={t('Open folder')} onopen={openReports} />
        </dl>
        {#if !d.reportPath}<p class="cb-ins-hint">{t('Java wrote this report in the game folder.')}</p>{/if}
      {:else}
        <p class="cb-ins-hint">{t('The game didn’t write a crash report. The log has the details.')}</p>
      {/if}
      <div class="cb-ins-acts">
        <Button icon="copy" onclick={onCopyReport}>{t('Copy report')}</Button>
        <Button icon="folder" onclick={openReports}>{t('Open folder')}</Button>
      </div>
    </InspectorSection>
  </Inspector>
</aside>

<style>
  p,
  ul {
    margin: 0;
  }
  .aside {
    flex: none;
    min-height: 0;
    overflow: auto;
    width: var(--size-inspector);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .state {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    font-size: 14px;
    font-weight: 500;
  }
  .state.is-ok {
    color: var(--grass-text);
  }
  .state.is-bad {
    color: var(--redstone-text);
  }
</style>
