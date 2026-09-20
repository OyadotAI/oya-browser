/**
 * Who drives the browser: the agent, a person, or nobody while the agent is
 * paused. The panel's control button cycles through them.
 */
import { api, errorMessage } from '@/lib/api-client';
import type { PanelContext } from './context';

/** What the control button asks for in each mode; any other mode resumes the agent. */
const CONTROL_ACTION: Record<string, string> = { agent: 'acquire', human: 'release' };
/** The control button's label in each mode. */
const BUTTON_LABEL: Record<string, string> = { agent: 'Take control', human: 'Release control' };
/** The status line in each mode; anything else reads as agent control. */
const MODE_TEXT: Record<string, string> = {
  human: 'Human control · agent paused',
  paused: 'Paused · awaiting agent resume',
};

/** Looks `mode` up in `table`, falling back when it has no entry. */
const pick = (table: Record<string, string>, mode: string, fallback: string) =>
  Object.hasOwn(table, mode) ? table[mode] : fallback;

/** The control request the button sends in this mode. */
export const controlAction = (mode: string) => pick(CONTROL_ACTION, mode, 'resume');
/** The button's label in this mode. */
export const buttonLabel = (mode: string) => pick(BUTTON_LABEL, mode, 'Resume agent');
/** The status line in this mode. */
export const modeText = (mode: string) => pick(MODE_TEXT, mode, 'Agent control · take control to drive');

/** What POST /control/sessions/:id/control answers. */
interface ControlResult {
  /** The mode after the change. */
  mode: string;
}

/** POSTs the control request for the current mode. */
const requestControl = (ctx: PanelContext, mode: string) =>
  api<ControlResult>(`/control/sessions/${ctx.browserId}/control`, {
    key: ctx.apiKey,
    method: 'POST',
    body: { action: controlAction(mode) },
  });

/** Asks the server to change who drives, and stores the mode it answers with. */
export async function toggleControl(ctx: PanelContext, mode: string, setMode: (m: string) => void) {
  try {
    setMode((await requestControl(ctx, mode)).mode);
  } catch (e) {
    ctx.toast(errorMessage(e), 'error');
  }
}
