/**
 * The shell page's composition root: builds every ViewModel once, in
 * dependency order, and wires the small interfaces one feature declares for
 * another (the start page asks through Ask, the palette presses the studio's
 * record button, the studio's record button goes through the control bar).
 * Views receive these instances; nothing else constructs a ViewModel.
 */
import { ChecklistViewModel } from '../features/connection/view-models/checklist-view-model.ts';
import { RendererConstants as C } from '../core/constants.ts';
import { NotificationsViewModel } from '../features/notifications/index.ts';
import { ProfilesViewModel } from '../features/profiles/index.ts';
import type { OyaBrowser } from '../core/bridge.ts';
import { ShellViewModel } from './shell-view-model.ts';
import { PanelViewModel, type FrameClock } from './panel/panel-view-model.ts';
import { PlaybooksViewModel } from '../features/playbooks/index.ts';
import { AskSessions, PersonaViewModel } from '../features/ask/index.ts';
import {
  ThemeViewModel,
  AgentActivityViewModel,
  BackdropViewModel,
  LaunchViewModel,
  systemDarkAtStart,
} from '../features/chrome/index.ts';
import { reducedMotion } from '../hooks/index.ts';
import { TabStripViewModel, TabDragViewModel, TabCardViewModel, type TabDragGate } from '../features/tabs/index.ts';
import { ToolbarViewModel, type TranslateHost } from '../features/toolbar/index.ts';
import { StartViewModel } from '../features/start/index.ts';
import { ActionsViewModel, NetLogViewModel, SourceViewModel } from '../features/inspect/index.ts';
import { RoutinesViewModel } from '../features/routines/index.ts';
import { StudioViewModel, type RecordingGate } from '../features/studio/index.ts';
import { ControlViewModel, blockedActions, CONTROL_ACTION_ID } from '../features/control/index.ts';
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
  AgentsViewModel,
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

/** Any tab but an agent's moves while agents work in their own tabs; a refused drag focuses Take control, as a refused button does. */
function tabDragGate(control: ControlViewModel, frames: FrameClock): TabDragGate {
  return {
    movable: (agent) =>
      !control.state.control ||
      !!control.state.control.interactive ||
      (!agent && !blockedActions(control.state).newTabBlocked),
    refuse: focusById(frames, CONTROL_ACTION_ID),
  };
}

/** Focuses an element of another feature's view by id, once the frame that shows it has painted. */
const focusById = (frames: FrameClock, id: string) => () =>
  void frames.request(() => document.getElementById(id)?.focus());

/** Every ViewModel of the shell page. */
export class ShellViewModels {
  /** Session notification inbox. */
  readonly notifications: NotificationsViewModel;
  /** The page's environment. */
  readonly env: PageEnvironment;
  /** Connected, mode, start page, recording. */
  declare readonly shell: ShellViewModel;
  /** The workspace panel. */
  declare readonly panel: PanelViewModel;
  /** Who drives the browser. */
  declare readonly control: ControlViewModel;
  /** The Ask pane. */
  declare readonly ask: AskSessions;
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
  /** Saved project playbooks and replay. */
  declare readonly playbooks: PlaybooksViewModel;
  /** The welcome screen, connection, account dialog and command palette. */
  declare readonly connection: ConnectionModels;

  /** Builds them all over `env`, in the order they need each other. */
  constructor(env: PageEnvironment) {
    this.env = env;
    this.notifications = new NotificationsViewModel(env.bridge);
    Object.assign(this, coreModels(env));
    Object.assign(this, featureModels(this));
    this.playbooks = new PlaybooksViewModel(env.bridge, this.panel);
    this.connection = connectionModels(env, this);
  }

  /** Stops every subscription and timer. */
  dispose(): void {
    const all = [this.shell, this.panel, this.control, this.ask, this.studio];
    const tools = [this.notifications, this.toolbar, this.start, this.routines, this.playbooks];
    const grouped = [this.chrome, this.tabs, this.inspect, this.connection].flatMap((group) => Object.values(group));
    for (const vm of [...all, ...tools, ...grouped]) vm.dispose();
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
  const ask = new AskSessions({ bridge, panel });
  const asker = { ask: (text: string) => ask.ask(text), busy: () => ask.busy() };
  const studio = new StudioViewModel({ bridge, shell, panel, gate: recordingGate(control), clipboard });
  const routines = new RoutinesViewModel({ bridge, panel });
  return { ask, studio, routines, start: new StartViewModel({ bridge, shell, asker }) };
}

/** Every feature but the connection, over the core ViewModels. */
function featureModels(root: ShellViewModels): Pick<ShellViewModels, FeatureKeys> {
  const { env, panel, control } = root;
  const chrome = chromeModels(env);
  const tools = toolModels(root);
  const toolbar = new ToolbarViewModel(env.bridge, translateHost(root, tools.ask));
  const tabs = tabModels(env, tabDragGate(control, env.frames));
  return { ...tools, chrome, toolbar, tabs, inspect: inspectModels(env, panel) };
}

/** Where the translate chip's setup clicks go: the Assistant panel's sign-in and model cards, and the claim page. */
function translateHost({ env, panel }: ShellViewModels, ask: ShellViewModels['ask']): TranslateHost {
  return {
    openAsk: () => void panel.open('chat'),
    openModels: () => void panel.open('chat').then(() => ask.state.current.model.openCard()),
    openClaim: () => void env.bridge.openClaim(),
  };
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
    launch: new LaunchViewModel(reducedMotion() || document.documentElement.dataset.secondary === 'true'),
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
function tabModels({ bridge, frames }: PageEnvironment, gate: TabDragGate): TabModels {
  const card = new TabCardViewModel();
  const drag = new TabDragViewModel({ bridge, frames, card, gate });
  const strip = new TabStripViewModel({ bridge, card, drag, reducedMotion, refused: gate.refuse });
  return { card, drag, strip };
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
  /** The first-run checklist. */
  checklist: ChecklistViewModel;
  /** Connection settings. */
  reconnect: ReconnectViewModel;
  /** The updater. */
  updates: UpdatesViewModel;
  /** The command palette. */
  palette: PaletteViewModel;
  /** The connection pill. */
  pill: ConnectionPillViewModel;
  /** The coding agents the footer connects to Oya's MCP port. */
  agents: AgentsViewModel;
  /** The profile circle's card: the browsing profiles and Settings. */
  profiles: ProfilesViewModel;
}

/** The account page's parts, then the dialog over them. */
function accountModels({ bridge, clipboard }: PageEnvironment, busy: () => boolean) {
  const parts = {
    account: new AccountViewModel(bridge),
    sync: new SyncViewModel(bridge),
    imports: new ImportViewModel(bridge),
    profile: new ProfileViewModel(bridge),
  };
  const dialog = new ShellDialogViewModel({ bridge, clipboard, ...parts });
  return connectImportOffer(parts, dialog, busy);
}

/** Opens the first-run offer once no agent is working, so it never lands on a person's first task. */
function offerWhenIdle(busy: () => boolean, open: () => void): void {
  if (!busy()) return open();
  setTimeout(() => offerWhenIdle(busy, open), C.IMPORT_OFFER_RETRY_MS);
}

/** Wire the first-login offer after both collaborators exist. */
function connectImportOffer<T extends { /** Login import invitation. */ imports: ImportViewModel }>(
  parts: T,
  dialog: ShellDialogViewModel,
  busy: () => boolean,
) {
  parts.imports.onOffer = () => offerWhenIdle(busy, () => void dialog.open(true));
  parts.imports.onDecline = () => dialog.close();
  return { ...parts, dialog };
}

/** What the command palette asks of other features: the address bar, the Ask box, the record button. */
function paletteHost(root: ShellViewModels) {
  return {
    focusAddress: () => root.toolbar.focusAddress(),
    openNotifications: () => root.notifications.open(),
    openModels: () => void root.panel.open('chat').then(() => root.ask.state.current.model.openCard()),
    focusChat: focusById(root.env.frames, 'chat-input'),
    recordButton: () => void root.studio.actions.recordShortcut(),
  };
}

/** The toolbar's end: the connection pill, the coding agents, and the profile circle's card (whose Settings opens the account page, as the circle used to). */
function toolbarModels(bridge: PageEnvironment['bridge'], shell: ShellViewModel, dialog: ShellDialogViewModel) {
  const pill = new ConnectionPillViewModel({ bridge, shell, account: dialog });
  const persona = new PersonaViewModel(bridge);
  const profiles = new ProfilesViewModel({ bridge, persona, openSettings: () => pill.click() });
  return { pill, agents: new AgentsViewModel(bridge), profiles };
}

/** The connection feature, wired to the panel, the studio and the toolbar. */
function connectionModels(env: PageEnvironment, root: ShellViewModels): ConnectionModels {
  const { bridge, platform } = env;
  const { shell, panel } = root;
  const { dialog, ...parts } = accountModels(env, () => root.ask.anyBusy());
  const [setup, checklist] = [new SetupViewModel({ bridge, shell }), new ChecklistViewModel(bridge)];
  const [reconnect, updates] = [new ReconnectViewModel({ bridge, shell, dialog, setup }), new UpdatesViewModel(bridge)];
  const palette = new PaletteViewModel({ bridge, panel, shell, dialog, updates, host: paletteHost(root), platform });
  const toolbar = toolbarModels(bridge, shell, dialog);
  return { ...parts, dialog, setup, checklist, reconnect, updates, palette, ...toolbar };
}
