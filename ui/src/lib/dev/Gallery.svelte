<!-- Dev-only page (gallery.html) showing every Cubby component in its main states. -->
<script lang="ts">
  import {
    Avatar,
    Banner,
    Button,
    CommandPalette,
    Dialog,
    Hotbar,
    Icon,
    IconButton,
    IconPicker,
    InstanceIcon,
    InstanceRow,
    InstanceTile,
    Kbd,
    LogViewer,
    Logo,
    MetaChip,
    ModRow,
    PlayButton,
    ProgressBar,
    SegmentedControl,
    ServerRow,
    Slider,
    StatusTag,
    Switch,
    Tabs,
    TextField,
    TitleBar,
    GROUND_IDS,
    GROUND_LABELS,
    ICON_NAMES,
    MOTIF_IDS,
    MOTIF_LABELS,
    pickIcon,
  } from '../cubby/index';
  import Content from '../cubby/Content.svelte';
  import Tooltips from '../cubby/Tooltips.svelte';
  import { pixelArt } from './pixelart';
  import type { CommandGroup, CommandItem, Ground, HotbarItem, InstanceRowProps, InstanceTileProps, LogLine, Motif, Status } from '../cubby/index';

  /* Theme, font and direction */
  type Theme = 'dark' | 'light' | 'contrast';
  let theme = $state<Theme>('dark');
  let plain = $state(false);
  let rtl = $state(false);
  $effect(() => {
    const root = document.documentElement;
    root.dataset.theme = theme;
    if (plain) root.dataset.font = 'plain';
    else delete root.dataset.font;
    root.dir = rtl ? 'rtl' : 'ltr';
  });

  const SECTIONS = [
    ['brand', 'Brand'],
    ['actions', 'Actions'],
    ['navigation', 'Navigation'],
    ['status', 'Status'],
    ['forms', 'Forms'],
    ['instances', 'Instances'],
    ['content', 'Content'],
    ['feedback', 'Feedback'],
    ['screen', 'Library screen'],
  ];

  const SKIN = pixelArt(
    ['00000000', '00000000', '01111110', '11111111', '12311321', '11144111', '11555511', '11111111'],
    { 0: '#3b2a20', 1: '#d9a07a', 2: '#ffffff', 3: '#3a4a7a', 4: '#b9805c', 5: '#7a4a36' },
  );
  const SERVER_ICON = pixelArt(
    ['00000000', '00100000', '00000000', '00022000', '00222200', '33244233', '53333335', '55555555'],
    { 0: '#4cc9d6', 1: '#f6e7a6', 2: '#9a9a9a', 3: '#7fc956', 4: '#5a3a22', 5: '#8b5a2b' },
  );
  const account = { name: 'Nyx_Ember', src: SKIN, label: 'Account: Nyx_Ember' };

  /* Shared demo state */
  let lastAction = $state('Nothing yet');
  function did(what: string) {
    lastAction = what;
  }

  let progress = $state(42);
  $effect(() => {
    const t = setInterval(() => (progress = progress >= 100 ? 8 : progress + 2), 500);
    return () => clearInterval(t);
  });

  const KBD_ROWS: [string | string[], string][] = [
    [['Ctrl', 'K'], 'Search or launch'],
    ['Enter', 'Play'],
    [['Ctrl', 'Enter'], 'Play offline'],
    ['F2', 'Rename'],
    ['Del', 'Move to trash'],
    [['Ctrl', 'L'], 'Game log'],
  ];

  /* Navigation */
  const HOTBAR: HotbarItem[] = [
    { id: 'library', label: 'Library', icon: 'library' },
    { id: 'discover', label: 'Discover', icon: 'discover' },
    { id: 'servers', label: 'Servers', icon: 'servers' },
    { id: 'captures', label: 'Screenshots', icon: 'captures' },
    { id: 'downloads', label: 'Downloads', icon: 'download', badge: 3 },
  ];
  const HOTBAR_FOOT: HotbarItem[] = [
    { id: 'accounts', label: 'Accounts', icon: 'accounts' },
    { id: 'settings', label: 'Settings', icon: 'settings' },
  ];
  let nav = $state('library');

  const TABS = [
    { id: 'all', label: 'All', count: 12 },
    { id: 'survival', label: 'Survival', count: 4 },
    { id: 'packs', label: 'Modpacks', count: 5 },
    { id: 'tests', label: 'Testing', count: 3 },
    { id: 'trash', label: 'Trash', count: 1 },
  ];
  let tab = $state('all');

  // Built lazily ($derived) because the items carry snippets declared in the markup below.
  const paletteGroups = $derived<CommandGroup[]>([
    {
      label: 'Instances',
      items: [
        { id: 'i1', label: 'Friends SMP', detail: '1.21.1 · Fabric 0.16.5', art: artHouse, kbd: 'Enter', hint: 'Play' },
        { id: 'i2', label: 'Create: Above and Beyond', detail: '1.20.1 · Fabric · 214 mods', art: artGear, hint: 'Play' },
        { id: 'i3', label: 'Snapshot 24w45a', detail: '24w45a · Vanilla', art: artPotion, status: 'download', hint: 'Play' },
      ],
    },
    { label: 'Servers', items: [{ id: 's1', label: 'Join The Crew', detail: 'crew.example · with Friends SMP', icon: 'servers', hint: 'Join' }] },
    {
      label: 'Actions',
      items: [
        { id: 'a1', label: 'Create instance', icon: 'plus', kbd: ['Ctrl', 'N'] },
        { id: 'a2', label: 'Import from Prism Launcher', icon: 'import' },
        { id: 'a3', label: 'Open the last game log', icon: 'terminal', kbd: ['Ctrl', 'L'] },
      ],
    },
  ]);
  let query = $state('fri');
  let activeId = $state('i1');
  let paletteOpen = $state(false);
  let paletteHost: HTMLDivElement | undefined = $state();
  let paletteOpener: HTMLElement | null = null;
  function openPalette(e: MouseEvent) {
    paletteOpener = e.currentTarget as HTMLElement;
    paletteOpen = true;
  }
  function closePalette() {
    paletteOpen = false;
    paletteOpener?.focus();
  }
  $effect(() => {
    if (paletteOpen && paletteHost) paletteHost.querySelector<HTMLInputElement>('.cb-cmd__input')?.focus();
  });
  function picked(item: CommandItem, opts: { offline: boolean }) {
    did(`Palette: ${item.label}${opts.offline ? ' (offline)' : ''}`);
  }

  /* Status */
  const STATES: Status[] = ['ready', 'download', 'installing', 'running', 'crashed', 'update', 'online', 'offline', 'disabled'];

  /* Forms */
  let instanceName = $state('Friends SMP');
  let address = $state('play.example');
  let search = $state('');
  let javaArgs = $state('-XX:+UseG1GC -XX:MaxGCPauseMillis=50');
  let trackTime = $state(true);
  let snapshots = $state(false);
  let whileRunning = $state('hide');
  let memory = $state(6144);
  const fmt = new Intl.NumberFormat('en-US');

  /* Instances */
  let pickMotif = $state<Motif | undefined>('island');
  let pickGround = $state<Ground | undefined>('lagoon');
  let pickSrc = $state<string | null | undefined>(null);
  const SEEDS = ['Friends SMP', 'Vanilla 1.21', 'Create: Above and Beyond', 'Lou’s world', 'Test 24w45a', 'Speedrun'];

  type Tile = InstanceTileProps & { id: string };
  const TILES: Tile[] = [
    { id: 'a', name: 'Friends SMP', motif: 'house', ground: 'moss', version: '1.21.1', loader: 'Fabric', meta: 'Played 2h ago · 41h' },
    {
      id: 'b',
      name: 'Create: Above and Beyond',
      motif: 'gear',
      ground: 'amber',
      version: '1.20.1',
      loader: 'Fabric',
      status: 'installing',
      progress: 42,
      statusDetail: '42%',
      meta: 'Installing · 214 mods',
    },
    { id: 'c', name: 'Chill Skyblock', motif: 'island', ground: 'lagoon', version: '1.21.1', loader: 'Quilt', status: 'running', statusDetail: '00:42', meta: 'Playing for 42 min' },
    { id: 'd', name: 'Snapshot 24w45a', motif: 'potion', ground: 'rose', version: '24w45a', snapshot: true, status: 'download', meta: 'Never played' },
    { id: 'e', name: 'Hardcore 1.8 PvP', motif: 'sword', ground: 'brick', version: '1.8.9', status: 'crashed', meta: '3 weeks ago · 6h' },
    { id: 'f', name: 'Better Minecraft', motif: 'pickaxe', ground: 'slate', version: '1.20.1', loader: 'Fabric', status: 'update', meta: '4 days ago · 63h' },
    { id: 'g', name: 'Old Forge pack (picked from its name)', version: '1.12.2', loader: 'Forge', status: 'disabled', meta: 'Needs Java 8' },
    { id: 'h', name: 'Imported image', iconSrc: SERVER_ICON, version: '1.21.1', meta: 'Custom icon' },
  ];
  const ROWS: (InstanceRowProps & { id: string })[] = [
    { id: 'a', name: 'Friends SMP', group: 'Survival', motif: 'house', ground: 'moss', version: '1.21.1', loader: 'Fabric 0.16.5', mods: 38, lastPlayed: '2h ago' },
    { id: 'b', name: 'Create: Above and Beyond', group: 'Modpacks', motif: 'gear', ground: 'amber', version: '1.20.1', loader: 'Fabric 0.15.11', mods: 214, lastPlayed: 'Yesterday', status: 'update' },
    { id: 'c', name: 'Chill Skyblock', group: 'Survival', motif: 'island', ground: 'lagoon', version: '1.21.1', loader: 'Quilt 0.27.1', mods: 12, lastPlayed: 'Now', status: 'running', statusDetail: '00:42' },
    { id: 'd', name: 'Snapshot 24w45a', group: 'Testing', motif: 'potion', ground: 'rose', version: '24w45a', snapshot: true, mods: 0, lastPlayed: 'Never', status: 'download' },
    { id: 'e', name: 'Hardcore 1.8 PvP', group: 'Testing', motif: 'sword', ground: 'brick', version: '1.8.9', mods: 0, lastPlayed: '3 weeks ago', status: 'crashed' },
    { id: 'f', name: 'Better Minecraft', group: 'Modpacks', version: '1.20.1', loader: 'Fabric 0.15.11', lastPlayed: '4 days ago', status: 'installing', statusDetail: '42%' },
  ];
  let tileSel = $state('a');
  let rowSel = $state('a');

  /* Content */
  let mods = $state({ sodium: true, iris: true, create: true, optifine: false, lithium: false });

  const LOG: LogLine[] = [
    { time: '14:02:09', level: 'info', source: 'main', text: 'Loading Minecraft 1.21.1 with Fabric Loader 0.16.5' },
    { time: '14:02:10', level: 'debug', source: 'main', text: 'Classpath has 214 entries' },
    { time: '14:02:10', level: 'info', source: 'main', text: 'Loading 38 mods' },
    { time: '14:02:11', level: 'info', source: 'Render thread', text: 'Setting user: Nyx_Ember' },
    { time: '14:02:12', level: 'warn', source: 'Render thread', text: 'Mod "iris" requests a newer version of "sodium" (0.6.0 < 0.6.1)' },
    { time: '14:02:14', level: 'info', source: 'Render thread', text: 'Reloading ResourceManager: vanilla, fabric, Faithful 32x' },
    { time: '14:02:18', level: 'info', source: 'Server thread', text: 'Preparing spawn area: 84%' },
    {
      time: '14:02:21',
      level: 'error',
      source: 'Render thread',
      text: 'java.lang.OutOfMemoryError: Java heap space\n    at net.minecraft.client.renderer.LevelRenderer.compileChunks(LevelRenderer.java:1834)',
    },
    { time: '14:02:21', level: 'info', source: 'main', text: 'Stopping!' },
  ];
  let logLevel = $state('all');
  let logFilter = $state('');
  let follow = $state(true);
  const shownLog = $derived(
    LOG.filter(
      (l) => (logLevel === 'all' || l.level === logLevel) && (!logFilter || l.text.toLowerCase().includes(logFilter.toLowerCase())),
    ),
  );

  /* Feedback */
  let dismissed = $state({ warning: false, success: false });
  let dialog = $state<'' | 'trash' | 'rename' | 'icon'>('');
  let renameValue = $state('Hardcore 1.8 PvP');
  function closeDialog() {
    dialog = '';
  }

  /* Library screen */
  type LibItem = Tile & { group: string };
  const LIB: LibItem[] = [
    { id: 'a', name: 'Friends SMP', motif: 'house', ground: 'moss', version: '1.21.1', loader: 'Fabric', meta: 'Played 2h ago · 41h', group: 'survival' },
    {
      id: 'b',
      name: 'Create: Above and Beyond',
      motif: 'gear',
      ground: 'amber',
      version: '1.20.1',
      loader: 'Fabric',
      status: 'installing',
      progress: 42,
      statusDetail: '42%',
      meta: 'Installing · 214 mods',
      group: 'packs',
    },
    { id: 'c', name: 'Chill Skyblock', motif: 'island', ground: 'lagoon', version: '1.21.1', loader: 'Quilt', meta: 'Played yesterday · 12h', group: 'survival' },
    { id: 'd', name: 'Snapshot 24w45a', motif: 'potion', ground: 'rose', version: '24w45a', snapshot: true, status: 'download', meta: 'Never played', group: 'tests' },
    { id: 'e', name: 'Better Minecraft', motif: 'pickaxe', ground: 'slate', version: '1.20.1', loader: 'Fabric', status: 'update', meta: '4 days ago · 63h', group: 'packs' },
    { id: 'f', name: 'Hardcore 1.8 PvP', motif: 'sword', ground: 'brick', version: '1.8.9', meta: '3 weeks ago · 6h', group: 'tests' },
    { id: 'g', name: 'Medieval Town', motif: 'castle', ground: 'night', version: '1.21.1', loader: 'Fabric', meta: '1 month ago · 22h', group: 'survival' },
    { id: 'h', name: 'Antarctic Ski Trip', motif: 'snowflake', ground: 'sky', version: '1.21.1', meta: '2 months ago · 3h', group: 'tests' },
  ];
  const LIB_TABS = [
    { id: 'all', label: 'All', count: 8 },
    { id: 'survival', label: 'Survival', count: 3 },
    { id: 'packs', label: 'Modpacks', count: 2 },
    { id: 'tests', label: 'Testing', count: 3 },
  ];
  let libNav = $state('library');
  let libTab = $state('all');
  let libSel = $state('a');
  const libShown = $derived(LIB.filter((d) => libTab === 'all' || d.group === libTab));
  const libCur = $derived(LIB.find((d) => d.id === libSel) || LIB[0]);
</script>

<!-- Snippets passed as Renderable props ---------- -->
{#snippet artHouse()}<InstanceIcon motif="house" ground="moss" size={32} framed={false} />{/snippet}
{#snippet artGear()}<InstanceIcon motif="gear" ground="amber" size={32} framed={false} />{/snippet}
{#snippet artPotion()}<InstanceIcon motif="potion" ground="rose" size={32} framed={false} />{/snippet}

{#snippet crashActions()}
  <Button variant="primary" icon="memory" onclick={() => did('Use 6 GB and relaunch')}>Use 6 GB and relaunch</Button>
  <Button icon="terminal" onclick={() => did('View log')}>View log</Button>
{/snippet}
{#snippet viewModsAction()}<Button size="sm">View mods</Button>{/snippet}
{#snippet updateAllAction()}<Button size="sm" icon="update">Update all</Button>{/snippet}
{#snippet crashText()}
  <p>Likely cause: not enough memory (<code>OutOfMemoryError</code>). The instance has 2 GB for 38 mods.</p>
{/snippet}
{#snippet crashTitle()}Friends SMP crashed after 3 minutes{/snippet}

{#snippet trashActions()}
  <Button variant="ghost" onclick={closeDialog}>Cancel</Button>
  <Button variant="danger" icon="trash" onclick={() => (did('Moved to trash'), closeDialog())}>Move to trash</Button>
{/snippet}
{#snippet renameActions()}
  <Button variant="ghost" onclick={closeDialog}>Cancel</Button>
  <Button variant="primary" icon="check" onclick={() => (did('Renamed to ' + renameValue), closeDialog())}>Rename</Button>
{/snippet}
{#snippet iconActions()}
  <Button variant="ghost" onclick={closeDialog}>Cancel</Button>
  <Button variant="primary" icon="check" onclick={closeDialog}>Use this icon</Button>
{/snippet}
{#snippet trashBody()}
  <p>The instance, its 2 worlds and its 14 screenshots can be restored from the trash for 30 days.</p>
  <p class="g-muted">1.2 GB will be freed when you empty the trash.</p>
{/snippet}

{#snippet logToolbar()}
  <SegmentedControl
    label="Level"
    bind:value={logLevel}
    options={[
      { value: 'all', label: 'All' },
      { value: 'info', label: 'Info' },
      { value: 'warn', label: 'Warnings' },
      { value: 'error', label: 'Errors' },
    ]}
  />
  <TextField label="Filter the log" hideLabel size="sm" icon="search" placeholder="Filter…" bind:value={logFilter} />
  <span class="g-spacer"></span>
  <Switch bind:checked={follow} label="Follow" />
  <IconButton icon="copy" label="Copy the log" variant="secondary" size="sm" />
  <IconButton icon="folder" label="Open the logs folder" size="sm" />
{/snippet}

{#snippet installAction()}<Button size="sm" variant="primary" icon="download">Install</Button>{/snippet}

<div class="g">
  <header class="g-top">
    <Logo size={32} />
    <nav class="g-nav text-small" aria-label="Sections">
      {#each SECTIONS as [id, label] (id)}<a href="#{id}">{label}</a>{/each}
    </nav>
    <span class="g-spacer"></span>
    <SegmentedControl
      label="Theme"
      value={theme}
      onChange={(v) => (theme = v as Theme)}
      options={[
        { value: 'dark', label: 'Night' },
        { value: 'light', label: 'Day' },
        { value: 'contrast', label: 'High contrast' },
      ]}
    />
    <Switch bind:checked={plain} label="Hyperlegible font" />
    <Switch bind:checked={rtl} label="Right to left" />
  </header>

  <p class="g-status text-small" role="status">Last action: <span class="text-body-strong">{lastAction}</span></p>

  <main class="g-main">
    <!-- ================= Brand ================= -->
    <section id="brand" class="g-section">
      <h2 class="text-title">Brand</h2>
      <div class="g-block">
        <h3 class="text-label">Logo</h3>
        <div class="g-row g-row--wide">
          <Logo size={64} />
          <Logo size={32} />
          <Logo size={48} wordmark={false} />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Icons ({ICON_NAMES.length})</h3>
        <div class="g-cells">
          {#each ICON_NAMES as n (n)}
            <div class="g-cell">
              <Icon name={n} size={27} />
              <span class="text-mono g-muted">{n}</span>
            </div>
          {/each}
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Icon sizes: 9, 18, 27, 36 (others snap: 20 → 18, 32 → 36)</h3>
        <div class="g-row">
          {#each [9, 18, 27, 36, 20, 32] as s (s)}<Icon name="cube" size={s} />{/each}
          <Icon name="chevron-right" size={18} label="Next (mirrored right to left)" />
          <Icon name="play" size={18} label="Play (never mirrored)" />
        </div>
      </div>
    </section>

    <!-- ================= Actions ================= -->
    <section id="actions" class="g-section">
      <h2 class="text-title">Actions</h2>
      <div class="g-block">
        <h3 class="text-label">PlayButton</h3>
        <div class="g-row">
          <PlayButton size="lg" detail="Friends SMP" onclick={() => did('Play Friends SMP')} />
          <PlayButton detail="1.21.1 · Fabric" />
          <PlayButton state="installing" {progress} detail="Java 21 · 312 MB" />
          <PlayButton state="running" detail="Playing · 00:42:10" />
          <PlayButton size="sm" />
          <PlayButton detail="Downloads 412 MB first" disabled />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Button</h3>
        <div class="g-row">
          <Button variant="primary" icon="plus" onclick={() => did('New instance')}>New instance</Button>
          <Button icon="import">Import from Prism</Button>
          <Button variant="ghost" icon="folder">Open folder</Button>
          <Button variant="danger" icon="trash">Move to trash</Button>
          <Button disabled icon="wrench">Repair</Button>
        </div>
        <div class="g-row">
          <Button size="sm" icon="copy">Copy</Button>
          <Button icon="search" kbd={['Ctrl', 'K']}>Search</Button>
          <Button variant="primary" size="lg" icon="check">Finish setup</Button>
          <Button iconEnd="chevron-right">Next</Button>
          <Button busy icon="update">Checking for updates</Button>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">IconButton (hover or focus for the tooltip)</h3>
        <div class="g-row">
          <IconButton icon="more" label="More actions" />
          <IconButton icon="folder" label="Open folder" />
          <IconButton icon="copy" variant="secondary" label="Duplicate" />
          <IconButton icon="play" variant="primary" size="lg" label="Play Friends SMP" />
          <IconButton icon="stop" variant="danger" label="Stop the game" />
          <IconButton icon="close" size="sm" label="Close" />
          <IconButton icon="wrench" variant="secondary" label="Repair" disabled />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Kbd</h3>
        <div class="g-row g-row--wide">
          {#each KBD_ROWS as [keys, label], i (i)}
            <span class="g-kbd text-small"><Kbd {keys} />{label}</span>
          {/each}
          <span class="g-kbd text-small"><Kbd>Esc</Kbd>Close (children)</span>
        </div>
      </div>
    </section>

    <!-- ================= Navigation ================= -->
    <section id="navigation" class="g-section">
      <h2 class="text-title">Navigation</h2>
      <div class="g-block">
        <h3 class="text-label">TitleBar</h3>
        <div class="g-bleed">
          <TitleBar {account} onSearch={() => did('Search')} onAccount={() => did('Account')} onMinimize={() => did('Minimize')} onMaximize={() => did('Maximize')} onClose={() => did('Close window')} />
        </div>
        <div class="g-bleed">
          <TitleBar {account}>
            <StatusTag state="running" detail="00:42" />
            <span class="text-body-strong">Friends SMP</span>
            <Button size="sm" icon="terminal">Log</Button>
            <Button size="sm" variant="danger" icon="stop">Stop</Button>
          </TitleBar>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Hotbar, collapsed and expanded</h3>
        <div class="g-hotbars">
          <Hotbar items={HOTBAR} footer={HOTBAR_FOOT} value={nav} onChange={(id) => (nav = id)} />
          <Hotbar items={HOTBAR} footer={HOTBAR_FOOT} value={nav} onChange={(id) => (nav = id)} expanded />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Tabs (arrows, Home, End)</h3>
        <Tabs items={TABS} bind:value={tab} label="Instance groups" />
      </div>
      <div class="g-block">
        <h3 class="text-label">CommandPalette (arrows move the active result, Enter or Ctrl Enter launches, Tab: more)</h3>
        <div class="g-center">
          <CommandPalette
            groups={paletteGroups}
            bind:activeId
            bind:query
            onSelect={picked}
            onMore={(item) => did('More actions for ' + item.label)}
          />
        </div>
        <div class="g-row">
          <Button icon="search" kbd={['Ctrl', 'K']} onclick={openPalette}>Open as a modal palette</Button>
        </div>
      </div>
    </section>

    <!-- ================= Status ================= -->
    <section id="status" class="g-section">
      <h2 class="text-title">Status</h2>
      <div class="g-block">
        <h3 class="text-label">StatusTag</h3>
        <div class="g-row g-row--tight">
          {#each STATES as s (s)}<StatusTag state={s} />{/each}
        </div>
        <div class="g-row g-row--tight">
          <StatusTag state="installing" detail="42%" />
          <StatusTag state="running" detail="00:42" />
          <StatusTag state="update">3 updates</StatusTag>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">MetaChip</h3>
        <div class="g-row g-row--tight">
          <MetaChip>1.21.1</MetaChip>
          <MetaChip>Fabric 0.16.5</MetaChip>
          <MetaChip>Quilt 0.27.1</MetaChip>
          <MetaChip tone="snapshot" icon="warning" title="Development version">24w45a</MetaChip>
          <MetaChip icon="mods">214</MetaChip>
          <MetaChip icon="memory">6 GB</MetaChip>
          <MetaChip tone="accent">Java 21</MetaChip>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">ProgressBar</h3>
        <div class="g-two">
          <ProgressBar value={42} label="Installing Friends SMP" detail="Libraries · 128 of 304 files" />
          <ProgressBar value={85} tone="diamond" label="Downloading Java 21" detail="178 MB of 210 MB · 12 s left" />
          <ProgressBar label="Checking files" detail="Preparing…" />
          <ProgressBar value={100} label="Mod updates" detail="14 mods up to date" />
          <ProgressBar value={progress} segments={16} />
          <ProgressBar tone="diamond" />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Avatar</h3>
        <div class="g-row">
          <Avatar src={SKIN} size={48} alt="Nyx_Ember’s skin" />
          <Avatar src={SKIN} size={32} />
          <Avatar src={SKIN} size={24} />
          <Avatar name="Guest" size={48} />
          <Avatar name="Lou" size={32} />
          <Avatar size={24} />
        </div>
      </div>
    </section>

    <!-- ================= Forms ================= -->
    <section id="forms" class="g-section">
      <h2 class="text-title">Forms</h2>
      <div class="g-block">
        <h3 class="text-label">TextField</h3>
        <div class="g-two">
          <TextField label="Instance name" bind:value={instanceName} hint="Shown in the library and in search." />
          <TextField label="Server address" bind:value={address} error="Address not found. Check the spelling or the port." />
          <TextField label="Search mods" icon="search" placeholder="Sodium, Iris, Create…" bind:value={search} />
          <TextField label="Java arguments" mono bind:value={javaArgs} suffix="JVM" />
          <TextField label="Maximum memory" type="number" value={6144} suffix="MB" size="sm" />
          <TextField label="Game directory" mono value="C:\Games\Cubby\instances\friends-smp" disabled />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Switch</h3>
        <div class="g-col">
          <Switch bind:checked={trackTime} label="Track play time" hint="Shown on every instance and in your stats." />
          <Switch bind:checked={snapshots} label="Show snapshots in the version list" />
          <Switch checked={false} disabled label="Automatic world backups (coming soon)" />
          <div class="g-row"><Switch checked label="Enable Sodium" hideLabel /><span class="text-small g-muted">hideLabel (named by aria-label)</span></div>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">SegmentedControl (arrows, Home, End)</h3>
        <div>
          <span class="cb-label text-body-strong">While the game is running</span>
          <SegmentedControl
            label="While the game is running"
            bind:value={whileRunning}
            options={[
              { value: 'hide', label: 'Minimize', icon: 'minimize' },
              { value: 'close', label: 'Close', icon: 'close' },
              { value: 'stay', label: 'Keep open', icon: 'maximize' },
            ]}
          />
        </div>
      </div>
      <div class="g-block g-narrow">
        <h3 class="text-label">Slider (native keys: arrows, Page Up, Page Down, Home, End)</h3>
        <Slider
          label="Memory"
          min={1024}
          max={16384}
          step={512}
          bind:value={memory}
          valueText={fmt.format(memory) + ' MB'}
          marks={['1 GB', '4 GB', '8 GB', '12 GB', '16 GB']}
          hint="Recommended for 214 mods: 6 to 8 GB. This PC has 32 GB."
        />
      </div>
    </section>

    <!-- ================= Instances ================= -->
    <section id="instances" class="g-section">
      <h2 class="text-title">Instances</h2>
      <div class="g-block">
        <h3 class="text-label">InstanceIcon: the {MOTIF_IDS.length} motifs on their own ground</h3>
        <div class="g-motifs">
          {#each MOTIF_IDS as m (m)}
            <div>
              <InstanceIcon motif={m} size={64} raised />
              <div class="text-small g-muted g-cap">{MOTIF_LABELS[m]}</div>
            </div>
          {/each}
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Sizes, then the {GROUND_IDS.length} grounds</h3>
        <div class="g-row g-row--end">
          {#each [16, 32, 48, 64, 96] as s (s)}
            <div class="g-stack">
              <InstanceIcon motif="tree" size={s} raised={s >= 48} />
              <span class="text-small g-muted g-cap">{s} px</span>
            </div>
          {/each}
          <span class="g-gap"></span>
          {#each GROUND_IDS as g (g)}
            <div class="g-stack">
              <InstanceIcon motif="tree" ground={g} size={48} />
              <span class="text-small g-muted g-cap">{GROUND_LABELS[g]}</span>
            </div>
          {/each}
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Picked from the instance name (seed), unframed, with a label, imported image</h3>
        <div class="g-row g-row--end">
          {#each SEEDS as seed (seed)}
            {@const p = pickIcon(seed)}
            <div class="g-stack">
              <InstanceIcon {seed} size={48} />
              <span class="text-small g-muted g-cap">{seed}</span>
              <span class="text-mono g-muted">{p.motif} · {p.ground}</span>
            </div>
          {/each}
          <div class="g-stack">
            <InstanceIcon motif="star" size={48} framed={false} />
            <span class="text-small g-muted g-cap">framed false</span>
          </div>
          <div class="g-stack">
            <InstanceIcon motif="heart" ground="night" size={48} label="Friends SMP icon" />
            <span class="text-small g-muted g-cap">label</span>
          </div>
          <div class="g-stack">
            <InstanceIcon src={SERVER_ICON} size={48} raised />
            <span class="text-small g-muted g-cap">src</span>
          </div>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">IconPicker</h3>
        <IconPicker
          name="Chill Skyblock"
          bind:motif={pickMotif}
          bind:ground={pickGround}
          bind:src={pickSrc}
          onImport={() => (pickSrc = SERVER_ICON)}
        />
        <p class="text-mono g-muted">{JSON.stringify({ motif: pickMotif, ground: pickGround, src: pickSrc ? 'data:…' : pickSrc })}</p>
      </div>
      <div class="g-block">
        <h3 class="text-label">InstanceTile in every status (click selects, double click or the corner button plays)</h3>
        <div class="g-tiles">
          {#each TILES as { id, ...t } (id)}
            <InstanceTile {...t} selected={tileSel === id} onSelect={() => (tileSel = id)} onPlay={() => did('Play ' + t.name)} />
          {/each}
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">InstanceRow</h3>
        <div class="g-boxed">
          <InstanceRow head />
          {#each ROWS as { id, ...r } (id)}
            <InstanceRow {...r} selected={rowSel === id} onSelect={() => (rowSel = id)} onPlay={() => did('Play ' + r.name)} />
          {/each}
        </div>
      </div>
    </section>

    <!-- ================= Content ================= -->
    <section id="content" class="g-section">
      <h2 class="text-title">Content</h2>
      <div class="g-block">
        <h3 class="text-label">ModRow</h3>
        <div class="g-boxed">
          <ModRow name="Sodium" version="0.6.0" authors="by CaffeineMC" description="A modern rendering engine that greatly improves performance and stability." bind:enabled={mods.sodium} onMore={() => did('Actions for Sodium')} />
          <ModRow name="Iris Shaders" version="1.7.3" authors="by coderbot, IMS" description="A shader loader that works with OptiFine shader packs." bind:enabled={mods.iris} update="1.8.0 available" />
          <ModRow name="Create" version="0.5.1j" authors="by simibubi" description="Gears, belts and machines to automate your world." bind:enabled={mods.create} warning="May not work with Sodium 0.6" />
          <ModRow name="OptiFine" version="HD U J1" authors="by sp614x" description="Disabled: replaced by Sodium and Iris." bind:enabled={mods.optifine} />
          <ModRow name="Lithium" version="0.13.0" authors="by CaffeineMC" description="Discover: the same row with an Install button in place of the menu." iconSrc={SERVER_ICON} bind:enabled={mods.lithium} actions={installAction} />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">ServerRow</h3>
        <div class="g-col">
          <ServerRow name="The Crew" address="crew.example:25565" iconSrc={SERVER_ICON} motd={'Season 4 · back on Saturday\nLive map on the website'} players={{ online: 5, max: 20 }} ping={24} instance="Friends SMP" onJoin={() => did('Join The Crew')} />
          <ServerRow name="Hypixel-like test" address="mc.example.net" motd="A busy server, 3 bars" players={{ online: 1204, max: 5000 }} ping={120} />
          <ServerRow name="Create Factory" address="play.createfactory.example" motd="Requires the Create: Above and Beyond modpack" players={{ online: 87, max: 100 }} ping={212} instance="Create: Above and Beyond" />
          <ServerRow name="Far away" address="au.example.org" motd="Ping above 300 ms" ping={420} />
          <ServerRow name="Old test server" address="192.168.1.42" online={false} instance="Snapshot 24w45a" />
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">LogViewer</h3>
        <LogViewer lines={shownLog} height={300} live toolbar={logToolbar} />
      </div>
    </section>

    <!-- ================= Feedback ================= -->
    <section id="feedback" class="g-section">
      <h2 class="text-title">Feedback</h2>
      <div class="g-block">
        <h3 class="text-label">Banner in each tone</h3>
        <div class="g-col">
          <Banner tone="danger" title={crashTitle} actions={crashActions}>{@render crashText()}</Banner>
          {#if !dismissed.warning}
            <Banner tone="warning" title="Two mods may not work together" onDismiss={() => (dismissed.warning = true)} actions={viewModsAction}>Iris 1.7.3 needs Sodium 0.6.1 or newer.</Banner>
          {/if}
          <Banner tone="info" title="14 mod updates available" actions={updateAllAction}>Create: Above and Beyond · checked 5 min ago</Banner>
          {#if !dismissed.success}
            <Banner tone="success" title="Instances imported from Prism Launcher" onDismiss={() => (dismissed.success = true)}>9 instances, 3 groups and 2 offline accounts imported.</Banner>
          {/if}
          <Banner tone="info" icon="download" title="Title only, custom icon" />
          {#if dismissed.warning || dismissed.success}
            <div><Button size="sm" variant="ghost" onclick={() => (dismissed = { warning: false, success: false })}>Show dismissed banners</Button></div>
          {/if}
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Dialog, inline previews</h3>
        <div class="g-dialogs">
          <div class="g-dialog-box">
            <Dialog inline tone="danger" icon="trash" title="Move “Hardcore 1.8 PvP” to the trash?" onClose={() => {}} actions={trashActions}>{@render trashBody()}</Dialog>
          </div>
          <div class="g-dialog-box">
            <Dialog inline icon="info" tone="info" title="Rename instance" onClose={() => {}} actions={renameActions}>
              <TextField label="Instance name" value="Hardcore 1.8 PvP" hint="Shown in the library and in search." />
            </Dialog>
          </div>
        </div>
      </div>
      <div class="g-block">
        <h3 class="text-label">Dialog, modal (focus moves in, Tab stays inside, Esc closes, focus returns)</h3>
        <div class="g-row">
          <Button variant="danger" icon="trash" onclick={() => (dialog = 'trash')}>Move to trash…</Button>
          <Button icon="wrench" onclick={() => (dialog = 'rename')}>Rename…</Button>
          <Button icon="cube" onclick={() => (dialog = 'icon')}>Change icon… (wide)</Button>
        </div>
      </div>
    </section>

    <!-- ================= Library screen ================= -->
    <section id="screen" class="g-section">
      <h2 class="text-title">Library screen, 1280 × 800</h2>
      <div class="g-screen-scroll">
        <div class="g-screen">
          <TitleBar {account} />
          <div class="g-screen__body">
            <Hotbar items={HOTBAR.map((it) => (it.id === 'downloads' ? { ...it, badge: 1 } : it))} footer={HOTBAR_FOOT} value={libNav} onChange={(id) => (libNav = id)} />
            <main class="g-screen__main">
              <div class="g-screen__head">
                <h1 class="text-title">Library</h1>
                <span class="g-spacer"></span>
                <Button icon="import">Import</Button>
                <Button variant="primary" icon="plus" kbd={['Ctrl', 'N']}>New instance</Button>
              </div>
              <Tabs label="Groups" bind:value={libTab} items={LIB_TABS} />
              <div class="g-tiles">
                {#each libShown as { id, group: _group, ...d } (id)}
                  <InstanceTile {...d} selected={id === libSel} onSelect={() => (libSel = id)} />
                {/each}
              </div>
            </main>
            <aside class="g-screen__aside" aria-label="Instance details">
              <div class="g-screen__who">
                <InstanceIcon motif={libCur.motif} ground={libCur.ground} size={64} raised />
                <div>
                  <h2 class="text-heading">{libCur.name}</h2>
                  <p class="text-small g-muted"><Content value={libCur.meta} /></p>
                </div>
              </div>
              <PlayButton
                size="lg"
                state={libCur.status === 'installing' ? 'installing' : 'ready'}
                progress={libCur.progress}
                detail={libCur.status === 'installing' ? 'Libraries · 128 / 304' : libCur.status === 'download' ? 'Downloads 412 MB first' : 'Last world: Lake Village'}
                style="width: 100%"
              />
              <div class="g-screen__actions">
                <Button size="sm" icon="offline">Offline</Button>
                <Button size="sm" icon="servers">Server</Button>
                <Button size="sm" icon="folder">Folder</Button>
                <Button size="sm" icon="more">More</Button>
              </div>
              <dl class="g-screen__facts">
                <dt class="text-small g-muted">Version</dt>
                <dd><MetaChip>{libCur.version}</MetaChip></dd>
                <dt class="text-small g-muted">Loader</dt>
                <dd class="text-mono">{libCur.loader ? libCur.loader + ' 0.16.5' : 'None'}</dd>
                <dt class="text-small g-muted">Content</dt>
                <dd class="text-body">38 mods · 4 worlds · 112 screenshots</dd>
                <dt class="text-small g-muted">On disk</dt>
                <dd class="text-mono">2.4 GB</dd>
              </dl>
              <div class="g-screen__foot">
                <ProgressBar value={42} tone="diamond" label="Create: Above and Beyond" detail="1 download running · 18 MB/s" />
              </div>
            </aside>
          </div>
        </div>
      </div>
    </section>
  </main>
</div>

<!-- Modal layers ---------- -->
{#if paletteOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="cb-dialog-layer" bind:this={paletteHost} onclick={(e) => e.target === e.currentTarget && closePalette()}>
    <CommandPalette
      modal
      idBase="g-modal-cmd"
      groups={paletteGroups}
      bind:activeId
      bind:query
      onSelect={(item, opts) => (picked(item, opts), closePalette())}
      onClose={closePalette}
    />
  </div>
{/if}

{#if dialog === 'trash'}
  <Dialog tone="danger" icon="trash" title="Move “Hardcore 1.8 PvP” to the trash?" onClose={closeDialog} actions={trashActions}>{@render trashBody()}</Dialog>
{:else if dialog === 'rename'}
  <Dialog title="Rename instance" onClose={closeDialog} actions={renameActions}>
    <TextField label="Instance name" bind:value={renameValue} hint="Shown in the library and in search." />
  </Dialog>
{:else if dialog === 'icon'}
  <Dialog wide title="Change icon" onClose={closeDialog} actions={iconActions}>
    <IconPicker name="Chill Skyblock" bind:motif={pickMotif} bind:ground={pickGround} bind:src={pickSrc} onImport={() => (pickSrc = SERVER_ICON)} />
  </Dialog>
{/if}

<Tooltips />

<style>
  .g {
    min-height: 100vh;
  }
  .g-top {
    position: sticky;
    top: 0;
    z-index: var(--layer-sticky);
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-5);
    padding: var(--space-3) var(--space-6);
    background: var(--bg-chrome);
    border-bottom: var(--border-chrome) solid var(--edge);
  }
  .g-nav {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-3);
  }
  .g-nav a {
    color: var(--ink-muted);
  }
  .g-nav a:hover {
    color: var(--ink);
  }
  .g-status {
    margin: 0;
    padding: var(--space-2) var(--space-6);
    color: var(--ink-muted);
    border-bottom: var(--border-hair) solid var(--line);
  }
  .g-main {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    max-width: 1340px;
    padding: var(--space-6);
  }
  .g-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    scroll-margin-top: 80px;
  }
  .g-section h2,
  .g-section h3 {
    margin: 0;
  }
  .g-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .g-block > h3 {
    color: var(--ink-muted);
  }
  .g-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
  }
  .g-row--tight {
    gap: var(--space-3);
  }
  .g-row--wide {
    gap: var(--space-8);
  }
  .g-row--end {
    align-items: flex-end;
  }
  .g-col {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .g-two {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-6) var(--space-8);
  }
  .g-narrow {
    max-width: 560px;
  }
  .g-spacer {
    flex: 1;
  }
  .g-gap {
    width: var(--space-6);
  }
  .g-muted {
    color: var(--ink-muted);
  }
  .g-cap {
    margin-top: var(--space-2);
    text-align: center;
  }
  .g-stack {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .g-cells {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
    gap: var(--space-2);
  }
  .g-cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-1);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .g-kbd {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--ink-muted);
  }
  .g-bleed {
    margin-inline: calc(-1 * var(--space-6));
  }
  .g-hotbars {
    display: flex;
    gap: 64px;
    height: 520px;
  }
  .g-center {
    display: flex;
    justify-content: center;
  }
  .g-motifs {
    display: grid;
    grid-template-columns: repeat(auto-fill, 80px);
    gap: var(--space-5) var(--space-4);
  }
  .g-tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
    gap: var(--space-5);
  }
  .g-boxed {
    border: var(--border-block) solid var(--edge);
    box-shadow: var(--shadow-block);
  }
  .g-dialogs {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
    gap: var(--space-5);
  }
  .g-dialog-box {
    height: 380px;
  }
  .g-screen-scroll {
    overflow: auto;
    border: var(--border-block) solid var(--edge);
  }
  .g-screen {
    display: flex;
    flex-direction: column;
    width: 1280px;
    height: 800px;
    background: var(--bg);
  }
  .g-screen__body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .g-screen__main {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
    padding: var(--space-6);
    overflow: auto;
  }
  .g-screen__head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .g-screen__head h1 {
    margin: 0;
  }
  .g-screen__aside {
    box-sizing: border-box;
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-4);
    width: var(--size-inspector);
    padding: var(--space-6) var(--space-5);
    background: var(--bg-chrome);
    border-inline-start: var(--border-chrome) solid var(--edge);
  }
  .g-screen__who {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .g-screen__who > div {
    min-width: 0;
  }
  .g-screen__who h2,
  .g-screen__who p {
    margin: 0;
  }
  .g-screen__who p {
    margin-top: 2px;
  }
  .g-screen__actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-2);
  }
  .g-screen__facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    margin: 0;
    padding: var(--space-4);
    background: var(--surface);
    border: var(--border-block) solid var(--edge);
  }
  .g-screen__facts dd {
    margin: 0;
  }
  .g-screen__foot {
    margin-top: auto;
  }
</style>
