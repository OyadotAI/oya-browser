/**
 * The shell page's composition root: builds every ViewModel once, in
 * dependency order, and wires the small interfaces one feature declares for
 * another (the start page asks through Ask, the palette presses the studio's
 * record button, the studio's record button goes through the control bar).
 * Views receive these instances; nothing else constructs a ViewModel.
 */
import type { OyaBrowser } from '../core/bridge.ts';
import { ShellViewModel } from './shell-view-model.ts';
import { PanelViewModel, type FrameClock } from './panel/panel-view-model.ts';
import { AskViewModel } from '../features/ask/index.ts';
import {
  ThemeViewModel,
  AgentActivityViewModel,
  BackdropViewModel,
  LaunchViewModel,
  systemDarkAtStart,
} from '../features/chrome/index.ts';
import { reducedMotion } from '../hooks/index.ts';
import { TabStripViewModel, TabDragViewModel, TabCardViewModel } from '../features/tabs/index.ts';
import { ToolbarViewModel } from '../features/toolbar/index.ts';
import { StartViewModel } from '../features/start/index.ts';
import { ActionsViewModel, NetLogViewModel, SourceViewModel } from '../features/inspect/index.ts';
import { RoutinesViewModel } from '../features/routines/index.ts';
import { StudioViewModel, type RecordingGate } from '../features/studio/index.ts';
import { ControlViewModel, blockedActions } from '../features/control/index.ts';
import {
  AccountViewModel,
  SyncViewModel,
  ImportViewModel,
  ProfileViewModel,
  ShellDialogViewModel,
  SetupViewModel,
  ReconnectViewModel,
  UpdatesViewModel,
  PaletteViewModel,
  ConnectionPillViewModel,
} from '../features/connection/index.ts';

/** What the page gives the root: the bridge, its clock and clipboard, and the platform. */
export interface PageEnvironment {
  /** window.oyaBrowser. */
  bridge: OyaBrowser;
  /** requestAnimationFrame. */
  frames: FrameClock;
  /** navigator.clipboard. */
  clipboard: Clipboard;
  /** navigator.platform, for shortcut labels. */
  platform: string;
}

/** Start recording through the control bar: refused while watch-only unless control can be taken first. */
function recordingGate(control: ControlViewModel): RecordingGate {
  return {
    blocked: () => !!control.state.control && blockedActions(control.state).recordBlocked,
    admit: async () => control.guard(true) === 'allow' || (control.guard(true) === 'take' && control.acquire()),
  };
}

/** Focuses an element of another feature's view by id, once the frame that shows it has painted. */
const focusById = (frames: FrameClock, id: string) => () =>
  void frames.request(() => document.getElementById(id)?.focus());

/** Every ViewModel of the shell page. */
export class ShellViewModels {
  /** The page's environment. */
  readonly env: PageEnvironment;
  /** Connected, mode, start page, recording. */
  declare readonly shell: ShellViewModel;
  /** The workspace panel. */
  declare readonly panel: PanelViewModel;
  /** Who drives the browser. */
  declare readonly control: ControlViewModel;
  /** The Ask pane. */
  declare readonly ask: AskViewModel;
  /** The workflow studio. */
  declare readonly studio: StudioViewModel;
  /** Theme, activity, backdrop and launch. */
  declare readonly chrome: ChromeModels;
  /** The tab strip, its drag and its hover card. */
  declare readonly tabs: TabModels;
  /** The navigation toolbar. */
  declare readonly toolbar: ToolbarViewModel;
  /** The start page. */
  declare readonly start: StartViewModel;
  /** The Inspect panes. */
  declare readonly inspect: InspectModels;
  /** The Routines pane. */
  declare readonly routines: RoutinesViewModel;
  /** The welcome screen, connection, account dialog and command palette. */
  declare readonly connection: ConnectionModels;

  /** Builds them all over `env`, in the order they need each other. */
  constructor(env: PageEnvironment) {
    this.env = env;
    Object.assign(this, coreModels(env));
    Object.assign(this, featureModels(this));
    this.connection = connectionModels(env, this);
  }

  /** Stops every subscription and timer. */
  dispose(): void {
    const all = [this.shell, this.panel, this.control, this.ask, this.studio, this.toolbar, this.start, this.routines];
    const grouped = [this.chrome, this.tabs, this.inspect, this.connection].flatMap((group) => Object.values(group));
    for (const vm of [...all, ...grouped]) vm.dispose();
  }
}

/** The ViewModels every feature may use: the shell, the panel, and who drives. */
function coreModels({ bridge, frames }: PageEnvironment): Pick<ShellViewModels, 'shell' | 'panel' | 'control'> {
  return {
    shell: new ShellViewModel(bridge),
    panel: new PanelViewModel(bridge, frames),
    control: new ControlViewModel(bridge),
  };
}

/** The keys `featureModels` builds. */
type FeatureKeys = 'ask' | 'studio' | 'chrome' | 'tabs' | 'toolbar' | 'start' | 'inspect' | 'routines';

/** The panel's tools and the start page, which asks through Ask. */
function toolModels(root: ShellViewModels): Pick<ShellViewModels, 'ask' | 'studio' | 'start' | 'routines'> {
  const { env, shell, panel, control } = root;
  const { bridge, clipboard } = env;
  const ask = new AskViewModel({ bridge, panel });
  const asker = { ask: (text: string) => ask.ask(text), busy: () => ask.state.sending };
  const studio = new StudioViewModel({ bridge, shell, panel, gate: recordingGate(control), clipboard });
  const routines = new RoutinesViewModel({ bridge, panel });
  return { ask, studio, routines, start: new StartViewModel({ bridge, shell, asker }) };
}

/** Every feature but the connection, over the core ViewModels. */
function featureModels(root: ShellViewModels): Pick<ShellViewModels, FeatureKeys> {
  const { env, panel } = root;
  const chrome = chromeModels(env);
  const toolbar = new ToolbarViewModel(env.bridge);
  return { ...toolModels(root), chrome, toolbar, tabs: tabModels(env), inspect: inspectModels(env, panel) };
}

/** Theme, the agent's activity, the page backdrop and the launch. */
export interface ChromeModels {
  /** System, light or dark. */
  theme: ThemeViewModel;
  /** Whether the agent is at work. */
  activity: AgentActivityViewModel;
  /** The page still behind a dialog. */
  backdrop: BackdropViewModel;
  /** The launch animation. */
  launch: LaunchViewModel;
}

/** The chrome around the page. */
function chromeModels({ bridge, frames }: PageEnvironment): ChromeModels {
  return {
    theme: new ThemeViewModel({ bridge, frames, systemDark: systemDarkAtStart() }),
    activity: new AgentActivityViewModel(bridge),
    backdrop: new BackdropViewModel({ bridge, frames }),
    launch: new LaunchViewModel(reducedMotion()),
  };
}

/** The tab strip and the two that work with it. */
export interface TabModels {
  /** The hover card. */
  card: TabCardViewModel;
  /** Dragging a tab. */
  drag: TabDragViewModel;
  /** The strip. */
  strip: TabStripViewModel;
}

/** The tab strip, in the order its parts need each other. */
function tabModels({ bridge, frames }: PageEnvironment): TabModels {
  const card = new TabCardViewModel();
  const drag = new TabDragViewModel({ bridge, frames, card });
  return { card, drag, strip: new TabStripViewModel({ bridge, card, drag, reducedMotion }) };
}

/** The Inspect panes. */
export interface InspectModels {
  /** Actions run by hand. */
  actions: ActionsViewModel;
  /** The activity log. */
  netLog: NetLogViewModel;
  /** The page's source. */
  source: SourceViewModel;
}

/** The Inspect panes over the shared panel. */
function inspectModels({ bridge, clipboard }: PageEnvironment, panel: PanelViewModel): InspectModels {
  const services = { bridge, panel, clipboard };
  return {
    actions: new ActionsViewModel(services),
    netLog: new NetLogViewModel(services),
    source: new SourceViewModel(services),
  };
}

/** The welcome screen, connection, account dialog and command palette. */
export interface ConnectionModels {
  /** The account card and its actions. */
  account: AccountViewModel;
  /** Sync now. */
  sync: SyncViewModel;
  /** Imported logins. */
  imports: ImportViewModel;
  /** Browsing as, and this device. */
  profile: ProfileViewModel;
  /** The commands and account dialog. */
  dialog: ShellDialogViewModel;
  /** The welcome screen. */
  setup: SetupViewModel;
  /** Connection settings. */
  reconnect: ReconnectViewModel;
  /** The updater. */
  updates: UpdatesViewModel;
  /** The command palette. */
  palette: PaletteViewModel;
  /** The connection pill. */
  pill: ConnectionPillViewModel;
}

/** The account page's parts, then the dialog over them. */
function accountModels({ bridge, clipboard }: PageEnvironment) {
  const parts = {
    account: new AccountViewModel(bridge),
    sync: new SyncViewModel(bridge),
    imports: new ImportViewModel(bridge),
    profile: new ProfileViewModel(bridge),
  };
  return { ...parts, dialog: new ShellDialogViewModel({ bridge, clipboard, ...parts }) };
}

/** What the command palette asks of other features: the address bar, the Ask box, the record button. */
function paletteHost(root: ShellViewModels) {
  return {
    focusAddress: () => root.toolbar.focusAddress(),
    focusChat: focusById(root.env.frames, 'chat-input'),
    recordButton: () => void root.studio.actions.recordShortcut(),
  };
}

/** The connection feature, wired to the panel, the studio and the toolbar. */
function connectionModels(env: PageEnvironment, root: ShellViewModels): ConnectionModels {
  const { bridge, platform } = env;
  const { shell, panel } = root;
  const { dialog, ...parts } = accountModels(env);
  const setup = new SetupViewModel({ bridge, shell });
  const [reconnect, updates] = [new ReconnectViewModel({ bridge, shell, dialog, setup }), new UpdatesViewModel(bridge)];
  const palette = new PaletteViewModel({ bridge, panel, shell, dialog, updates, host: paletteHost(root), platform });
  const pill = new ConnectionPillViewModel({ bridge, shell, account: dialog });
  return { ...parts, dialog, setup, reconnect, updates, palette, pill };
}
