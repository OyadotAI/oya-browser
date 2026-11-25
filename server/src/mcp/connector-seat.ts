/**
 * The one browser a chat connector (claude.ai, ChatGPT signed in over OAuth)
 * drives, as the local MCP's agent has its own tab: nothing to start or pick. Its
 * first page tool starts a browser, every later call goes to that browser, and
 * once it ends the next call starts a new one. Keyed by the connector's
 * credential (its seat), never the project key, so a connector does not share a
 * target with an SDK client or another connector in the same project. That is
 * routing, not a wall: anyone who can operate the project can drive any of its
 * browsers, this one included.
 *
 * Which browser a seat drives is remembered in the control store (seat-store.ts),
 * so a deploy or a dropped socket rejoins the same browser. Several server
 * replicas are not covered: a call landing on a replica the browser is not
 * connected to waits for it, then starts another. Production runs one server
 * replica; route /mcp/pool by seat before running more.
 */
import { ownsCloudBrowser } from '../modules/browsers/pool.ts';
import { connects } from './dial-in.ts';
import { recallSeat, rememberSeat, sessionLive } from './seat-store.ts';
import { selfApi, type Self } from './self-api.ts';
import { attempt } from './replies.ts';
import type { Acquired } from './target.ts';
import { CONNECTOR_START_REPLY_MS, REJOIN_WAIT_MS, START_RETRY_AFTER_MS } from './constants.ts';

/** A connector: its credential, the project key its browsers belong to, and how it reaches the public API. */
export interface Seat {
  /** Its credential id. */
  id: string;
  /** The project key that owns its browsers. */
  apiKey: string;
  /** Its origin and credential, for starting and stopping through the public API. */
  self: Self;
}

/** seat → the browser it drives. */
const held = new Map<string, string>();
/** seat → the start in flight, which concurrent first calls share. */
const starting = new Map<string, Promise<Acquired>>();
/** seat → the search for its remembered browser in flight, shared the same way. */
const rejoining = new Map<string, Promise<string | undefined>>();
/** seat → the idempotency key of a start whose reply was lost, so the repeat is the same start, not a second. */
const unsettled = new Map<string, string>();
/** seat → a refused start and until when it is answered from memory. */
const refused = new Map<string, Refusal>();
/** Seats whose browser ended (stopped, timed out, disconnected) since they last used it. */
const lapsed = new Set<string>();

/** What the first reply after a start says. */
const STARTED = 'Started a browser for this chat.';
/** What the first reply after a replacement start says. */
const RESTARTED = 'The previous browser ended, so a new one started. Pages from before are gone; open them again.';
/** The answer while a boot outlasts the call. */
const STILL_STARTING: Acquired = { error: 'The browser is still starting. Repeat this step shortly.' };

/** A start the API refused, remembered briefly. */
interface Refusal {
  /** What the model was told. */
  answer: Acquired;
  /** When the start may be tried again. */
  until: number;
}

/** A start's failure, as the model is told it: the reason, ending in one full stop whatever the API wrote. */
const couldNot = (reason: string): Acquired => ({
  error: `Could not start a browser: ${reason.replace(/\.+$/, '')}. Tell the person this reason.`,
});

/** What a tool that may not start a browser answers when none is running. */
const NOT_OPEN: Acquired = { error: 'No browser is open. Call open_tab with a URL to start one.' };

/**
 * The seat's browser: the one it holds, the one it is remembered to drive (after a deploy or a
 * dropped socket), or, when `start` allows, one started now. A start already under way is
 * joined either way.
 */
export async function acquire(seat: Seat, start = true): Promise<Acquired> {
  const id = current(seat) || (await within(rejoin(seat), CONNECTOR_START_REPLY_MS, undefined));
  if (id) return { id };
  return start || starting.has(seat.id) ? startOnce(seat) : NOT_OPEN;
}

/** The browser the seat holds while it runs; one that ended is let go. */
function current(seat: Seat) {
  const id = held.get(seat.id);
  if (id && ownsCloudBrowser(seat.apiKey, id)) return id;
  if (id) held.delete(seat.id);
  return undefined;
}

/** One search per seat for its remembered browser, however many calls arrive. */
function rejoin(seat: Seat): Promise<string | undefined> {
  let run = rejoining.get(seat.id);
  if (!run) {
    run = remembered(seat).finally(() => rejoining.delete(seat.id));
    rejoining.set(seat.id, run);
  }
  return run;
}

/**
 * The browser the store says this seat drives, once it is connected here: at once when it is,
 * after a short wait when its session is still live (its socket is redialling). One that is
 * gone marks the seat lapsed, so the start that follows says the old pages are gone.
 */
async function remembered(seat: Seat): Promise<string | undefined> {
  const found = await attempt(() => recallSeat(seat.id));
  const id = 'value' in found ? found.value : undefined;
  if (!id) return undefined;
  if (await back(seat, id)) return adopt(seat.id, id);
  lapsed.add(seat.id);
  return undefined;
}

/** Whether the remembered browser is connected here as this project's, waiting briefly while its session lives. */
async function back(seat: Seat, id: string) {
  if (ownsCloudBrowser(seat.apiKey, id)) return true;
  const live = await attempt(() => sessionLive(seat.apiKey, id));
  if (!('value' in live) || !live.value) return false;
  return (await connects(id, REJOIN_WAIT_MS)) && ownsCloudBrowser(seat.apiKey, id);
}

/** The seat holds its browser again: nothing ended. */
function adopt(seat: string, id: string) {
  held.set(seat, id);
  lapsed.delete(seat);
  return id;
}

/** One start per seat however many calls arrive; each waits a bounded time for it. */
function startOnce(seat: Seat): Promise<Acquired> {
  const recent = refused.get(seat.id);
  if (recent && recent.until > Date.now()) return Promise.resolve(recent.answer);
  let run = starting.get(seat.id);
  if (!run) {
    run = boot(seat).finally(() => starting.delete(seat.id));
    starting.set(seat.id, run);
  }
  return within(run, CONNECTOR_START_REPLY_MS, STILL_STARTING);
}

/** `run`'s answer, or `otherwise` once `ms` pass; the work carries on either way. */
function within<T>(run: Promise<T>, ms: number, otherwise: T): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<T>((resolve) => (timer = setTimeout(() => resolve(otherwise), ms)));
  return Promise.race([run, late]).finally(() => clearTimeout(timer));
}

/** Starts a browser through the public API as the connector, and holds it once it has dialled in. */
async function boot(seat: Seat): Promise<Acquired> {
  const started = await requestStart(seat);
  if ('error' in started) return refuse(seat, started.error);
  const id = String(started.value.id);
  if (!(await connects(id)) || !ownsCloudBrowser(seat.apiKey, id)) return abandon(seat, id);
  held.set(seat.id, id);
  // Remembered so a deploy or a dropped socket rejoins this browser; unremembered, it is only held here.
  await attempt(() => rememberSeat(seat.id, id));
  return { id, note: lapsed.delete(seat.id) ? RESTARTED : STARTED };
}

/**
 * Asks the public API for a browser. A start whose reply was lost keeps its idempotency key, so
 * asking again returns that browser instead of starting, and billing, another.
 */
async function requestStart(seat: Seat) {
  const key = unsettled.get(seat.id) || globalThis.crypto.randomUUID();
  unsettled.set(seat.id, key);
  const started = await attempt(() => selfApi(seat.self, '/browsers/start', { provider: 'oya-cloud' }, key));
  if ('value' in started || started.error?.answered) unsettled.delete(seat.id);
  return started;
}

/** A start that failed: logged, told to the model, and not tried again for a moment. */
function refuse(seat: Seat, error: unknown): Acquired {
  const reason = (error as Error)?.message || String(error);
  console.error(`[mcp] connector ${seat.id}: start failed: ${reason}`);
  const answer = couldNot(reason);
  refused.set(seat.id, { answer, until: Date.now() + START_RETRY_AFTER_MS });
  return answer;
}

/** A browser that never came up is stopped, so it holds no session and no seat. */
async function abandon(seat: Seat, id: string): Promise<Acquired> {
  const stopped = await attempt(() => selfApi(seat.self, '/browsers/stop', { ids: [id] }));
  const outcome = 'error' in stopped ? `and could not be stopped: ${stopped.error?.message}` : 'and was stopped';
  console.error(`[mcp] connector ${seat.id}: browser ${id} did not come up ${outcome}`);
  return couldNot('it did not come up in time');
}

/**
 * A browser whose socket closed is no longer held. Whether it ended or is only redialling is
 * settled on the seat's next call, from its session in the store.
 */
export function releaseBrowser(browserId: string) {
  for (const [seat, id] of held) if (id === browserId) held.delete(seat);
}

/** Every seat forgotten: for tests. */
export function resetSeats() {
  held.clear();
  starting.clear();
  rejoining.clear();
  unsettled.clear();
  refused.clear();
  lapsed.clear();
}
