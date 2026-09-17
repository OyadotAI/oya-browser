const { randomBytes } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { once } = require('node:events');
const { start } = require('../cdp-front-door');
// An isolated worker drives only fresh validation tabs, using the installed Chromium.
async function validate({ draft, options, event, app, utilityProcess, control, tabs, createTab, closeTab, cdpPort }) {
  const token = randomBytes(32).toString('hex'), targets = new Set(), runTabs = new Set();
  let server, worker, finished = false, stopTimer;
  const directory = fs.mkdtempSync(path.join(app.getPath('temp'), 'oya-validation-'));
  const cleanup = () => {
    if (finished) return; finished = true; clearTimeout(stopTimer);
    server?.close(); worker?.kill(); fs.rmSync(directory, { recursive: true, force: true });
  };
  const open = async url => {
    const id = createTab(url); runTabs.add(id); const tab = tabs().find(t => t.id === id);
    await tab.ready;
    const { targetInfo } = await tab.view.webContents.debugger.sendCommand('Target.getTargetInfo');
    tab.targetId = targetInfo.targetId; targets.add(targetInfo.targetId); return tab;
  };
  try {
    if (control.snapshot().mine || control.snapshot().mode === 'human') await control.change('return');
    const tab = await open('about:blank#oya-run-main');
    const pageUrls = { main: 'about:blank#oya-run-main' };
    for (const name of new Set(draft.steps.filter(step => step.enabled).map(step => step.tab))) {
      if (name === 'main') continue;
      pageUrls[name] = 'about:blank#oya-run-' + encodeURIComponent(name);
      await open(pageUrls[name]);
    }
    // Chromium writes the ephemeral debugging port to its profile directory.
    let upstream = cdpPort;
    for (let i = 0; !upstream && i < 50; i++) {
      try { upstream = Number(fs.readFileSync(path.join(app.getPath('userData'), 'DevToolsActivePort'), 'utf8').split('\n')[0]); } catch {}
      if (!upstream) await new Promise(resolve => setTimeout(resolve, 100));
    }
    if (!upstream) throw new Error('The browser debugging endpoint did not start. Restart Oya Browser.');
    server = start({ port: 0, upstream, host: '127.0.0.1', runToken: token, allowedTarget: id => targets.has(id),
      tabs: () => tabs().filter(t => runTabs.has(t.id)),
      // The proxy awaits tab.ready; add its target before exposing it to Playwright.
      createTab: url => { const id = createTab(url); runTabs.add(id); const t = tabs().find(t => t.id === id); const ready = t.ready; t.ready = ready.then(async () => { const { targetInfo } = await t.view.webContents.debugger.sendCommand('Target.getTargetInfo'); t.targetId = targetInfo.targetId; targets.add(t.targetId); }); return id; },
      closeTab, beginCommand: () => control.beginLocalCommand(), clientChanged: delta => control.localClient(delta) });
    await once(server, 'listening');
    worker = utilityProcess.fork(path.join(__dirname, 'workflow-worker.cjs'), [], { serviceName: 'Oya Playwright validation', stdio: 'pipe' });
    worker.on('message', message => { try { event(message); } finally { if (message.type === 'finished') cleanup(); } });
    worker.on('exit', () => { if (!finished) { event({ type: 'finished', status: 'interrupted', error: 'Validation process exited. No step was automatically resubmitted.' }); cleanup(); } });
    worker.postMessage({ type: 'start', draft, endpoint: `http://127.0.0.1:${server.address().port}`, token, targetId: tab.targetId, pageUrls, vars: options.vars || {}, directory, command: options.command, runTo: options.runTo, evidence: !!options.evidence, autoHeal: options.autoHeal !== false });
    return { control(command) { if (finished) return; worker.postMessage({ type: 'control', command }); if (command === 'stop') stopTimer = setTimeout(() => { event({ type: 'finished', status: 'interrupted', error: 'Worker stopped. The last website action may have completed; check before retrying.' }); cleanup(); }, 5000); }, dispose: cleanup };
  } catch (error) { cleanup(); throw error; }
}
module.exports = { validate };
