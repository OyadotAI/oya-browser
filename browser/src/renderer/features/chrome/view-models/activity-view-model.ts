/**
 * Whether an agent is working right now. Holding control is not the same as
 * working: a connected browser rests under agent control for hours. So the
 * chrome's signs of life (the window's orb turning, the thread of light along
 * the toolbar, the control capsule's ring; html[data-agent-active]) follow
 * the commands actually arriving, and settle a moment after the last one.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import { AGENT_COMMAND, INCOMING } from '../model/constants.ts';

/** The agent's activity. */
export interface ActivityState {
  /** A command arrived within the last AGENT_ACTIVE_MS. */
  active: boolean;
}

/** Whether an activity log entry is a command arriving from the server. */
export const isCommand = (entry: Payload | null | undefined): boolean =>
  entry?.dir === INCOMING && AGENT_COMMAND.test(String(entry.type ?? ''));

/** The agent's activity, as the chrome shows it. */
export class AgentActivityViewModel extends ViewModel<ActivityState> {
  /** The timer that settles the chrome after the last command. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** At rest, following the activity log. */
  constructor(bridge: Pick<OyaBrowser, 'onDevLog'>) {
    super({ active: false });
    this.own(bridge.onDevLog((entry) => this.note(entry)));
    this.own(() => clearTimeout(this.timer));
  }

  /** One activity entry: a command arriving means the agent is at work, until a moment after the last. */
  private note(entry: Payload): void {
    if (!isCommand(entry)) return;
    this.set({ active: true });
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.set({ active: false }), C.AGENT_ACTIVE_MS);
  }
}
