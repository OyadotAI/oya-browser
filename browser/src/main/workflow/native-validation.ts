/** Native workflow validation: run-owned tabs, admitted commands, no worker or debugging protocol. */
import type { Draft, Step } from '../../workflow/index.ts';
import { redact, normalizeDraft } from '../../workflow/index.ts';
import type { DriverTab, PageDriver } from '../actions/driver.ts';
import { withinTime } from '../../shared/within-time.ts';
import type { RunOptions, ValidationControl, ValidationSession } from './validation.ts';
import type { WorkerMessage } from './runs.ts';
import { VALIDATION, NATIVE_VALIDATION } from './constants.ts';
import { nativePreflight, nativeValues, nativeValue } from './native-preflight.ts';
import type { TargetRead } from './native-target.ts';
import { NativeTargets, type Selection } from './native-run-targets.ts';
import { evidenceOmitted } from './native-report.ts';
import { nativeAssertion } from './native-actions.ts';
import { executeNative } from './native-executor.ts';
import { workflowFiles, type WorkflowFile } from './native-files.ts';
/** Composition root supplies only protected tabs and production native page actions. */
export interface NativeValidationDeps {
  /** Normalized saved workflow. */ draft: Draft;
  /** Explicit run controls and variable values. */ options: RunOptions;
  /** Existing workspace event sink. */ event(message: WorkerMessage): void;
  /** Exact application control admission. */ control: ValidationControl;
  /** Existing production native actions. */ driver: PageDriver;
  /** Actual native tabs, not debugging target ids. */ tabs(): DriverTab[];
  /** Opens a protected automation tab. */ createTab(url: string): number;
  /** Closes prior run tabs. */ closeTab(id: number): void;
  /** Tabs retained after completion for inspection. */ leftOpen?: Set<number>;
}
/** Timer waits are rechecked against stop/control before native work. */
const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));
/** One run owns its controls and tab identities; no application active tab is used. */
class NativeValidation {
  /** Composition dependencies. */ private readonly deps: NativeValidationDeps;
  /** Locator resolution and repair reporting. */
  private readonly targets: NativeTargets;
  /** Preflight-approved bounded file snapshots. */
  private readonly files: Map<string, WorkflowFile[]>;
  /** Fully resolved variables. */ private readonly vars: Record<string, unknown>;
  /** Exact tab objects by recorded name. */ private readonly tabs = new Map<string, DriverTab>();
  /** Paused between native operations. */ private paused = false;
  /** Stop dispatch after any admitted action finishes. */ private stopped = false;
  /** Step mode pauses after each step. */ private single = false;
  /** Balanced client ownership. */ private finished = false;
  /** Current step may have changed the site. */ private inputIssued = false;
  /** Current failure attribution. */ private current?: Step;
  /** Paused wait cancellation. */ private wake?: () => void;
  /** Preflight completes before browser ownership is acquired. */
  constructor(deps: NativeValidationDeps, vars: Record<string, unknown>, files: Map<string, WorkflowFile[]>) {
    this.deps = deps;
    this.files = files;
    this.targets = new NativeTargets(deps, vars);
    this.vars = vars;
    this.single = deps.options.command === 'step';
  }
  /** Schedule execution after the workspace receives run controls. */
  start(): ValidationSession {
    this.deps.control.localClient(1);
    setImmediate(() => void this.run());
    return { control: (command) => this.command(command), dispose: () => this.command('stop') };
  }
  /** Resume never reclaims human control automatically. */
  private command(command: string): void {
    if (this.finished) return;
    if (command === 'stop') this.stopped = true;
    if (command === 'pause') this.paused = true;
    if (command === 'resume' || command === 'step') {
      this.paused = false;
      this.single = command === 'step';
    }
    if (!this.paused || this.stopped) this.wake?.();
  }
  /** Redact every event using all string variable values. */
  private emit(message: WorkerMessage): void {
    const secrets = Object.values(this.vars).filter((v): v is string => typeof v === 'string');
    this.deps.event(redact(message, secrets));
  }
  /** Preserve the workspace event contract. */
  private progress(status: string): void {
    const event = { at: Date.now(), kind: 'step', stepId: this.current?.id, status };
    const synthetic = ['select_option', 'upload_file'].includes(this.current?.action || '');
    this.emit({
      type: 'event',
      event: { ...event, ...(synthetic && status === 'passed' ? { inputEvents: 'synthetic' } : {}) },
    });
  }
  /** Paused runs consume no command slot. */
  private async held(): Promise<void> {
    if (this.paused && !this.stopped)
      await new Promise<void>((resolve) => {
        this.wake = resolve;
      });
    this.wake = undefined;
    if (this.stopped) throw new Error('Run stopped');
  }
  /** Recheck control before every native operation and balance admission on failures. */
  private async admitted<T>(operation: () => Promise<T>): Promise<T> {
    await this.held();
    const finish = await this.deps.control.beginLocalCommand();
    try {
      if (this.stopped) throw new Error('Run stopped');
      return await operation();
    } finally {
      finish();
    }
  }
  /** Only exact tabs retained by this run may be read or driven. */
  private tab(step: Step): DriverTab {
    const tab = this.tabs.get(step.tab);
    if (!tab || !this.deps.tabs().includes(tab) || tab.view.webContents.isDestroyed())
      throw new Error('Validation tab is unavailable');
    return tab;
  }
  /** Retire prior run tabs and open protected tabs for recorded names. */
  private async prepare(): Promise<void> {
    await this.admitted(async () => {
      for (const id of this.deps.leftOpen || []) this.deps.closeTab(id);
      this.deps.leftOpen?.clear();
      for (const name of new Set(this.deps.draft.steps.filter((s) => s.enabled).map((s) => s.tab)))
        await this.open(name);
    });
  }
  /** Never drive a partially initialized tab. */
  private async open(name: string): Promise<void> {
    if (this.stopped) throw new Error('Run stopped');
    const id = this.deps.createTab('about:blank#oya-native-run-' + encodeURIComponent(name));
    const tab = this.deps.tabs().find((t) => t.id === id);
    if (!tab) throw new Error('Validation tab is unavailable');
    this.tabs.set(name, tab);
    this.deps.leftOpen?.add(id);
    await withinTime(Promise.resolve(tab.ready), VALIDATION.TAB_OPEN_MS, 'Native validation tab did not open');
  }
  /** Run in order; never resubmit a possibly completed input action. */
  private async run(): Promise<void> {
    const result = await this.outcome();
    await this.targets.dispose();
    this.finished = true;
    this.deps.control.localClient(-1);
    this.emit(result);
  }
  /** Convert failures into a terminal report before releasing run ownership. */
  private async outcome(): Promise<WorkerMessage> {
    try {
      return await this.complete();
    } catch (error) {
      return this.fail(error);
    }
  }
  /** Success is reported only after all enabled steps acknowledge completion. */
  private async complete(): Promise<WorkerMessage> {
    await this.prepare();
    await this.steps();
    const assertions = this.deps.draft.steps.filter((s) => s.enabled && s.action.startsWith('assert_')).length;
    return { type: 'finished', status: 'succeeded', assertions };
  }
  /** Preserve breakpoint, step and run-to controls at boundaries. */
  private async steps(): Promise<void> {
    for (const step of this.deps.draft.steps.filter((s) => s.enabled)) {
      await this.step(step);
    }
  }
  /** One completed step may pause its successor without repeating a breakpoint on resume. */
  private async step(step: Step): Promise<void> {
    Object.assign(this, { current: step, inputIssued: false, paused: this.paused || step.breakpoint });
    if (this.paused) this.progress('paused');
    await this.held();
    this.progress('running');
    await this.execute(step);
    this.progress('passed');
    if (this.deps.options.evidence) this.emit(evidenceOmitted(step.id));
    if (this.single || step.id === this.deps.options.runTo) this.paused = true;
  }
  /** Unknown input outcomes are distinct from inspection failures. */
  private fail(error: unknown): WorkerMessage {
    const status = this.inputIssued ? 'outcome-unknown' : this.stopped ? 'stopped' : 'failed';
    this.progress(status);
    return {
      type: 'finished',
      status,
      stepId: this.current?.id,
      error: failureMessage(error),
    };
  }
  /** Checkpoints release the gate for the human. */
  private async execute(step: Step): Promise<void> {
    if (step.action === 'checkpoint') {
      this.paused = true;
      this.progress('paused');
      return this.held();
    }
    const slowMo = Math.min(Math.max(Number(this.deps.options.slowMo) || 0, 0), VALIDATION.MAX_SLOW_MO_MS);
    if (slowMo) await delay(slowMo);
    await this.poll(step);
  }
  /** Retry only read-only checks, never input. */
  private async poll(step: Step): Promise<void> {
    const deadline = Date.now() + step.timeout;
    do {
      if (await this.admitted(() => this.attempt(step))) return;
      if (Date.now() >= deadline) throw new Error('Native workflow target or assertion did not match before timeout');
      await delay(NATIVE_VALIDATION.POLL_MS);
    } while (!this.stopped);
    throw new Error('Run stopped');
  }
  /** Read and dispatch within one admission. */
  private async attempt(step: Step): Promise<boolean> {
    const tab = this.tab(step);
    const selected = await this.targets.select(step, tab);
    if (step.candidates.length && !selected) return false;
    if (step.action.startsWith('assert_') || step.action === 'wait') return this.assertion(step, selected?.read);
    if (this.stopped) throw new Error('Run stopped');
    this.tab(step);
    await this.input(step, selected);
    return true;
  }
  /** Assertions never dispatch input and may therefore be retried. */
  private assertion(step: Step, read?: TargetRead): boolean {
    return nativeAssertion(step, nativeValue(step.expected, this.vars), read, this.tab(step).view.webContents.getURL());
  }
  /** Dispatch once and conservatively report uncertain outcome if any native action fails. */
  private async input(step: Step, selected?: Selection): Promise<void> {
    this.inputIssued = true;
    const tab = this.tab(step);
    const guard = (): void => {
      if (this.stopped) throw new Error('Run stopped');
      this.tab(step);
    };
    await executeNative({ driver: this.deps.driver, tab, step, vars: this.vars, selected, files: this.files, guard });
  }
}
/** Unsupported drafts are refused before any browser side effect. */
export async function validateNative(input: NativeValidationDeps): Promise<ValidationSession> {
  const deps = snapshot(input);
  const vars = nativeValues(deps.draft, deps.options);
  nativePreflight(deps.draft, vars);
  const files = await workflowFiles(deps.draft, vars);
  if (deps.control.snapshot().mine || deps.control.snapshot().mode === 'human') await deps.control.change('return');
  return new NativeValidation(deps, vars, files).start();
}

/** Preserve immutable run semantics before the first asynchronous ownership change. */
function snapshot(input: NativeValidationDeps): NativeValidationDeps {
  return { ...input, draft: normalizeDraft(structuredClone(input.draft)), options: structuredClone(input.options) };
}

/** Native isolated execution may reject with an Error from a different V8 realm. */
function failureMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string')
    return error.message;
  return typeof error === 'string' ? error : 'Native validation failed';
}
