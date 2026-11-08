/* Props of the Cubby components, following components/index.d.ts of the design system (React names in camelCase).
   What React typed as ReactNode is a `Renderable`: plain text or a Svelte snippet. */
import type { Snippet } from 'svelte';
import type { ClassValue, HTMLButtonAttributes, HTMLInputAttributes } from 'svelte/elements';
import type { Ground, IconName, Motif, Status } from './data';

export type { ClassValue };
export type Renderable = string | number | Snippet;

type Btn = Omit<HTMLButtonAttributes, 'children'>;

/* Brand */
export interface LogoProps { size?: number; wordmark?: boolean; label?: string; class?: ClassValue }
export interface IconProps { name: IconName; size?: number; label?: string; class?: ClassValue }

/* Actions */
export interface PlayButtonProps extends Btn {
  state?: 'ready' | 'installing' | 'running';
  size?: 'sm' | 'md' | 'lg';
  label?: string;
  detail?: Renderable;
  progress?: number;
  percentText?: string;
}
export interface ButtonProps extends Btn {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md' | 'lg';
  icon?: IconName;
  iconEnd?: IconName;
  kbd?: string | string[];
  busy?: boolean;
  children?: Snippet;
}
export interface IconButtonProps extends Btn {
  icon: IconName;
  label: string;
  variant?: 'ghost' | 'secondary' | 'primary' | 'danger';
  size?: 'sm' | 'md' | 'lg';
}
export interface KbdProps { keys?: string | string[]; children?: Snippet; class?: ClassValue }

/* Navigation */
export interface TitleBarProps {
  /** `art` replaces the avatar image, e.g. a head cropped from a full skin texture. */
  account?: { name: string; src?: string; label?: string; art?: Renderable };
  onAccount?: () => void;
  onSearch?: () => void;
  searchLabel?: string;
  searchKeys?: string[];
  labels?: { minimize?: string; maximize?: string; close?: string };
  onMinimize?: () => void;
  onMaximize?: () => void;
  onClose?: () => void;
  children?: Snippet;
  class?: ClassValue;
}

export interface HotbarItem { id: string; label: string; icon: IconName; badge?: Renderable; href?: string }
export interface HotbarProps {
  items: HotbarItem[];
  footer?: HotbarItem[];
  value?: string;
  onChange?: (id: string) => void;
  expanded?: boolean;
  label?: string;
  class?: ClassValue;
}

export interface TabItem { id: string; label: string; count?: Renderable; panel?: string }
export interface TabsProps {
  items: TabItem[];
  /** Bindable. */
  value?: string;
  onChange?: (id: string) => void;
  label?: string;
  class?: ClassValue;
}

export interface CommandItem {
  id: string;
  label: string;
  detail?: string;
  icon?: IconName;
  art?: Renderable;
  status?: Status;
  kbd?: string | string[];
  hint?: string;
}
export interface CommandGroup { label: string; items: CommandItem[] }
export interface CommandPaletteProps {
  groups: CommandGroup[];
  /** Bindable: the arrow keys move it through the results without leaving the field. */
  activeId?: string;
  /** Bindable. */
  query?: string;
  onQuery?: (q: string) => void;
  /** Click or Enter on a result; `offline` is true for Ctrl Enter. */
  onSelect?: (item: CommandItem, opts: { offline: boolean }) => void;
  /** Called when the arrow keys move the active result (alongside the activeId binding). */
  onActiveChange?: (id: string) => void;
  /** Tab on the active result: open its other actions. Without it, Tab moves focus as usual. */
  onMore?: (item: CommandItem) => void;
  /** Esc in the field. */
  onClose?: () => void;
  placeholder?: string;
  label?: string;
  resultsLabel?: string;
  closeKey?: string;
  footer?: { keys: string | string[]; label: string }[];
  modal?: boolean;
  idBase?: string;
  class?: ClassValue;
}

/* Status */
export interface StatusTagProps { state: Status; detail?: Renderable | null; children?: Snippet; class?: ClassValue }
export interface MetaChipProps { children: Snippet; icon?: IconName; tone?: 'snapshot' | 'accent'; title?: string; class?: ClassValue }
export interface ProgressBarProps {
  value?: number;
  label?: string;
  detail?: Renderable;
  valueText?: string;
  ariaValueText?: string;
  tone?: 'grass' | 'diamond';
  segments?: number;
  class?: ClassValue;
}
export interface AvatarProps { src?: string; name?: string; size?: number; alt?: string; class?: ClassValue }

/* Forms */
export interface TextFieldProps extends Omit<HTMLInputAttributes, 'size' | 'children'> {
  label?: string;
  hideLabel?: boolean;
  hint?: string;
  error?: string;
  icon?: IconName;
  suffix?: Renderable;
  mono?: boolean;
  size?: 'sm' | 'md';
}
export interface SwitchProps {
  /** Bindable. */
  checked: boolean;
  onChange?: (checked: boolean) => void;
  label?: string;
  hint?: string;
  hideLabel?: boolean;
  disabled?: boolean;
  id?: string;
  class?: ClassValue;
}
export interface SegmentedOption { value: string; label: string; icon?: IconName }
export interface SegmentedControlProps {
  options: SegmentedOption[];
  /** Bindable. */
  value?: string;
  onChange?: (value: string) => void;
  label: string;
  class?: ClassValue;
}
export interface SliderProps {
  label: string;
  min?: number;
  max?: number;
  step?: number;
  /** Bindable. */
  value?: number;
  onChange?: (value: number) => void;
  valueText?: string;
  marks?: string[];
  hint?: string;
  id?: string;
  class?: ClassValue;
}

/* Instances */
export interface InstanceIconProps {
  motif?: Motif;
  ground?: Ground;
  seed?: string;
  src?: string | null;
  size?: number;
  framed?: boolean;
  raised?: boolean;
  label?: string;
  class?: ClassValue;
}
export interface IconPickerValue { motif: Motif; ground: Ground; src: string | null }
export interface IconPickerProps {
  /** Bindable, like ground and src. */
  motif?: Motif;
  ground?: Ground;
  src?: string | null;
  name?: string;
  onChange?: (value: IconPickerValue) => void;
  onImport?: () => void;
  labels?: Partial<Record<'motifs' | 'grounds' | 'importImage' | 'reset' | 'custom' | 'importHint', string>>;
  motifLabels?: Partial<Record<Motif, string>>;
  groundLabels?: Partial<Record<Ground, string>>;
  class?: ClassValue;
}
export interface InstanceProps {
  name: string;
  motif?: Motif;
  ground?: Ground;
  iconSrc?: string;
  art?: Renderable;
  version?: string;
  loader?: string;
  snapshot?: boolean;
  status?: Status;
  statusDetail?: string;
  progress?: number;
  selected?: boolean;
  onSelect?: () => void;
  onPlay?: () => void;
  playLabel?: string;
  class?: ClassValue;
}
export interface InstanceTileProps extends InstanceProps { meta?: Renderable }
export interface InstanceRowProps extends Partial<InstanceProps> {
  head?: boolean;
  labels?: Partial<Record<'name' | 'version' | 'loader' | 'mods' | 'played' | 'status', string>>;
  group?: string;
  mods?: number;
  lastPlayed?: string;
}

/* Content */
export interface ModRowProps {
  name: string;
  version?: string;
  authors?: string;
  description?: string;
  iconSrc?: string;
  /** Bindable. */
  enabled?: boolean;
  onToggle?: (enabled: boolean) => void;
  toggleLabel?: string;
  update?: string;
  warning?: string;
  actions?: Renderable;
  moreLabel?: string;
  /** Click on the default ⋯ button (when no `actions` are given). */
  onMore?: (e: MouseEvent) => void;
  class?: ClassValue;
}
export interface ServerRowProps {
  name: string;
  address: string;
  iconSrc?: string;
  motd?: string;
  /** The message of the day with its `§` codes: drawn in the game's colours when given. */
  motdFormatted?: string;
  online?: boolean;
  offlineText?: string;
  players?: { online: number; max: number };
  playersLabel?: string;
  ping?: number;
  pingText?: string;
  instance?: string;
  onJoin?: () => void;
  joinText?: string;
  joinLabel?: string;
  class?: ClassValue;
}
export type LogLevel = 'debug' | 'info' | 'warn' | 'error';
export interface LogLine { time: string; level?: LogLevel; source?: string; text: string }
export interface LogViewerProps {
  lines: LogLine[];
  levelLabels?: Partial<Record<LogLevel, string>>;
  live?: boolean;
  height?: number | string;
  toolbar?: Renderable;
  label?: string;
  class?: ClassValue;
}

/* Feedback */
export type Tone = 'danger' | 'warning' | 'info' | 'success';
export interface BannerProps {
  tone?: Tone;
  title?: Renderable;
  children?: Snippet;
  actions?: Renderable;
  icon?: IconName;
  onDismiss?: () => void;
  dismissLabel?: string;
  class?: ClassValue;
}
export interface DialogProps {
  title: Renderable;
  children?: Snippet;
  actions?: Renderable;
  icon?: IconName;
  tone?: Tone;
  onClose?: () => void;
  closeLabel?: string;
  inline?: boolean;
  wide?: boolean;
}
