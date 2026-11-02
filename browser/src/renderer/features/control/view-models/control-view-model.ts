/**
 * The toolbar's control status: who is driving this browser (the agent, you,
 * another operator), the Take control / Release and Resume buttons, and the
 * watch-only guard that refuses page actions until you hold control. Start
 * recording is the exception: recording needs a person's hands on the page,
 * so pressing it takes control first, then records.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ControlChange } from '../../../../shared/ipc.ts';
import { CONTROL_LABELS, OFFLINE_MODE, RESUME_TITLE, TEXT } from '../model/constants.ts';

/** Who drives, as control-state.cjs `snapshot()` reports it (the fields the bar reads). */
export interface ControlSnapshot {
  /** agent, human, paused, offline, disconnected, unavailable. */
  mode?: string;
  /** This desktop holds the control the mode names. */
  mine?: boolean;
  /** A person may use the page right now. */
  interactive?: boolean;
  /** The person may use the tab this window shows (any but the agent's while it drives one). */
  pageInteractive?: boolean;
  /** The person may open a new tab (beside agents' tabs, always). */
  newTabInteractive?: boolean;
  /** A handoff is under way. */
  busy?: boolean;
  /** Which handoff ('return' while handing back). */
  busyAction?: string;
  /** This desktop is taking control. */
  taking?: boolean;
  /** The server supports handoffs. */
  supported?: boolean;
  /** Local automation clients attached. */
  localClients?: number;
  /** Local automation is on. */
  local?: boolean;
  /** Changes with every handoff. */
  revision?: number;
  /** The control socket is up. */
  connected?: boolean;
}

/** Which of the bar's buttons is waiting on a change. */
export type Pending = '' | 'action' | 'resume';

/** What the bar knows. */
export interface ControlState {
  /** The last control state, or null before the first. */
  control: ControlSnapshot | null;
  /** The last change's error, shown until the state moves on. */
  error: string;
  /** The button whose change is under way. */
  pending: Pending;
}

/** What the bar shows. */
export interface ControlBarLook {
  /** data-mode: the status chip's look. */
  mode: string;
  /** The status words. */
  label: string;
  /** The chip's tooltip (none before the first state). */
  title: string | undefined;
  /** Take control / Release. */
  action: ActionLook;
  /** Resume after a pause ("Return to agent"). */
  resume: ButtonLook;
}

/** One of the bar's buttons. */
export interface ButtonLook {
  /** It is not offered. */
  hidden: boolean;
  /** It cannot be pressed. */
  disabled: boolean;
}

/** Take control / Release. */
export interface ActionLook extends ButtonLook {
  /** Its words. */
  text: string;
}

/** Which page actions are marked blocked. */
export interface Blocked {
  /** Back, Forward, Reload, a tab's close, and the address bar. */
  pageBlocked: boolean;
  /** New tab. */
  newTabBlocked: boolean;
  /** Start recording. */
  recordBlocked: boolean;
}

/** What the guard does with a click on a page action. */
export type Verdict = 'allow' | 'take' | 'refuse';

/** The parts of the bridge the bar uses. */
export type ControlBridge = Pick<
  OyaBrowser,
  'getControlState' | 'changeControl' | 'onControlState' | 'requestTakeover'
>;

/** The person holds control, here. */
const isMine = (c: ControlSnapshot): boolean => c.mode === 'human' && !!c.mine;

/** The label for a mode; "you" versus "another operator" when a person holds control. */
export function controlLabel(c: ControlSnapshot, mode: string): string {
  if (c.busyAction === 'return') return TEXT.returning;
  if (mode === 'human') return isMine(c) ? TEXT.mine : TEXT.other;
  return (Object.hasOwn(CONTROL_LABELS, mode) ? CONTROL_LABELS[mode] : '') || TEXT.checking;
}

/** Take control / Release: offered when handoffs are possible and nobody else holds control. */
function actionLook(c: ControlSnapshot, pending: Pending): ActionLook {
  const available = c.supported || (c.localClients ?? 0) > 0 || c.local;
  const hidden = !available || (c.mode === 'human' && !c.mine);
  const disabled = !!c.busy || (!!c.taking && !c.mine) || pending === 'action';
  return { hidden, disabled, text: isMine(c) ? TEXT.release : TEXT.take };
}

/** The bar before any control state has come (matches the first paint). */
const FIRST_LOOK: ControlBarLook = {
  mode: OFFLINE_MODE,
  label: CONTROL_LABELS[OFFLINE_MODE],
  title: undefined,
  action: { hidden: true, disabled: false, text: TEXT.take },
  resume: { hidden: true, disabled: false },
};

/** Everything the bar shows for `state`. */
export function controlLook({ control: c, error, pending }: ControlState): ControlBarLook {
  if (!c) return FIRST_LOOK;
  const mode = c.busy || c.taking ? 'taking' : (c.mode ?? '');
  const title = error || (c.interactive ? TEXT.interactive : TEXT.watchOnly);
  const resume = { hidden: c.mode !== 'paused' || !!c.taking || !!c.busy, disabled: !!c.busy || pending === 'resume' };
  return { mode, label: error || controlLabel(c, mode), title, action: actionLook(c, pending), resume };
}

/** One side of the AI Mode / Human Mode switch. */
export interface SwitchSide {
  /** This side is the one in effect. */
  on: boolean;
  /** It cannot be chosen now (in effect already, or a change is under way). */
  disabled: boolean;
  /** What choosing it does, as a tooltip. */
  title: string;
}

/** The AI Mode / Human Mode switch: its sides, and a status for the moments neither side says it all. */
export interface ModeSwitch {
  /** No handoff is possible here (another operator, or no agent to hand to): only the status shows. */
  hidden: boolean;
  /** AI Mode: the agent drives. Choosing it hands back, or resumes after a pause. */
  agent: SwitchSide;
  /** Human Mode: you drive. Choosing it takes control. */
  human: SwitchSide;
  /** Said beside the switch while it is changing, paused, failed or out of reach; '' when a side says it. */
  status: string;
}

/**
 * The shown page is the person's: they hold control, or the agent drives only its own
 * tab and this is another. Without the second, browsing beside the agent read as AI Mode.
 */
const shownIsYours = (c: ControlSnapshot | null): boolean =>
  !!c && (isMine(c) || (c.mode === 'agent' && !!c.pageInteractive));

/** The switch for this state, built on the control bar's look so the two never disagree. */
export function modeSwitch(state: ControlState): ModeSwitch {
  const look = controlLook(state);
  const mine = !!state.control && isMine(state.control);
  const yours = shownIsYours(state.control);
  // Still clickable on your own tab beside a working agent: it is the one way to take over from there.
  const human = { on: yours, disabled: mine || look.action.hidden || look.action.disabled, title: TEXT.take };
  const hidden = look.action.hidden && look.resume.hidden;
  return { hidden, agent: agentSide(look, mine, yours), human, status: statusOf(state, look, mine) };
}

/**
 * AI Mode: in effect while the agent drives the shown page; never while it is yours,
 * as during a hand-back (the agent's next command must find it returned). Chosen, it
 * hands back (yours) or resumes (paused).
 */
function agentSide(look: ControlBarLook, mine: boolean, yours: boolean): SwitchSide {
  const paused = look.mode === 'paused';
  const on = !yours && (look.mode === 'agent' || look.mode === 'taking');
  const back = paused ? look.resume : { ...look.action, hidden: look.action.hidden || !mine };
  return { on, disabled: on || back.hidden || back.disabled, title: paused ? RESUME_TITLE : TEXT.release };
}

/** The status beside the switch: none once a side says it, else the bar's own label. */
function statusOf(state: ControlState, look: ControlBarLook, mine: boolean): string {
  const settled = (look.mode === 'agent' || mine) && !state.error && !state.control?.busy;
  return settled ? '' : look.label;
}

/** Whether Take control is on offer right now. */
export function takeable(state: ControlState): boolean {
  const { action } = controlLook(state);
  return !action.hidden && !action.disabled;
}

/** Whether each page action is marked blocked: all of them while watch-only, except a record button that can take control. */
export function blockedActions(state: ControlState): Blocked {
  const interactive = !!state.control?.interactive;
  const recordBlocked = !interactive && !takeable(state);
  return { pageBlocked: !pageOpen(state), newTabBlocked: !newTabOpen(state), recordBlocked };
}

/** Whether a new tab may be opened: beside an agent's tab, even while the person looks at it. */
const newTabOpen = (state: ControlState): boolean =>
  !!(state.control?.newTabInteractive ?? state.control?.pageInteractive ?? state.control?.interactive);

/** Whether the tab this window shows is the person's: with control, or beside the agent's own tab. */
const pageOpen = (state: ControlState): boolean => !!(state.control?.pageInteractive ?? state.control?.interactive);

/** The control bar. */
export class ControlViewModel extends ViewModel<ControlState> {
  /** The main process. */
  private readonly bridge: ControlBridge;

  /** Nothing known until the first state, following the main process. */
  constructor(bridge: ControlBridge) {
    super({ control: null, error: '', pending: '' });
    this.bridge = bridge;
    this.own(bridge.onControlState((state) => this.received(state)));
    bridge.getControlState().then(
      (state) => this.set({ control: state }),
      () => {},
    );
  }

  /** Take control, or release it if it is already yours. */
  async toggle(): Promise<void> {
    const c = this.state.control;
    if (!takeable(this.state)) return;
    if (c?.mode === 'agent') return this.requestTakeover();
    await this.change(c && isMine(c) ? 'return' : 'acquire', 'action');
  }

  /** Resume: hands control back to the agent after a pause. */
  async resume(): Promise<void> {
    await this.change('return', 'resume');
  }

  /** Takes control (for Start recording); whether the page is interactive afterwards. */
  async acquire(): Promise<boolean> {
    await this.change('acquire', 'action');
    return !!this.state.control?.interactive;
  }

  /** Only the explicit header action opens the native confirmation; ownership updates arrive as events. */
  async requestTakeover(): Promise<void> {
    this.set({ pending: 'action', error: '' });
    await this.bridge.requestTakeover().catch(() => this.set({ error: TEXT.failed }));
    this.set({ pending: '' });
  }

  /** A click on a page action: allowed, refused, or (Start recording) allowed once control is taken. */
  guard(record: boolean, newTab = false): Verdict {
    const open = newTab ? newTabOpen(this.state) : pageOpen(this.state);
    if (this.state.control?.interactive || (!record && open)) return 'allow';
    return record && takeable(this.state) ? 'take' : 'refuse';
  }

  /** Asks for a control change and shows the answer. */
  private async change(action: 'acquire' | 'return', pending: Pending): Promise<void> {
    this.set({ error: '', pending });
    const answer: ControlChange | null = await this.bridge.changeControl(action).catch(() => null);
    const error = answer ? answer.error || '' : TEXT.failed;
    this.set({ error, pending: '', control: answer?.state ?? this.state.control });
  }

  /** A new state from the main process; a real change clears the last error. */
  private received(next: ControlSnapshot): void {
    const c = this.state.control;
    const moved = next.mode !== c?.mode || next.revision !== c?.revision || next.connected !== c?.connected;
    this.set(moved ? { control: next, error: '' } : { control: next });
  }
}
