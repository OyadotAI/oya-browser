/**
 * Routines: saved prompts the agent runs on a schedule, "every N minutes or
 * hours" or "daily at HH:MM". They belong to the project and live on the
 * server with their history, so every desktop on the project shows the same
 * list; this app runs them while it is open and connected. Before a run, the
 * app claims it from the server, which gives each due run to exactly one
 * desktop. A run is an ordinary Ask (control is lent to the agent), one at a
 * time, never over a chat or a recording; a run missed while every desktop was
 * closed fires once, on the next tick. Routines this app kept locally before
 * they moved to the server are handed to the project on the first connect.
 *
 * ponytail: a run's ending that cannot reach the server (offline mid-run) is
 * not retried; the server's lease marks it interrupted. Queue it if that matters.
 */
import { randomUUID } from 'node:crypto';
import type { AppServices } from '../app/services.ts';
import { Chat, STOPPED } from '../connection/chat.ts';
import { ServerApi } from '../connection/server-api.ts';
import { ROUTINE_TICK_MS, ROUTINE_UNIT_MS, ROUTINE_RESULT_CHARS, ROUTINE_STEPS_KEPT } from './constants.ts';

/** The services routines use. */
type Deps = Pick<AppServices, 'socket' | 'shell' | 'config' | 'recorder' | 'chatAbort' | 'control' | 'shield'>;

/** When a routine runs: "every N units" (`n`, `unit`) or "daily at HH:MM" (`at`), as the server holds it. */
export interface Schedule {
  /** 'every' or 'daily'. */
  kind: string;
  /** How many units apart an 'every' routine runs. */
  n: number;
  /** 'minutes' or 'hours'. */
  unit: string;
  /** A daily routine's HH:MM. */
  at: string;
}

/** One run of a routine, as the server records it. */
export interface RoutineRun {
  /** The run's id. */
  id: string;
  /** 'running', 'done', 'failed', 'stopped' or 'interrupted'. */
  status: string;
  /** The browser running it. */
  by?: string;
}

/** A routine, as the server holds it. */
export interface Routine {
  /** The routine's id. */
  id: string;
  /** Its name in the pane. */
  name?: string;
  /** What the agent is asked. */
  prompt: string;
  /** Whether its schedule is on. */
  enabled: boolean;
  /** When it was made, in epoch milliseconds. */
  createdAt: number;
  /** When it last ran, or null. */
  lastRunAt?: number | null;
  /** When it runs. */
  schedule?: Schedule;
  /** Its recent runs. */
  runs?: RoutineRun[];
  /** Where it runs: on a desktop (the default), or in the cloud, where the server runs it and no desktop does. */
  target?: 'desktop' | 'cloud';
  /** The time zone its daily time is in, for the server running it in the cloud. */
  tz?: string;
}

/** What the agent answered a run. */
export interface ChatAnswer {
  /** Its reply. */
  text?: string;
  /** Why it could not answer, or STOPPED. */
  error?: string;
  /** Whether the run failed though it answered. */
  failed?: boolean;
  /** The tools it called, in order. */
  toolCalls?: ToolCall[];
}

/** One tool the agent called. */
interface ToolCall {
  /** The tool's name. */
  name: string;
}

/** One message of an Ask. */
interface ChatMessage {
  /** 'user' for a routine's prompt. */
  role: string;
  /** What is said. */
  content: string;
}

/** A routine with when it next runs, as the pane shows it. */
export interface ScheduledRoutine extends Routine {
  /** When it is next due, or null when off or unknown. */
  nextRunAt: number | null;
}

/** A finished run's ending, as the server records it. */
interface RunEnding {
  /** 'done', 'failed' or 'stopped'. */
  status: string;
  /** What the agent answered, cut to size. */
  result: string;
  /** The tools it called, cut to size. */
  steps: string[];
}

/** A change refused, with why. */
export interface Refusal {
  /** Why, for the pane. */
  error: string;
  /** Lets the pane read it as a payload (src/shared/ipc.ts). */
  [key: string]: unknown;
}

/** Asks the agent (Chat.send, faked in tests). */
export type RunChat = (messages: ChatMessage[]) => Promise<ChatAnswer>;

/** The run this app is running now. */
interface CurrentRun {
  /** The routine. */
  routineId: string;
  /** The claimed run. */
  runId: string;
}

/** What the Routines pane shows. */
export interface RoutinesSnapshot {
  /** The routines, each with its next run. */
  routines: ScheduledRoutine[];
  /** The routine this app is running, or null. */
  running: string | null;
  /** This browser's id, to tell its runs from other desktops' (null before the first sign-in). */
  browserId: string | null;
  /** Whether the server can be called. */
  online: boolean;
  /** Why the list could not be read or changed last, or ''. */
  error: string;
  /** Why a routine cannot run now, or ''. */
  busy: string;
  /** Lets the pane read it as a payload (src/shared/ipc.ts). */
  [key: string]: unknown;
}

/** The first moment after `since` that today's (or tomorrow's) HH:MM comes round. */
function nextDaily(at: string, since: number): number {
  const [hours, minutes] = at.split(':').map(Number);
  const next = new Date(since);
  next.setHours(hours, minutes, 0, 0);
  if (next.getTime() <= since) next.setDate(next.getDate() + 1);
  return next.getTime();
}

/** Schedule kind → when a routine last run at `since` is next due. */
const NEXT_RUN: Record<string, (schedule: Schedule, since: number) => number> = {
  every: ({ n, unit }, since) => since + n * ROUTINE_UNIT_MS[unit as keyof typeof ROUTINE_UNIT_MS],
  daily: ({ at }, since) => nextDaily(at, since),
};

/** When `routine` is next due, counted from its last run (or its creation); null for a schedule this app does not know. */
export function nextRunAt(routine: Routine): number | null {
  const { schedule } = routine;
  if (!schedule || !Object.hasOwn(NEXT_RUN, schedule.kind)) return null;
  return NEXT_RUN[schedule.kind](schedule, routine.lastRunAt ?? routine.createdAt);
}

/** The routine's run in progress, on any desktop, or undefined. */
const runningRun = (routine: Routine): RoutineRun | undefined =>
  (routine.runs || []).find((run) => run.status === 'running');

/** Whether this app should run `routine` at `now`: a desktop routine, on, due, and not running anywhere. */
export const isDue = (routine: Routine, now: number): boolean => {
  const next = nextRunAt(routine);
  const here = routine.target !== 'cloud';
  return here && routine.enabled && next !== null && next <= now && !runningRun(routine);
};

/** How a run ended, from the agent's answer. */
function runStatusOf(answer: ChatAnswer | undefined): string {
  if (answer?.error === STOPPED) return 'stopped';
  return answer?.error || answer?.failed ? 'failed' : 'done';
}

/** A finished run's ending, as the server records it: how, what it answered, and the steps it took. */
function endingOf(answer: ChatAnswer | undefined): RunEnding {
  const text = answer?.error && answer.error !== STOPPED ? `Error: ${answer.error}` : answer?.text || '';
  const steps = (answer?.toolCalls || []).map((call) => call.name).slice(0, ROUTINE_STEPS_KEPT);
  return { status: runStatusOf(answer), result: text.slice(0, ROUTINE_RESULT_CHARS), steps };
}

/** What Run now and the pane say when this app cannot run a routine now, or '' when it can. */
function busyReason(api: ServerApi, ctx: Deps, running: unknown): string {
  if (!api.canCall()) return 'Connect to Oya to run routines.';
  if (running) return 'Another routine is running. Stop it first.';
  if (ctx.recorder.recording) return 'Finish recording first.';
  return ctx.chatAbort ? 'The agent is busy with an Ask. Stop it first.' : '';
}

/** GET /api/routines. */
interface RoutineList {
  /** The project's routines. */
  routines?: Routine[];
}

/** The project's routines, read from the server. */
const readRoutines = async (api: ServerApi): Promise<Routine[]> =>
  ((await api.get('routines')) as RoutineList).routines || [];

/** The API route of routine `id`, with `rest` after it. */
const routineRoute = (id: string, rest = ''): string => `routines/${encodeURIComponent(id)}${rest}`;

/** The project's routines, as this app last read them, and the timer that runs the due ones. */
export class Routines {
  /** The project's routines, as last read from the server. */
  list: Routine[] = [];
  /** The routine and run this app is running now, or null. */
  current: CurrentRun | null = null;
  /** Why the list could not be read or changed last, or ''. */
  error = '';
  /** The main-process services. */
  private readonly ctx: Deps;
  /** Asks the agent. */
  private readonly run: RunChat;
  /** The server's API, as this browser. */
  private readonly api: ServerApi;

  /** `ctx` is the main-process services (see src/main/main.ts); `run` asks the agent (an Ask through Chat, faked in tests). */
  constructor(ctx: Deps, run?: RunChat) {
    this.ctx = ctx;
    this.api = new ServerApi(ctx);
    this.run = run ?? ((messages) => new Chat(ctx).send(messages));
  }

  /** The list with each routine's next run, and what this app is doing, as the Routines pane shows it. */
  snapshot(): RoutinesSnapshot {
    const routines = this.list.map((r) => ({ ...r, nextRunAt: r.enabled ? nextRunAt(r) : null }));
    const running = this.current?.routineId || null;
    const state = { online: this.api.canCall(), error: this.error, busy: busyReason(this.api, this.ctx, running) };
    return { routines, running, browserId: this.ctx.socket.browserId, ...state };
  }

  /** Tells the pane what changed. */
  publish(): RoutinesSnapshot {
    this.ctx.shell.send('routines-changed', this.snapshot());
    return this.snapshot();
  }

  /** Checks for a due routine every tick, for as long as the app runs. */
  start(): void {
    setInterval(() => void this.tick(), ROUTINE_TICK_MS).unref?.();
  }

  /** Re-reads the project's routines (handing over local ones first); answers the snapshot. */
  async refresh(): Promise<RoutinesSnapshot> {
    if (!this.api.canCall()) return this.publish();
    this.error = await this.load().then(
      () => '',
      (e: Error) => e.message,
    );
    return this.publish();
  }

  /** Hands over local routines, reads the project's, and closes out runs a quit left behind. */
  private async load(): Promise<void> {
    await this.handOverLocal();
    this.list = await readRoutines(this.api);
    await this.markInterrupted();
  }

  /** Routines kept in config.json before they moved to the server: given to this project once, then dropped here. */
  private async handOverLocal(): Promise<void> {
    const local = this.ctx.config.values.routines as Routine[] | undefined;
    if (!local?.length) return;
    await this.api.send('POST', 'routines/import', { routines: local });
    delete this.ctx.config.values.routines;
    this.ctx.config.save();
  }

  /** Runs the server shows running on this browser that this app is not running: the app quit mid-run, so they were interrupted. */
  private async markInterrupted(): Promise<void> {
    const mine = (r: Routine) =>
      (r.runs || []).filter((run) => run.status === 'running' && run.by === this.ctx.socket.browserId);
    const orphans = this.list.flatMap((r) => mine(r).map((run) => [r.id, run.id]));
    const stale = orphans.filter(([, runId]) => runId !== this.current?.runId);
    for (const [id, runId] of stale) await this.end(id, runId, { status: 'interrupted' });
    if (stale.length) this.list = await readRoutines(this.api);
  }

  /** Sends one change to the server and re-reads the list; answers the snapshot, or `{ error }` saying why not. */
  private async change(method: string, route: string, body?: object): Promise<RoutinesSnapshot | Refusal> {
    if (!this.api.canCall()) return { error: 'Connect to Oya to change routines.' };
    try {
      await this.api.send(method, route, body);
    } catch (e) {
      return { error: (e as Error).message };
    }
    return this.refresh();
  }

  /** Adds a routine, or changes the one with its id (its history is kept). */
  save(input: Partial<Routine> | null | undefined): Promise<RoutinesSnapshot | Refusal> {
    const { id, name, prompt, schedule, enabled, target } = input || {};
    // This desktop's time zone, so a daily time means the same hour when the server runs it in the cloud.
    const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
    const body = { name, prompt, schedule, enabled, target, tz };
    return id ? this.change('PATCH', routineRoute(id), body) : this.change('POST', 'routines', body);
  }

  /** Deletes a routine and its history. */
  remove(id: string): Promise<RoutinesSnapshot | Refusal> {
    return this.change('DELETE', routineRoute(id));
  }

  /** Turns a routine's schedule on or off; a run in progress is not stopped (that is Stop). */
  setEnabled(id: string, enabled: unknown): Promise<RoutinesSnapshot | Refusal> {
    return this.change('PATCH', routineRoute(id), { enabled: !!enabled });
  }

  /** Clears a routine's finished runs. */
  clearHistory(id: string): Promise<RoutinesSnapshot | Refusal> {
    return this.change('DELETE', routineRoute(id, '/runs'));
  }

  /** Stops this app's run of routine `id`, and only that; answers whether one was stopped. */
  stop(id: string): boolean {
    if (this.current?.routineId !== id) return false;
    this.ctx.chatAbort?.abort();
    return true;
  }

  /** Re-reads the routines, then runs the first due one when this app is free. */
  async tick(now = Date.now()): Promise<void> {
    if (!this.api.canCall()) return;
    await this.refresh();
    const due = this.list.find((r) => isDue(r, now));
    if (due && !busyReason(this.api, this.ctx, this.current))
      await this.perform(due, await this.claim(due).catch(() => null));
  }

  /** Runs one routine now: answers at once with the snapshot, or `{ error }` saying why it cannot run. */
  async runNow(id: string): Promise<RoutinesSnapshot | Refusal> {
    const reason = busyReason(this.api, this.ctx, this.current);
    const routine = this.list.find((r) => r.id === id);
    if (reason || !routine) return { error: reason || 'That routine is gone.' };
    const claimed = await this.claim(routine).then(String, (e: Error) => ({ error: e.message }));
    if (typeof claimed !== 'string') return (await this.refresh(), claimed);
    void this.perform(routine, claimed);
    return this.snapshot();
  }

  /** Claims the routine's next run from the server for this browser; answers the run id, or throws (another desktop has it). */
  private async claim(routine: Routine): Promise<string> {
    const runId = randomUUID();
    const body = { lastRunAt: routine.lastRunAt ?? null, runId, browserId: this.ctx.socket.browserId };
    await this.api.send('POST', routineRoute(routine.id, '/claim'), body);
    return runId;
  }

  /** Asks the agent the routine's prompt as the claimed run `runId`, then records how it ended. */
  private async perform(routine: Routine, runId: string | null): Promise<unknown> {
    if (!runId) return this.refresh();
    this.current = { routineId: routine.id, runId };
    await this.refresh();
    const ask = [{ role: 'user', content: routine.prompt }];
    const answer = await this.run(ask).catch((e: Error) => ({ error: e.message }));
    this.current = null;
    await this.end(routine.id, runId, endingOf(answer));
    await this.refresh();
  }

  /** Records a run's ending on the server; one that cannot be sent is left to the server's lease. */
  private end(id: string, runId: string, ending: object): Promise<unknown> {
    return this.api.send('PATCH', routineRoute(id, `/runs/${encodeURIComponent(runId)}`), ending).catch(() => {});
  }
}
