<!-- Wizard › Review: the name, group and emblem, then what Cubby installs, each with a way back to its step. -->
<script lang="ts">
  import { t } from '../../../i18n.svelte'
  import { Button, Icon, IconPicker, InstanceIcon, Switch, TextField, pickIcon, type IconName } from '../../../cubby'
  import { loaderName, plural } from '../../../format'
  import { app } from '../../../state.svelte'
  import { readIconImage } from '../Identity.svelte'
  import { wizard as w, type StepId } from './state.svelte'

  let { nameError, onName, onChange }: { nameError?: string; onName?: () => void; onChange: (step: StepId) => void } = $props()

  // The name follows the choices until the player types one.
  $effect(() => {
    if (!w.nameTouched) w.name = w.defaultName
  })

  // The emblem follows the name too, until the player picks one.
  $effect(() => {
    if (w.iconTouched || w.src) return
    const pair = pickIcon(w.name.trim() || 'Minecraft')
    w.motif = pair.motif
    w.ground = pair.ground
  })
  /** A modpack's own image stands in until the player picks an icon. */
  const shownSrc = $derived(w.src ?? (w.path === 'modpack' && !w.iconTouched ? (w.pack?.iconUrl ?? undefined) : undefined))

  const groupsId = $props.id()
  let fileInput: HTMLInputElement | undefined = $state()
  function readImage(e: Event & { currentTarget: HTMLInputElement }) {
    const file = e.currentTarget.files?.[0]
    e.currentTarget.value = ''
    if (!file) return
    readIconImage(file, (data) => {
      w.src = data
      w.iconTouched = true
    })
  }

  const titles = (kind: Parameters<typeof w.picked>[0]) => w.picked(kind).map((p) => p.title)
  const list = (names: string[]) => (names.length > 3 ? `${names.slice(0, 3).join(', ')}…` : names.join(', '))

  const rows = $derived.by((): { icon: IconName; title: string; detail: string; step: StepId }[] => {
    const pack = w.path === 'modpack'
    const mods = titles('mod')
    const looks = [...titles('resourcepack'), ...titles('shader')]
    const dps = titles('datapack')
    const game = `Minecraft ${w.gameVersion ?? '—'}${w.gameLoader ? ` · ${loaderName(w.gameLoader)}` : ''}`
    return [
      {
        icon: pack ? 'library' : 'cube',
        title: pack ? (w.pack?.name ?? t('No modpack')) : game,
        detail: pack ? `${w.packVersion?.name ?? ''} · ${game}` : t('Downloads the first time you play'),
        step: 'game',
      },
      { icon: 'mods', title: pack ? t('{n} on top of the pack', { n: plural(mods.length, 'mod', 'mods') }) : plural(mods.length, 'mod', 'mods'), detail: mods.length ? list(mods) : t('None'), step: 'mods' },
      {
        icon: 'captures',
        title: [plural(w.picked('resourcepack').length, 'resource pack', 'resource packs'), w.picked('shader').length ? t('1 shader pack') : ''].filter(Boolean).join(', '),
        detail: looks.length ? list(looks) : t('None'),
        step: 'looks',
      },
      { icon: 'globe', title: plural(dps.length, 'datapack', 'datapacks'), detail: dps.length ? t('Added to each new world: {list}', { list: list(dps) }) : t('None'), step: 'datapacks' },
    ]
  })
  const hasLooks = $derived(w.picked('resourcepack').length + w.picked('shader').length > 0)
</script>

<div class="intro">
  <h2 class="text-heading">{t('Name it and create it')}</h2>
  <p class="text-small muted">{t('Everything downloads when you create the instance. You can keep using Cubby meanwhile.')}</p>
</div>

<div class="cols">
  <section class="block" aria-labelledby="wz-who">
    <h3 id="wz-who" class="text-label muted">{t('Name, group and icon')}</h3>
    <div class="card">
      <div class="who">
      <InstanceIcon motif={w.motif} ground={w.ground} src={shownSrc} size={96} raised label={t('Icon')} />
      <div class="fields">
        <TextField
          label={t('Name')}
          bind:value={w.name}
          error={nameError}
          maxlength={64}
          oninput={() => {
            w.nameTouched = true
            onName?.()
          }}
        />
        <TextField label={t('Group')} icon="folder" placeholder={t('No group')} list={groupsId} bind:value={w.group} maxlength={40} hint={t('Leave empty for no group')} />
        <datalist id={groupsId}>
          {#each app.groups as g (g.name)}<option value={g.name}></option>{/each}
        </datalist>
      </div>
      </div>
      <!-- the emblem above is the preview: the picker's own one is hidden -->
      <div class="pick">
        <IconPicker
          bind:motif={w.motif}
          bind:ground={w.ground}
          bind:src={w.src}
          name={w.shownName}
          onChange={() => (w.iconTouched = true)}
          onImport={() => fileInput?.click()}
        />
      </div>
    </div>
  </section>

  <section class="block" aria-labelledby="wz-installs">
    <h3 id="wz-installs" class="text-label muted">{t('What Cubby installs')}</h3>
    <ul class="card list">
      {#each rows as r (r.step)}
        <li class="row">
          <span class="ic"><Icon name={r.icon} size={18} /></span>
          <span class="row__txt"><span class="text-body-strong">{r.title}</span><span class="text-small muted">{r.detail}</span></span>
          <Button size="sm" variant="ghost" onclick={() => onChange(r.step)}>{t('Change')}</Button>
        </li>
      {/each}
    </ul>
    {#if w.shaderNeedsIris}
      <p class="text-small warn"><Icon name="warning" size={18} />{t('The shader pack needs Iris.')} <button type="button" class="link" onclick={() => w.addIris()}>{t('Add Iris and Sodium')}</button></p>
    {/if}
    {#if hasLooks}
      <Switch checked={w.turnOn} onChange={(v) => (w.turnOn = v)} label={t('Turn the packs on')} hint={t('The resource packs go on top, and the shader pack is used, the first time you play.')} />
    {/if}
  </section>
</div>

<input bind:this={fileInput} class="cb-visually-hidden" type="file" accept="image/png,image/jpeg" tabindex="-1" aria-hidden="true" onchange={readImage} />

<style>
  h2,
  h3,
  p,
  ul {
    margin: 0;
  }
  ul {
    padding: 0;
    list-style: none;
  }
  .muted {
    color: var(--ink-muted);
  }
  .intro,
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  /* side by side when there is room, else one under the other */
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
    gap: var(--space-6);
    align-items: start;
  }
  .block {
    gap: var(--space-3);
  }
  .card {
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block-sm);
  }
  .who {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-5);
    align-items: center;
    padding: var(--space-5);
  }
  .who > :global(.cb-instance-icon) {
    flex: none;
  }
  .pick {
    padding: var(--space-5);
    border-top: var(--border-hair) solid var(--line);
  }
  .pick :global(.cb-ipick__preview) {
    display: none;
  }
  .pick :global(.cb-ipick__grid) {
    margin-bottom: var(--space-4);
  }
  .pick :global(.cb-ipick__swatches) {
    margin-bottom: var(--space-4);
  }
  .fields {
    flex: 1 1 240px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .row {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    gap: var(--space-3);
    align-items: center;
    padding: 10px 14px;
    background: var(--surface);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .ic {
    display: inline-flex;
    color: var(--grass-text);
  }
  .row__txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .warn {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    color: var(--gold-text);
  }
  .link {
    padding: 0;
    font: inherit;
    color: var(--grass-text);
    text-decoration: underline;
    background: none;
    border: 0;
    cursor: pointer;
  }
</style>
