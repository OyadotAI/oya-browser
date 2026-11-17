/**
 * Lanes: which agent a browser command comes from (a chat run, an MCP client, a
 * playbook run). An Oya desktop gives each lane a tab of its own, so several agents
 * work side by side while the person uses the other tabs. The lane rides along with
 * the work that sends the commands, so no tool needs to pass it on.
 */
import { AsyncLocalStorage } from 'node:async_hooks';
import { randomUUID } from 'node:crypto';
import { LANE_PATTERN } from './constants.ts';
import { registry } from '../registry.ts';

/** The lane of the work in progress. */
const lanes = new AsyncLocalStorage<string>();

/** Runs `work` as lane `lane`'s; a lane that is not a short name runs in no lane. */
export function inLane<T>(lane: unknown, work: () => T): T {
  return typeof lane === 'string' && LANE_PATTERN.test(lane) ? lanes.run(lane, work) : work();
}

/**
 * Runs `work` in a lane of its own that this server names, and tells browser `browserId`
 * when it has finished, however it finishes. A desktop only learns a server-named lane's
 * end this way; without it the lane holds its tab until it lapses, and the next run on
 * that browser gets a hidden background tab. The end goes out before `work`'s promise
 * settles, so it reaches the socket ahead of anything sent after the run reports done.
 */
export async function inOwnLane<T>(browserId: string, work: () => T): Promise<Awaited<T>> {
  const lane = randomUUID();
  try {
    return await inLane(lane, work);
  } finally {
    endLane(browserId, lane);
  }
}

/**
 * Runs `work` in the lane already running (one the caller named), or in a lane of its own when
 * there is none. A caller's lane is the caller's to end; without this, each call of an SDK
 * browser that named one lane would still get a lane, and on a desktop a tab, of its own.
 */
export function inCallerOrOwnLane<T>(browserId: string, work: () => T): Promise<Awaited<T>> {
  return currentLane() ? Promise.resolve(work()) : inOwnLane(browserId, work);
}

/** Tells browser `browserId` that lane `lane` has finished, so a desktop gives back its tab. */
export function endLane(browserId: string, lane: string): void {
  registry.send(browserId, { type: 'lane_end', lane });
}

/**
 * Express middleware: the rest of the request runs in the lane its caller named (X-Oya-Agent or
 * ?agent=). An SDK browser names one lane for its whole life, so its goto, ask and play all land
 * in one tab of the person's desktop instead of the shown tab plus a new tab per call.
 */
export function laneMiddleware(req, _res, next) {
  return inLane(laneFrom(req), next);
}

/** The lane of the work in progress, if any. */
export function currentLane(): string | undefined {
  return lanes.getStore();
}

/** The parts of a request a lane is read from. */
interface Named {
  /** Its headers, lower-cased. */
  headers?: Record<string, unknown>;
  /** Its query string. */
  query?: Record<string, unknown>;
}

/** The lane an external caller names: the X-Oya-Agent header, or ?agent= on its URL. */
export function laneFrom(req: Named): unknown {
  return req.headers?.['x-oya-agent'] ?? req.query?.agent;
}

/**
 * State kept per browser, one entry per lane: two agents on one browser (two desktop
 * tabs' chats) never read each other's page or element ids. Outside any lane, a read
 * gets the browser's latest entry, which is what a single agent always had.
 */
export class LaneMap<V> {
  /** Entries by browser id and lane. */
  private readonly entries = new Map<string, V>();
  /** Each browser's latest entry key. */
  private readonly latest = new Map<string, string>();
  /** Most entries kept; the oldest goes first. */
  private readonly cap: number;

  /** Keeps at most `cap` entries. */
  constructor(cap = Infinity) {
    this.cap = cap;
  }

  /** The running lane's entry for `browserId`, or its latest outside a lane. */
  get(browserId: string): V | undefined {
    const lane = currentLane();
    return this.entries.get(lane ? `${browserId}#${lane}` : (this.latest.get(browserId) ?? browserId));
  }

  /** Forgets the running lane's entry for `browserId`; outside a lane, every lane's (the browser went). */
  delete(browserId: string): void {
    const lane = currentLane();
    if (lane) return void this.entries.delete(`${browserId}#${lane}`);
    for (const key of [...this.entries.keys()])
      if (key === browserId || key.startsWith(`${browserId}#`)) this.entries.delete(key);
    this.latest.delete(browserId);
  }

  /** Sets the running lane's entry for `browserId`, newest last. */
  set(browserId: string, value: V): void {
    const lane = currentLane();
    const key = lane ? `${browserId}#${lane}` : browserId;
    this.entries.delete(key);
    this.entries.set(key, value);
    this.latest.set(browserId, key);
    if (this.entries.size > this.cap) this.entries.delete(this.entries.keys().next().value!);
  }
}
