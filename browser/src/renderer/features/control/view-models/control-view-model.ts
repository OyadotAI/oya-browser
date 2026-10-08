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
import { CONTROL_LABELS, OFFLINE_MODE, TEXT } from '../model/constants.ts';

/** Who drives, as control-state.cjs `snapshot()` reports it (the fields the bar reads). */
export interface ControlSnapshot {
  /** agent, human, paused, offline, disconnected, unavailable. */
  mode?: string;
  /** This desktop holds the control the mode names. */
  mine?: boolean;
  /** A person may use the page right now. */
  interactive?: boolean;
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
  /** Back, Forward, Reload, New tab, a tab's close, and the address bar. */
  pageBlocked: boolean;
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

/** Whether Take control is on offer right now. */
export function takeable(state: ControlState): boolean {
  const { action } = controlLook(state);
  return !action.hidden && !action.disabled;
}

/** Whether each page action is marked blocked: all of them while watch-only, except a record button that can take control. */
export function blockedActions(state: ControlState): Blocked {
  const interactive = !!state.control?.interactive;
  return { pageBlocked: !interactive, recordBlocked: !interactive && !takeable(state) };
}

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

  /** Blocked navigation asks to take control instead of silently refusing input. */
  requestTakeover(): void {
    void this.bridge.requestTakeover().catch(() => {});
  }

  /** A click on a page action: allowed, refused, or (Start recording) allowed once control is taken. */
  guard(record: boolean): Verdict {
    if (this.state.control?.interactive) return 'allow';
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
