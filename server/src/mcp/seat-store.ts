/**
 * Which browser each chat connector drives, kept in the control store so it
 * outlives this process: after a deploy, or while a browser's socket is briefly
 * down, the connector finds its browser again instead of starting a second one
 * and leaving the first running.
 */
import { control, terminal } from '../modules/control/service.ts';

/** The row kind: one row per connector credential, rewritten on each start. */
const KIND = 'connector_seat';

/** Remembers that `seat` drives `browserId`. */
export async function rememberSeat(seat: string, browserId: string) {
  await control().store.transact(async (tx) => {
    // Read first: the store refuses to overwrite a row the transaction has not seen.
    await tx.get(KIND, seat);
    tx.put(KIND, seat, { browserId, updatedAt: Date.now() });
  });
}

/** The browser `seat` last drove, or undefined. */
export async function recallSeat(seat: string): Promise<string | undefined> {
  return (await control().store.get(KIND, seat))?.browserId;
}

/** Whether this project's session `id` may still come back: it exists and has not stopped or failed. */
export async function sessionLive(apiKey: string, id: string) {
  const session = await control().findSession(apiKey, id);
  return Boolean(session) && !terminal.has(session.state);
}
