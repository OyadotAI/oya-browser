/**
 * The live run in the Ask pane. While the agent works, its card under the
 * question shows the plan it wrote, ticking off steps, and a timeline of what
 * it did, one narrated line per action, from the server's agent events. With a
 * server that sends none, the live line follows the browser's commands instead
 * (the activity log's `cmd:` entries). When the answer comes, the card folds
 * into a one-line summary, and the panel's orb shows how it went, then rests.
 *
 * ponytail: the activity entries are socket commands, not the model's tool
 * calls: one `click` shows "Click" then "Reading the page", and a command
 * another client sends mid-question counts as a step. Stream the real tool
 * calls through the chat response if that is ever not close enough.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OrbState } from '../../../ui/index.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { AgentEvent } from '../../../../shared/ipc.ts';
import type { AgentRunEvent, DevLogEntry, PlanStep } from '../model/types.ts';
import { commandAction, commandParams, elapsedSeconds, runSummary, stepLabel } from '../model/steps.ts';

/** One narrated action in a run's timeline. */
export interface StepRow {
  /** What the agent did. */
  line: string;
  /** When, as seconds into the run ("4s"). */
  time: string;
}

/** A run's card: its plan and its timeline. */
export interface RunCard {
  /** The plan the agent wrote. */
  plan: PlanStep[];
  /** What it did so far. */
  steps: StepRow[];
}

/** A finished run's card, folded behind its summary when it had steps. */
export interface FinishedRun extends RunCard {
  /** How it went. */
  outcome: 'done' | 'failed';
  /** "3 steps · 12s", or '' for a run with a plan and no steps (shown open, without a summary). */
  summary: string;
}

/** What the live run shows. */
export interface RunState {
  /** The card of the run in flight, or null. */
  card: RunCard | null;
  /** What the latest command is doing ('' before any, or after a narrated step). */
  label: string;
  /** Steps so far. */
  count: number;
  /** Seconds since the question was asked. */
  seconds: number;
  /** The panel orb's state. */
  orb: OrbState;
  /** Counts the steps, so the panel orb pulses once for each. */
  pulse: number;
}

/** The parts of the bridge the live run hears. */
export type RunBridge = Pick<OyaBrowser, 'onAgentEvent' | 'onDevLog'>;

/** The live run card, the step line and the panel's orb. */
export class RunViewModel extends ViewModel<RunState> {
  /** The server's id for this run, from its start event; events of any other run are ignored. */
  private runId: string | null = null;
  /** This run has had agent events, so they, not the browser's commands, name the steps. */
  private live = false;
  /** When the run began. */
  private started = 0;
  /** The timer that keeps the elapsed count moving. */
  private ticker: ReturnType<typeof setInterval> | undefined;
  /** The timer that rests the orb after a run. */
  private restTimer: ReturnType<typeof setTimeout> | undefined;

  /** Resting, hearing the agent's events and the activity log. */
  constructor(bridge: RunBridge) {
    super({ card: null, label: '', count: 0, seconds: 0, orb: 'idle', pulse: 0 });
    this.own(bridge.onAgentEvent((payload) => this.event(payload)));
    this.own(bridge.onDevLog((entry) => this.note(entry as DevLogEntry)));
    this.own(() => this.stopTimers());
  }

  /** A question was asked: a new card with its live line, and the orb starts thinking. */
  begin(): void {
    this.stopTimers();
    Object.assign(this, { runId: null, live: false, started: Date.now() });
    this.set({ card: { plan: [], steps: [] }, label: '', count: 0, seconds: 0, orb: 'thinking' });
    this.ticker = setInterval(() => this.tick(), C.CHAT_PROGRESS_TICK_MS);
  }

  /** The answer came: the card folds (or goes, with neither steps nor a plan) and the orb shows how it went. */
  finish(ok: boolean): FinishedRun | null {
    const card = this.state.card;
    if (!card) return null;
    clearInterval(this.ticker);
    this.set({ card: null });
    this.settle(ok);
    if (!card.steps.length && !card.plan.length) return null;
    const summary = card.steps.length ? runSummary(card.steps.length, this.elapsed()) : '';
    return { ...card, outcome: ok ? 'done' : 'failed', summary };
  }

  /** The chat was cleared mid-run: the card goes with it, and the orb rests. */
  reset(): void {
    this.stopTimers();
    Object.assign(this, { runId: null, live: false });
    this.set({ card: null, orb: 'idle' });
  }

  /** One event from the agent: its run's start names it; until then, and for any other run, events are ignored. */
  private event({ runId, event }: AgentEvent): void {
    const run = event as AgentRunEvent | undefined;
    if (!this.state.card || !run) return;
    if (run.kind === 'start' && !this.runId) this.runId = runId;
    if (!this.runId || runId !== this.runId) return;
    if (run.kind === 'plan') this.setPlan(run.steps ?? []);
    if (run.kind === 'step') this.step(run.line || run.tool || '');
  }

  /** Redraws the plan. */
  private setPlan(plan: PlanStep[]): void {
    this.live = true;
    this.set({ card: { ...this.state.card!, plan } });
  }

  /** Adds one narrated action; the live line is what comes next, its count the rows so far. */
  private step(line: string): void {
    this.live = true;
    const card = this.state.card!;
    const steps = [...card.steps, { line, time: `${this.elapsed()}s` }];
    const { pulse } = this.state;
    this.set({ card: { ...card, steps }, label: '', count: steps.length, pulse: pulse + 1, seconds: this.elapsed() });
  }

  /** Counts one command the server sent while a question is in flight, unless the agent narrates the run. */
  private note(entry: DevLogEntry): void {
    const action = commandAction(entry);
    if (!this.state.card || this.live || !action) return;
    const label = stepLabel(action, commandParams(entry));
    this.set({ label, count: this.state.count + 1, seconds: this.elapsed() });
  }

  /** Moves the elapsed count. */
  private tick(): void {
    this.set({ seconds: this.elapsed() });
  }

  /** The orb shows done or failed for a moment, then rests. */
  private settle(ok: boolean): void {
    clearTimeout(this.restTimer);
    this.set({ orb: ok ? 'done' : 'failed' });
    this.restTimer = setTimeout(() => this.set({ orb: 'idle' }), C.CHAT_ORB_REST_MS);
  }

  /** Seconds since the run began. */
  private elapsed(): number {
    return elapsedSeconds(this.started, Date.now());
  }

  /** Stops the ticker and the orb's rest. */
  private stopTimers(): void {
    clearInterval(this.ticker);
    clearTimeout(this.restTimer);
  }
}
