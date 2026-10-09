/** Server-requested profile capture stays on the authenticated connection and never drives a web page. */
import type { ServerMessageDeps } from './server-messages.ts';
/** Whether ordered native updates reached the control transport. */
interface CaptureResult {
  /** False leaves the server-side stop unsuccessful. */
  ok: boolean;
  /** Sanitized and actionable, never copied from a credential-bearing native failure. */
  error?: string;
}
/** Flush native state before answering; the server persists the preceding ordered updates before stopping. */
export async function captureProfile(deps: ServerMessageDeps, id: string | undefined): Promise<void> {
  if (!id || !deps.socket.ready) return;
  const socket = deps.socket.ws;
  const persona = deps.persona.active?.id;
  const outcome = await capture(deps);
  if (deps.socket.ws !== socket || deps.persona.active?.id !== persona || !deps.socket.ready) return;
  deps.socket.send({ type: 'cmd_result', id, ...outcome });
}
/** Report only a sanitized error; credential-bearing native exceptions never enter the command response. */
async function capture(deps: ServerMessageDeps): Promise<CaptureResult> {
  try {
    await deps.cookies.captureProfile();
    if (!(await deps.persona.flushStorage())) throw Error('Storage transport refused');
    return { ok: true };
  } catch {
    return { ok: false, error: 'Native profile capture failed; the browser was not stopped.' };
  }
}
