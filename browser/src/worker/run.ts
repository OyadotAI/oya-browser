/**
 * One validation run: connect to the run's tabs over the front door, then
 * generate the draft's module and run it with the step hooks, regenerating
 * after each repair.
 */
import { chromium, expect, type Browser, type Page } from '@playwright/test';
import { writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { generate, redact } from '../workflow/index.ts';
import { MODULE_FILE_MODE } from './constants.ts';
import { attachEvidence } from './evidence.ts';
import { stepHooks, type StepHooks } from './hooks.ts';
import { failureMessage } from './failure.ts';
import type { RunState } from './run-state.ts';
import type { Run, RunEvent, StartOptions, Tell } from './types.ts';

/** What the generated module exports. */
interface GeneratedModule {
  /** Runs the workflow on `page` with the run's values and hooks. */
  default: (page: Page, vars: Record<string, unknown> | undefined, hooks: StepHooks) => Promise<void>;
}

/** Everything one run shares across its steps. */
function runContext(options: StartOptions, state: RunState, tell: Tell): Run {
  const { draft, vars, runTo, evidence, autoHeal = true, slowMo = 0 } = options;
  const secrets = Object.values(vars || {}).filter((value) => typeof value === 'string');
  const emit = (event: RunEvent) => tell({ type: 'event', event: { at: Date.now(), ...redact(event, secrets) } });
  const redactMessage = (message: string) => redact(message, secrets);
  const maps = { done: new Set<string>(), attempts: new Map(), repairDeadlines: new Map(), pages: new Map() };
  const progress = { revision: 0, inputIssued: false, stepStarted: 0, repairSignal: null };
  const settings = { draft, vars, runTo, evidence, autoHeal, slowMo };
  return { ...settings, state, tell, emit, redactMessage, ...maps, ...progress };
}

/** Maps each named run tab to its page; a missing one fails the run. */
function mapPages(run: Run, available: Page[], pageUrls: Record<string, string> | undefined): void {
  for (const [name, url] of Object.entries(pageUrls || {})) {
    const p = available.find((candidate) => candidate.url() === url);
    if (!p) throw new Error('A validation tab is unavailable');
    run.pages.set(name, p);
  }
}

/** Finds the run's tabs by address; the main one drives the module. */
function findPages(run: Run, browser: Browser, pageUrls: Record<string, string> | undefined): Page {
  const available = browser.contexts().flatMap((context) => context.pages());
  const page = pageUrls ? available.find((p) => p.url() === pageUrls.main) : available[0];
  mapPages(run, available, pageUrls);
  if (!page) throw new Error('Validation tab is unavailable');
  run.pages.set('main', page);
  return page;
}

/** Watches every run tab, and any the run opens, for evidence. */
function watchPages(run: Run, page: Page): void {
  const watch = (p: Page) => attachEvidence(p, run.emit, run.state);
  for (const p of run.pages.values()) watch(p);
  page.context().on('page', watch);
}

/** Generates the module afresh and imports it (a new revision each time, so repairs take effect). */
async function loadModule(run: Run, codeFile: string): Promise<GeneratedModule> {
  const artifact = generate(run.draft);
  await writeFile(codeFile, artifact.code, { mode: MODULE_FILE_MODE });
  return import(pathToFileURL(codeFile).href + '?revision=' + run.revision++);
}

/** The final report for a run that threw. */
function failure(run: Run, error: unknown): Record<string, unknown> {
  const status = run.state.stopped ? 'stopped' : run.inputIssued ? 'outcome-unknown' : 'failed';
  const step = run.draft.steps.find((s) => s.id === run.state.current);
  return { type: 'finished', status, error: run.redactMessage(failureMessage(error, step)), stepId: run.state.current };
}

/** Reports success, with how many assertions ran. */
function succeed(run: Run): void {
  const assertions = run.draft.steps.filter((step) => step.enabled && step.action.startsWith('assert_')).length;
  run.tell({ type: 'finished', status: 'succeeded', assertions });
}

/** Runs the module once; true when the run is over, false when a repair needs a rerun. */
async function attempt(run: Run, page: Page, exported: GeneratedModule): Promise<boolean> {
  try {
    await exported.default(page, run.vars, stepHooks(run, expect));
    succeed(run);
  } catch (error) {
    if (run.repairSignal) return false;
    run.tell(failure(run, error));
  }
  return true;
}

/** Runs until the module finishes or fails, regenerating after each repair. */
async function replay(run: Run, page: Page, codeFile: string): Promise<void> {
  for (;;) {
    const exported = await loadModule(run, codeFile);
    run.repairSignal = null;
    if (await attempt(run, page, exported)) return;
  }
}

/** Connects over the front door and finds and watches the run's tabs; returns the main page. */
async function connect(ctx: Run, options: StartOptions, state: RunState): Promise<Page> {
  state.browser = await chromium.connectOverCDP(options.endpoint, { headers: { 'X-Oya-Run': options.token } });
  const page = findPages(ctx, state.browser, options.pageUrls);
  watchPages(ctx, page);
  return page;
}

/** Closes the connection and deletes the generated module, whatever happened. */
async function disconnect(state: RunState, codeFile: string): Promise<void> {
  await state.browser?.close().catch(() => {});
  await rm(codeFile, { force: true });
}

/** Connects to the run's tabs and replays the draft; always disconnects and deletes the module. */
export async function run(options: StartOptions, state: RunState, tell: Tell): Promise<void> {
  const ctx = runContext(options, state, tell);
  const codeFile = path.join(options.directory, 'workflow.mjs');
  Object.assign(state, { single: options.command === 'step', paused: false });
  try {
    await replay(ctx, await connect(ctx, options, state), codeFile);
  } finally {
    await disconnect(state, codeFile);
  }
}
