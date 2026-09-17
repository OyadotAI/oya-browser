const { chromium, expect } = require('@playwright/test');
const { writeFile, rm } = require('node:fs/promises');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { generate } = require('./workflow.cjs');
const { redact, safeUrl } = require('./diagnostics.cjs');
let paused = false, stopped = false, single = false, wake, browser, current, requestedStop = false;
const tell = message => process.parentPort.postMessage(message);
const control = command => {
  if (command === 'stop') { stopped = true; wake?.(); browser?.close().catch(() => {}); }
  if (command === 'pause') paused = true;
  if (command === 'resume' || command === 'step') { single = command === 'step'; paused = false; wake?.(); }
};
process.parentPort.on('message', ({ data }) => {
  if (data.type === 'start') run(data).catch(error => tell({ type: 'finished', status: stopped ? 'stopped' : 'failed', error: redact(error.message) }));
  if (data.type === 'control') control(data.command);
});
function locate(page, candidate) {
  if (candidate.kind === 'css') return page.locator(candidate.value);
  if (candidate.kind === 'role') return page.getByRole(candidate.role, { name: candidate.value, exact: true });
  return page[({ testId: 'getByTestId', label: 'getByLabel', text: 'getByText', placeholder: 'getByPlaceholder' })[candidate.kind]](candidate.value, { exact: true });
}
async function run({ draft, endpoint, token, targetId, pageUrls, vars, directory, command, runTo, evidence, autoHeal = true }) {
  const secrets = Object.values(vars || {}).filter(value => typeof value === 'string');
  const emit = event => tell({ type: 'event', event: { at: Date.now(), ...redact(event, secrets) } });
  const done = new Set(), attempts = new Map(), repairDeadlines = new Map(), pages = new Map();
  let revision = 0, inputIssued = false, stepStarted = 0;
  const codeFile = path.join(directory, 'workflow.mjs');
  single = command === 'step'; paused = false;
  try {
    browser = await chromium.connectOverCDP(endpoint, { headers: { 'X-Oya-Run': token } });
    const available = browser.contexts().flatMap(context => context.pages());
    const page = pageUrls ? available.find(p => p.url() === pageUrls.main) : available[0];
    for (const [name, url] of Object.entries(pageUrls || {})) { const p = available.find(p => p.url() === url); if (!p) throw new Error('A validation tab is unavailable'); pages.set(name, p); }
    if (!page) throw new Error('Validation tab is unavailable');
    pages.set('main', page);
    const attachEvidence = p => {
      p.on('console', msg => { if (['error', 'warning'].includes(msg.type())) emit({ kind: 'console', stepId: current, level: msg.type(), message: 'Page console ' + msg.type() + ' (message omitted)' }); });
      p.on('pageerror', () => emit({ kind: 'console', stepId: current, level: 'error', message: 'Uncaught page error' }));
      p.on('requestfailed', request => emit({ kind: 'network', stepId: current, url: safeUrl(request.url()), message: 'Request failed' }));
      p.on('response', response => emit({ kind: 'network', stepId: current, url: safeUrl(response.url()), status: response.status() }));
      p.on('dialog', async dialog => { emit({ kind: 'attention', stepId: current, message: 'A browser dialog requires a recorded checkpoint. Dialog dismissed.' }); await dialog.dismiss(); });
      p.on('download', download => emit({ kind: 'download', stepId: current, message: 'Download started' }));
    };
    for (const p of pages.values()) attachEvidence(p); page.context().on('page', attachEvidence);
    for (;;) {
      const artifact = generate(draft);
      await writeFile(codeFile, artifact.code, { mode: 0o600 });
      const exported = await import(pathToFileURL(codeFile).href + '?revision=' + revision++);
      let repairSignal = null;
      try {
        await exported.default(page, vars, {
          expect, pages, shouldRun: id => !done.has(id),
          beforeStep: async (id, p) => {
            current = id; inputIssued = false; const step = draft.steps.find(step => step.id === id);
            if (step.breakpoint && !requestedStop) paused = true;
            if (paused) { emit({ kind: 'step', stepId: id, status: 'paused' }); await new Promise(resolve => { wake = resolve; }); wake = null; }
            if (stopped) throw new Error('Run stopped');
            requestedStop = false; inputIssued = false; stepStarted = Date.now();
            emit({ kind: 'step', stepId: id, status: 'running', action: step.action });
            if (step.candidates.length) {
              let scope = p; for (const frame of step.frames) scope = scope.frameLocator(frame);
              const resolved = candidate => ({ ...candidate, value: candidate.value.replace(/\{\{([A-Za-z_]\w*)\}\}/g, (_, key) => String(vars?.[key] ?? draft.variables[key]?.default ?? '')) });
              const countTarget = async locator => {
                const remaining = Math.max(1, Math.min(step.timeout, (repairDeadlines.get(id) || Infinity) - Date.now()));
                let timer;
                try { return await Promise.race([locator.count(), new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Target inspection timed out')), remaining); })]); }
                finally { clearTimeout(timer); }
              };
              const primary = locate(scope, resolved(step.candidates[0]));
              let count = await countTarget(primary);
              if (!count) { await primary.waitFor({ state: 'attached', timeout: Math.max(1, Math.min(step.timeout, (repairDeadlines.get(id) || Infinity) - Date.now())) }).catch(() => {}); count = await countTarget(primary); }
              emit({ kind: 'target', stepId: id, count, message: count === 1 ? 'One matching target' : count ? 'Multiple matching targets' : 'Target not found' });
              if (count !== 1) {
                // Ambiguity is as repairable as absence: another recorded handle for the
                // same element — its id, its name — often still matches exactly one.
                if (autoHeal && !step.action.startsWith('assert_') && (attempts.get(id) || 0) < 2 && Date.now() < (repairDeadlines.get(id) || Infinity)) {
                  if (!repairDeadlines.has(id)) repairDeadlines.set(id, Date.now() + 30000);
                  for (const candidate of step.candidates.slice(1)) if (Date.now() < (repairDeadlines.get(id) || Infinity) && await countTarget(locate(scope, resolved(candidate))) === 1) {
                    if (!repairDeadlines.has(id)) repairDeadlines.set(id, Date.now() + 30000);
                    attempts.set(id, (attempts.get(id) || 0) + 1);
                    const original = step.candidates[0]; step.candidates = [candidate, ...step.candidates.filter(c => c !== candidate)];
                    repairSignal = id;
                    tell({ type: 'repair', stepId: id, original, replacement: candidate, draft });
                    emit({ kind: 'step', stepId: id, status: 'repairing', message: 'A recorded alternative uniquely matches the target.' });
                    throw new Error('RECOMPILE_REPAIRED_STEP');
                  }
                }
                throw new Error(count ? `${count} elements match. Pick a unique target.` : 'Target not found. Pick a replacement or update the wait.');
              }
            }
            // Once dispatched, a failed action may have changed the website.
            inputIssued = ['click', 'press_key', 'upload_file', 'select_option'].includes(step.action);
          },
          checkpoint: async () => { paused = true; emit({ kind: 'attention', stepId: current, status: 'paused', message: 'Complete the checkpoint using Take control, then resume.' }); await new Promise(resolve => { wake = resolve; }); if (stopped) throw new Error('Run stopped'); },
          afterStep: async (id, p) => {
            done.add(id); emit({ kind: 'step', stepId: id, status: 'passed', duration: Date.now() - stepStarted });
            if (evidence) emit({ kind: 'evidence', stepId: id, message: 'Screenshot omitted: safe masking cannot be guaranteed for this page.' });
            if (single || id === runTo) { paused = true; requestedStop = true; }
          },
          failedStep: async (id, error) => { if (!repairSignal) emit({ kind: 'step', stepId: id, status: inputIssued ? 'outcome-unknown' : 'failed', duration: Date.now() - stepStarted, message: error.message === 'Run stopped' ? 'Run stopped' : 'The step did not complete. Check the matching target, timeout, and expected result.' }); },
        });
        tell({ type: 'finished', status: 'succeeded', assertions: draft.steps.filter(step => step.enabled && step.action.startsWith('assert_')).length }); break;
      } catch (error) {
        if (repairSignal) continue;
        tell({ type: 'finished', status: stopped ? 'stopped' : inputIssued ? 'outcome-unknown' : 'failed', error: error.message === 'Run stopped' ? 'Run stopped' : 'The step did not complete. Check the matching target, timeout, and expected result.', stepId: current }); break;
      }
    }
  } finally { await browser?.close().catch(() => {}); await rm(codeFile, { force: true }); }
}
