/**
 * Real Electron check of workflow validation: clean runs, assertions,
 * variable redaction, tab isolation, bounded repair, step/resume and stop.
 * Run: npm run test:workflow.
 */
const { app, BrowserWindow, utilityProcess } = require('electron');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const { validate } = require('../../scripts/validation.cjs');
const { normalizeDraft } = require('../../scripts/workflow.cjs');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-workflow-electron-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('remote-debugging-port', '0');
const fixture = http.createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(
    '<!doctype html><title>Workflow fixture</title><label>Name <input aria-label="Name"></label><button id="submit" onclick="document.querySelector(\'output\').textContent=\'Saved\'">Save</button><output>Ready</output>',
  );
});
const windows = [],
  tabs = [];
let counter = 0;
const createTab = (url) => {
  const win = new BrowserWindow({ show: false, webPreferences: { nodeIntegration: false, contextIsolation: true } });
  windows.push(win);
  const id = ++counter;
  win.webContents.debugger.attach('1.3');
  tabs.push({ id, view: win, ready: win.loadURL(url) });
  return id;
};
const control = {
  snapshot: () => ({ mine: false, mode: 'agent' }),
  change: async () => {},
  beginLocalCommand: async () => () => {},
  localClient() {},
};
let active;
/** Validates `draft` and resolves with its finished message and every event. */
async function run(draft, options = {}, onEvent = () => {}) {
  return new Promise((resolve, reject) => {
    const messages = [];
    const timeout = setTimeout(() => {
      active?.dispose();
      reject(new Error('Validation timed out: ' + JSON.stringify(messages)));
    }, 25000);
    validate({
      draft: normalizeDraft(draft),
      options,
      event: (message) => {
        messages.push(message);
        onEvent(message);
        if (message.type === 'finished') {
          clearTimeout(timeout);
          resolve({ message, messages });
        }
      },
      app,
      utilityProcess,
      control,
      tabs: () => tabs,
      createTab,
      closeTab: (id) => tabs.find((t) => t.id === id)?.view.close(),
    })
      .then((session) => {
        active = session;
      })
      .catch((error) => {
        clearTimeout(timeout);
        reject(error);
      });
  });
}
app.whenReady().then(async () => {
  try {
    await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
    const url = `http://127.0.0.1:${fixture.address().port}`;
    const userTab = createTab(url);
    await tabs.find((t) => t.id === userTab).ready;
    const steps = [
      { action: 'navigate', url },
      { action: 'type', text: '{{name}}', candidates: [{ kind: 'label', value: 'Name' }] },
      { action: 'click', candidates: [{ kind: 'role', role: 'button', value: 'Save' }] },
      { action: 'assert_text', expected: 'Saved', candidates: [{ kind: 'css', value: 'output' }] },
    ];
    for (let iteration = 0; iteration < 3; iteration++) {
      const result = await run({ steps }, { vars: { name: 'Private customer' } });
      assert.equal(result.message.status, 'succeeded', JSON.stringify(result.messages));
      assert.equal(result.message.assertions, 1);
      assert.ok(!JSON.stringify(result.messages).includes('Private customer'));
      assert.equal(
        await tabs[0].view.webContents.executeJavaScript('document.querySelector("output").textContent'),
        'Ready',
      );
    }
    const repaired = await run({
      steps: [
        steps[0],
        {
          ...steps[2],
          timeout: 500,
          candidates: [
            { kind: 'css', value: '#old-submit' },
            { kind: 'role', role: 'button', value: 'Save' },
          ],
        },
        steps[3],
      ],
    });
    assert.equal(repaired.message.status, 'succeeded', JSON.stringify(repaired.messages));
    assert.equal(repaired.messages.filter((m) => m.type === 'repair').length, 1);
    const failure = await run({ steps: [steps[0], { ...steps[3], expected: 'Incorrect', timeout: 500 }] });
    assert.equal(failure.message.status, 'failed');
    assert.ok(!failure.messages.some((m) => m.type === 'repair'));
    const multipleTabs = await run({
      steps: [
        steps[0],
        { ...steps[0], tab: 'second' },
        { ...steps[2], tab: 'second' },
        { ...steps[3], tab: 'second' },
        { ...steps[3], expected: 'Ready' },
      ],
    });
    assert.equal(multipleTabs.message.status, 'succeeded', JSON.stringify(multipleTabs.messages));
    let stepped = false;
    const stepping = await run({ steps }, { vars: { name: 'Test' }, command: 'step' }, (message) => {
      if (message.event?.status === 'paused') {
        stepped = true;
        setTimeout(() => active.control('resume'), 50);
      }
    });
    assert.equal(stepping.message.status, 'succeeded');
    assert.ok(stepped);
    const stopped = await run({ steps }, { vars: { name: 'Test' }, command: 'step' }, (message) => {
      if (message.event?.status === 'paused') setTimeout(() => active.control('stop'), 50);
    });
    assert.equal(stopped.message.status, 'stopped');
    console.log(
      'Electron Playwright validation: three clean runs, real assertions, variable redaction, unrelated-tab isolation, bounded repair, step/resume, stop passed',
    );
  } catch (error) {
    console.error(error);
    process.exitCode = 1;
  } finally {
    active?.dispose();
    for (const win of windows) if (!win.isDestroyed()) win.destroy();
    fixture.close();
    fs.rmSync(profile, { recursive: true, force: true });
    app.exit(process.exitCode || 0);
  }
});
