/**
 * A person at the desktop taking or returning control, and the local commands
 * the desktop runs while it holds control.
 */
import { randomUUID } from 'crypto';
import { control } from '../../../control/service.ts';
import { desktopControl } from '../../../control/desktop.ts';
import { MAX_CONTROL_ID, MAX_LOCAL_COMMANDS } from '../constants.ts';
import type { Connection, Handler } from './types.ts';

/** Answers a desktop_control request; a socket that closed meanwhile gets nothing. */
function reply(conn: Connection, id: string, body: object) {
  if (conn.isOpen()) conn.send({ type: 'desktop_control_result', id, ...body });
}

/** The desktop starts a local command: it holds a control-plane command slot until it ends. */
async function startLocalCommand(conn: Connection, id: string) {
  if (conn.localCommands.size >= MAX_LOCAL_COMMANDS) throw new Error('Too many pending local commands');
  const finish = await control().beginCommand(conn.browserId);
  if (!conn.isOpen()) return finish();
  const token = randomUUID();
  conn.localCommands.set(token, finish);
  reply(conn, id, { token });
}

/** The desktop finished a local command: release its slot. */
async function endLocalCommand(conn: Connection, id: string, token: string) {
  const finish = conn.localCommands.get(token);
  conn.localCommands.delete(token);
  await finish?.();
  reply(conn, id, {});
}

/** Local command bookkeeping; a failure is answered, not thrown. */
async function localCommand(conn: Connection, msg) {
  try {
    if (msg.action === 'command-start') await startLocalCommand(conn, msg.id);
    else await endLocalCommand(conn, msg.id, msg.token);
  } catch (error) {
    reply(conn, msg.id, { error: error.message });
  }
}

/** Runs the handoff and answers with the new state, or the error and the current state. */
async function changeControl(conn: Connection, msg) {
  const { apiKey, browserId } = conn;
  const stillHere = () => conn.isCurrent() && conn.isOpen();
  try {
    reply(conn, msg.id, { state: await desktopControl(apiKey, browserId, msg.action, stillHere) });
  } catch (error) {
    const state = await desktopControl(apiKey, browserId, 'get').catch(() => null);
    reply(conn, msg.id, { error: error.message, state });
  }
}

/** One handoff at a time. */
async function handOff(conn: Connection, msg) {
  if (conn.changingControl) return reply(conn, msg.id, { error: 'A control handoff is already in progress' });
  conn.changingControl = true;
  await changeControl(conn, msg).finally(() => {
    conn.changingControl = false;
  });
}

/** Whether a desktop_control message is well-formed and from the browser's current socket. */
const isValid = (conn: Connection, msg) =>
  conn.isCurrent() && typeof msg.id === 'string' && msg.id.length <= MAX_CONTROL_ID;

/** Desktop control: local command bookkeeping, or a control handoff. */
export const desktopControlMessage: Handler = async (conn, msg) => {
  if (!isValid(conn, msg)) return;
  const local = msg.action === 'command-start' || msg.action === 'command-end';
  await (local ? localCommand(conn, msg) : handOff(conn, msg));
};
