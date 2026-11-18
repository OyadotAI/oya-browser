/**
 * Chat runs that outlive the request that started them. A desktop's chat is one
 * long HTTP request, and a network that drops it used to stop the run. A run
 * started here keeps going while nobody listens, for a grace window, so the
 * desktop can ask again for the same run and collect its answer. Kept in this
 * process, one run per browser lane: a restart or another replica has no run to
 * resume, and the caller is told so.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { CHAT_GRACE_MS } from './constants.ts';

/** A run a caller is listening to. */
export interface HeldRun {
  /** The run's answer, or its error. */
  answer: Promise<unknown>;
}

/** A run and who may collect it. */
interface Entry extends HeldRun {
  /** The API key that started it. */
  owner: string;
  /** The token its caller named, so a resume never collects another run's answer. */
  token: string;
  /** Stops the run. */
  abort: AbortController;
  /** Requests waiting on the answer. */
  listeners: number;
  /** Whether the run has ended. */
  done: boolean;
  /** The countdown to stopping an unheard run, or to forgetting a finished one. */
  timer?: ReturnType<typeof setTimeout>;
}

/** The key a browser lane's run is kept under. */
export const runKey = (browserId: string, lane: string) => `${browserId}#${lane}`;

/** What a resume answers when there is no such run: it ended long ago, or it ran on a server that has since restarted. */
const gone = () => new HttpError(Status.NOT_FOUND, 'No chat run to resume', { code: 'chat_run_gone' });

/** The runs that can be resumed, one per key. */
export class ResumableRuns {
  /** How long a run goes unheard before it is stopped, and how long a finished answer is kept. */
  declare private readonly graceMs: number;
  /** The run on each key. */
  declare private readonly runs: Map<string, Entry>;

  /** `graceMs` is the window a dropped caller has to come back in. */
  constructor(graceMs = CHAT_GRACE_MS) {
    this.graceMs = graceMs;
    this.runs = new Map();
  }

  /**
   * Starts `work` as the run on `key`, with the caller listening. A run already there is stopped:
   * two runs in one lane would fight over the same tab. `work` is called before this returns, so
   * it keeps the caller's async context (its lane).
   */
  start(key: string, owner: string, token: string, work: (signal: AbortSignal) => Promise<unknown>): HeldRun {
    this.replace(key, owner);
    const abort = new AbortController();
    const answer = new Promise((resolve) => resolve(work(abort.signal)));
    const entry: Entry = { owner, token, abort, listeners: 1, done: false, answer };
    this.runs.set(key, entry);
    const settle = () => this.settled(key, entry);
    answer.then(settle, settle);
    return entry;
  }

  /** Stops the run already on `key` to make way for its owner's next; another key's unfinished run is not its to stop. */
  private replace(key: string, owner: string): void {
    const entry = this.runs.get(key);
    if (entry && !entry.done && entry.owner !== owner)
      throw new HttpError(Status.CONFLICT, 'Another chat is running here');
    entry?.abort.abort();
  }

  /** Listens to the run on `key` again, which stops its countdown; refused unless the key and token are its own. */
  attach(key: string, owner: string, token: string): HeldRun {
    const entry = this.runs.get(key);
    if (!entry || entry.owner !== owner || entry.token !== token) throw gone();
    entry.listeners++;
    if (!entry.done) clearTimeout(entry.timer);
    return entry;
  }

  /** A listener went away. The last one leaving an unfinished run starts the countdown to stopping it. */
  release(run: HeldRun): void {
    const entry = run as Entry;
    entry.listeners--;
    if (entry.listeners > 0 || entry.done) return;
    entry.timer = setTimeout(() => entry.abort.abort(), this.graceMs);
  }

  /**
   * Stops `owner`'s unfinished run on `key` named `token` now; answers whether there was one. The
   * token matters: a stop that arrives late must not end the run its caller started next.
   */
  stop(key: string, owner: string, token: string): boolean {
    const entry = this.runs.get(key);
    if (!entry || entry.owner !== owner || entry.token !== token || entry.done) return false;
    entry.abort.abort();
    return true;
  }

  /** The run ended: its answer stays collectable for the grace window, then the entry is forgotten. */
  private settled(key: string, entry: Entry): void {
    entry.done = true;
    clearTimeout(entry.timer);
    const forget = () => this.runs.get(key) === entry && this.runs.delete(key);
    entry.timer = setTimeout(forget, this.graceMs);
    entry.timer.unref?.();
  }
}

/** This server's resumable chat runs. */
export const resumableRuns = new ResumableRuns();
