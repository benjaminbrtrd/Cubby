<!-- Name, group and icon of a new instance: the same fields as the Custom form, for packs and imports. -->
<script lang="ts" module>
  import { encodeIcon } from '../../icon'
  import type { Ground, Motif } from '../../cubby'

  /** What goes into the instance's `icon`: the picked image, else "motif:ground". */
  export function iconValue(src: string | null, motif: Motif | undefined, ground: Ground | undefined): string | null {
    return src ?? (motif && ground ? encodeIcon(motif, ground) : null)
  }
</script>

<script lang="ts">
  import { Button, IconPicker, InstanceIcon, TextField, pickIcon } from '../../cubby'
  import { app } from '../../state.svelte'

  interface Props {
    name: string
    group: string
    motif?: Motif
    ground?: Ground
    src?: string | null
    /** Shown under the name field when it is empty after a try. */
    nameError?: string
    /** Called when the player types a name. */
    onName?: () => void
    /** Called when the player picks an icon. */
    onIcon?: () => void
    /** Icons follow the name until the player picks one. */
    iconTouched?: boolean
    /** false: name and group only (imports keep the pack's own icon). */
    withIcon?: boolean
  }

  let {
    name = $bindable(),
    group = $bindable(),
    motif = $bindable(),
    ground = $bindable(),
    src = $bindable(null),
    nameError,
    onName,
    onIcon,
    iconTouched = $bindable(false),
    withIcon = true,
  }: Props = $props()

  const listId = $props.id()
  let fileInput: HTMLInputElement | undefined = $state()
  let pick = $state(false)

  // a pack without an image gets an emblem from its name, until the player picks one
  $effect(() => {
    if (iconTouched || src) return
    const pair = pickIcon(name.trim() || 'Minecraft')
    motif = pair.motif
    ground = pair.ground
  })

  function readImage(e: Event & { currentTarget: HTMLInputElement }) {
    const file = e.currentTarget.files?.[0]
    e.currentTarget.value = ''
    if (!file) return
    if (!/^image\/(png|jpeg)$/.test(file.type)) return app.toast('Pick a PNG or JPG image.', 'error')
    if (file.size > 512_000) return app.toast('Pick an image under 512 KB.', 'error')
    const reader = new FileReader()
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        src = reader.result
        iconTouched = true
      }
    }
    reader.onerror = () => app.toast("Couldn't read this image.", 'error')
    reader.readAsDataURL(file)
  }
</script>

<section class="identity" aria-label="Name, group and icon">
  <div class="named">
    {#if withIcon}<InstanceIcon {motif} {ground} {src} size={44} raised label="Icon" />{/if}
    <TextField label="Name" bind:value={name} error={nameError} maxlength={64} oninput={() => onName?.()} />
  </div>
  <div>
    <TextField label="Group" icon="folder" placeholder="No group" list={listId} bind:value={group} maxlength={40} hint="Leave empty for no group" />
    <datalist id={listId}>
      {#each app.groups as g (g.name)}<option value={g.name}></option>{/each}
    </datalist>
  </div>
  {#if withIcon}
  <div class="icon">
    <Button size="sm" variant="ghost" icon={pick ? 'close' : 'import'} aria-expanded={pick} onclick={() => (pick = !pick)}>{pick ? 'Done' : 'Change icon'}</Button>
    {#if pick}
      <IconPicker
        bind:motif
        bind:ground
        bind:src
        name={name.trim()}
        onChange={() => {
          iconTouched = true
          onIcon?.()
        }}
        onImport={() => fileInput?.click()}
      />
    {/if}
  </div>
  {/if}
  <input bind:this={fileInput} class="cb-visually-hidden" type="file" accept="image/png,image/jpeg" tabindex="-1" aria-hidden="true" onchange={readImage} />
</section>

<style>
  .identity {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }
  .named {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }
  .named :global(.cb-tf) {
    flex: 1;
    min-width: 0;
  }
  .named :global(.cb-instance-icon) {
    flex: none;
    margin-top: 28px;
  }
  .icon {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
  }
</style>
