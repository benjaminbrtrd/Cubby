/* Cubby design system for Svelte 5. Load src/styles/fonts.css, tokens.css, then cubby.css once in the app. */

/* Brand */
export { default as Logo } from './Logo.svelte';
export { default as Icon } from './Icon.svelte';
/* Actions */
export { default as PlayButton } from './PlayButton.svelte';
export { default as Button } from './Button.svelte';
export { default as IconButton } from './IconButton.svelte';
export { default as Kbd } from './Kbd.svelte';
/* Navigation */
export { default as TitleBar } from './TitleBar.svelte';
export { default as Hotbar } from './Hotbar.svelte';
export { default as Tabs } from './Tabs.svelte';
export { default as CommandPalette } from './CommandPalette.svelte';
/* Status */
export { default as StatusTag } from './StatusTag.svelte';
export { default as MetaChip } from './MetaChip.svelte';
export { default as ProgressBar } from './ProgressBar.svelte';
export { default as Avatar } from './Avatar.svelte';
/* Forms */
export { default as TextField } from './TextField.svelte';
export { default as Switch } from './Switch.svelte';
export { default as SegmentedControl } from './SegmentedControl.svelte';
export { default as Slider } from './Slider.svelte';
/* Instances */
export { default as InstanceIcon } from './InstanceIcon.svelte';
export { default as IconPicker } from './IconPicker.svelte';
export { default as InstanceTile } from './InstanceTile.svelte';
export { default as InstanceRow } from './InstanceRow.svelte';
/* Content */
export { default as ModRow } from './ModRow.svelte';
export { default as ServerRow } from './ServerRow.svelte';
export { default as Motd } from './Motd.svelte';
export { default as LogViewer } from './LogViewer.svelte';
/* Feedback */
export { default as Banner } from './Banner.svelte';
export { default as Dialog } from './Dialog.svelte';
export { default as Tooltips } from './Tooltips.svelte';
/* Side panels */
export { default as Inspector } from './Inspector.svelte';
export { default as InspectorHead } from './InspectorHead.svelte';
export { default as InspectorSection } from './InspectorSection.svelte';
export { default as InspectorStatus } from './InspectorStatus.svelte';
export { default as InspectorNotice } from './InspectorNotice.svelte';
export { default as InspectorPath } from './InspectorPath.svelte';

/* Data and helpers */
export {
  ICONS,
  ICON_NAMES,
  MIRRORED,
  MARK,
  WORDMARK,
  WORD_W,
  MOTIFS,
  MOTIF_IDS,
  MOTIF_LABELS,
  GROUNDS,
  GROUND_IDS,
  GROUND_LABELS,
  cx,
  pct,
  iconSize,
  iconStroke,
  logoScale,
  hash,
  pickIcon,
  isIconName,
  isMotif,
  isGround,
  rovingKeys,
} from './data';
export type { Run, ColorRun, IconName, IconShape, Status, Motif, Ground, MotifDef, GroundDef } from './data';
export type * from './types';
