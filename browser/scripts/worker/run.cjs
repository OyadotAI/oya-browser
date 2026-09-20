/**
 * One validation run: connect to the run's tabs over the front door, then
 * generate the draft's module and run it with the step hooks, regenerating
 * after each repair.
 */
const { chromium, expect } = require('@playwright/test');
const { writeFile, rm } = require('node:fs/promises');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { generate } = require('../workflow.cjs');
const { redact } = require('../diagnostics.cjs');
const { CRYPTO } = require('../constants.cjs');
const { attachEvidence } = require('./evidence.cjs');
const { stepHooks, failureMessage } = require('./hooks.cjs');

/** Everything one run shares across its steps. */
function runContext(options, state, tell) {
  const { draft, vars, runTo, evidence, autoHeal = true } = options;
  const secrets = Object.values(vars || {}).filter((value) => typeof value === 'string');
  const emit = (event) => tell({ type: 'event', event: { at: Date.now(), ...redact(event, secrets) } });
  const maps = { done: new Set(), attempts: new Map(), repairDeadlines: new Map(), pages: new Map() };
  const progress = { revision: 0, inputIssued: false, stepStarted: 0, repairSignal: null };
  return { draft, vars, runTo, evidence, autoHeal, state, tell, emit, ...maps, ...progress };
}

/** Maps each named run tab to its page; a missing one fails the run. */
function mapPages(run, available, pageUrls) {
  for (const [name, url] of Object.entries(pageUrls || {})) {
    const p = available.find((candidate) => candidate.url() === url);
    if (!p) throw new Error('A validation tab is unavailable');
    run.pages.set(name, p);
  }
}

/** Finds the run's tabs by address; the main one drives the module. */
function findPages(run, browser, pageUrls) {
  const available = browser.contexts().flatMap((context) => context.pages());
  const page = pageUrls ? available.find((p) => p.url() === pageUrls.main) : available[0];
  mapPages(run, available, pageUrls);
  if (!page) throw new Error('Validation tab is unavailable');
  run.pages.set('main', page);
  return page;
}

/** Watches every run tab, and any the run opens, for evidence. */
function watchPages(run, page) {
  const watch = (p) => attachEvidence(p, run.emit, run.state);
  for (const p of run.pages.values()) watch(p);
  page.context().on('page', watch);
}

/** Generates the module afresh and imports it (a new revision each time, so repairs take effect). */
async function loadModule(run, codeFile) {
  const artifact = generate(run.draft);
  await writeFile(codeFile, artifact.code, { mode: CRYPTO.FILE_MODE });
  return import(pathToFileURL(codeFile).href + '?revision=' + run.revision++);
}

/** The final report for a run that threw. */
function failure(run, error) {
  const status = run.state.stopped ? 'stopped' : run.inputIssued ? 'outcome-unknown' : 'failed';
  return { type: 'finished', status, error: failureMessage(error), stepId: run.state.current };
}

/** Reports success, with how many assertions ran. */
function succeed(run) {
  const assertions = run.draft.steps.filter((step) => step.enabled && step.action.startsWith('assert_')).length;
  run.tell({ type: 'finished', status: 'succeeded', assertions });
}

/** Runs the module once; true when the run is over, false when a repair needs a rerun. */
async function attempt(run, page, exported) {
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
async function replay(run, page, codeFile) {
  for (;;) {
    const exported = await loadModule(run, codeFile);
    run.repairSignal = null;
    if (await attempt(run, page, exported)) return;
  }
}

/** Connects over the front door and finds and watches the run's tabs; returns the main page. */
async function connect(ctx, options, state) {
  state.browser = await chromium.connectOverCDP(options.endpoint, { headers: { 'X-Oya-Run': options.token } });
  const page = findPages(ctx, state.browser, options.pageUrls);
  watchPages(ctx, page);
  return page;
}

/** Closes the connection and deletes the generated module, whatever happened. */
async function disconnect(state, codeFile) {
  await state.browser?.close().catch(() => {});
  await rm(codeFile, { force: true });
}

/** Connects to the run's tabs and replays the draft; always disconnects and deletes the module. */
async function run(options, state, tell) {
  const ctx = runContext(options, state, tell);
  const codeFile = path.join(options.directory, 'workflow.mjs');
  Object.assign(state, { single: options.command === 'step', paused: false });
  try {
    await replay(ctx, await connect(ctx, options, state), codeFile);
  } finally {
    await disconnect(state, codeFile);
  }
}

module.exports = { run };
