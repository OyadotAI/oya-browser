/**
 * POST /control/sessions/:id/cancel: stops the session whatever its state and
 * answers its final state. Cancel used to write `cleanup_pending` and leave a
 * ready browser to the worker's next tick, so the caller was told cancelled
 * while `oya ls` still showed the browser running. The stop goes through the
 * same path as /stop, so what the caller reads is what the fleet shows.
 */
import { control, fault, terminal } from '../service.ts';
import { Status } from '../../../platform/http-status.ts';
import { key } from './guards.ts';

/** Stops the session and answers the state it ended in; a terminal session is answered as it is. */
export async function cancelSession(req) {
  const before = await control().session(key(req), req.params.id);
  if (terminal.has(before.state)) return before;
  await stopOrRefuse(req, req.params.id);
  return control().session(key(req), req.params.id);
}

/** Runs the one stop path. A stop that could not go through, such as a profile that would be lost, is refused the way /stop refuses it. */
async function stopOrRefuse(req, id) {
  // Dynamic, as /stop does it: control depends on browsers, and browsers' admission reads control.
  const { stopBrowser } = await import('../../../app/api.ts');
  const result = await stopBrowser(req, id);
  if (result.ok) return;
  throw fault('cancel_failed', result.error, typeof result.status === 'number' ? result.status : Status.CONFLICT);
}
