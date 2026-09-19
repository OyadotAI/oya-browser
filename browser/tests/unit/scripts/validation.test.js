/**
 * Unit tests for scripts/validation.cjs: preparing a run's tabs, the
 * run-scoped CDP front door, the worker, run controls and cleanup. The front
 * door and the utility process are faked; nothing listens or forks.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { FakeView, freshRequire, flush } = require('../support/fakes.cjs');
const { normalizeDraft } = require('../../../scripts/workflow.cjs');

/** Replaces the CDP front door with a fake that records its options. */
function fakeFrontDoor() {
  const door = { options: null, server: null };
  const id = require.resolve('../../../cdp-front-door.js');
  const previous = require.cache[id];
  const start = (options) => {
    door.options = options;
    door.server = Object.assign(new EventEmitter(), { closed: false, address: () => ({ port: 4567 }) });
    door.server.close = () => (door.server.closed = true);
    setImmediate(() => door.server.emit('listening'));
    return door.server;
  };
  require.cache[id] = { id, filename: id, loaded: true, exports: { start } };
  door.restore = () => (previous ? (require.cache[id] = previous) : delete require.cache[id]);
  return door;
}

/** The browser side validate() needs: tabs, control, the app and the utility process. */
function fakeBrowserSide(tmp, { mode = 'agent', cdpPort = 9222 } = {}) {
  const tabs = [];
  const events = [];
  const worker = Object.assign(new EventEmitter(), { posted: [], killed: false });
  worker.postMessage = (m) => worker.posted.push(m);
  worker.kill = () => (worker.killed = true);
  const createTab = (url) => {
    const id = tabs.length + 1;
    const view = new FakeView({
      url,
      debuggerResponses: { 'Target.getTargetInfo': { targetInfo: { targetId: 'T' + id } } },
    });
    tabs.push({ id, url, view, ready: Promise.resolve() });
    return id;
  };
  const control = { changes: [], snapshot: () => ({ mode }), change: async (c) => control.changes.push(c) };
  Object.assign(control, { beginLocalCommand: () => 'begun', localClient: (d) => d });
  const app = { getPath: (name) => path.join(tmp, name) };
  const utilityProcess = { fork: (file, args, options) => Object.assign(worker, { file, options }) };
  return { tabs, events, worker, control, app, utilityProcess, createTab, deps: null, cdpPort };
}

describe('validate', () => {
  let tmp;
  let door;
  let validate;
  beforeEach(() => {
    tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-validate-'));
    fs.mkdirSync(path.join(tmp, 'temp'));
    fs.mkdirSync(path.join(tmp, 'userData'));
    door = fakeFrontDoor();
    ({ validate } = freshRequire('scripts/validation.cjs'));
  });
  afterEach(() => {
    mock.timers.reset();
    door.restore();
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  /** Starts a validation of `steps` with the given browser side and options. */
  function start(side, steps = [{ action: 'press_key', key: 'A' }], options = {}) {
    const draft = normalizeDraft({ steps });
    return validate({
      draft,
      options,
      event: (e) => side.events.push(e),
      ...{ app: side.app, utilityProcess: side.utilityProcess, control: side.control },
      ...{ tabs: () => side.tabs, createTab: side.createTab, closeTab: () => {}, cdpPort: side.cdpPort },
    });
  }

  it('opens a tab per named tab, starts the front door and the worker, and tells it to start', async () => {
    const side = fakeBrowserSide(tmp);
    await start(
      side,
      [
        { action: 'press_key', key: 'A' },
        { action: 'press_key', key: 'B', tab: 'second tab' },
      ],
      { vars: { a: '1' } },
    );
    assert.deepEqual(
      side.tabs.map((t) => t.url),
      ['about:blank#oya-run-main', 'about:blank#oya-run-second%20tab'],
    );
    assert.equal(door.options.upstream, 9222);
    assert.equal(door.options.host, '127.0.0.1');
    const message = side.worker.posted[0];
    assert.equal(message.type, 'start');
    assert.equal(message.endpoint, 'http://127.0.0.1:4567');
    assert.match(message.token, /^[0-9a-f]{64}$/);
    assert.equal(message.token, door.options.runToken);
    assert.deepEqual(
      [message.targetId, message.vars, message.autoHeal, message.evidence],
      ['T1', { a: '1' }, true, false],
    );
    assert.deepEqual(message.pageUrls, {
      main: 'about:blank#oya-run-main',
      'second tab': 'about:blank#oya-run-second%20tab',
    });
    assert.ok(fs.existsSync(message.directory));
    assert.match(side.worker.file, /workflow-worker\.cjs$/);
  });

  it('lets the worker reach only the run’s own tabs', async () => {
    const side = fakeBrowserSide(tmp);
    side.createTab('https://unrelated.test');
    await start(side);
    assert.ok(door.options.allowedTarget('T2'));
    assert.ok(!door.options.allowedTarget('T1'));
    assert.deepEqual(
      door.options.tabs().map((t) => t.id),
      [2],
    );
  });

  it('registers a tab the worker opens once it is ready', async () => {
    const side = fakeBrowserSide(tmp);
    await start(side);
    const id = door.options.createTab('about:blank');
    await side.tabs.find((t) => t.id === id).ready;
    assert.ok(door.options.allowedTarget('T' + id));
    assert.equal(door.options.beginCommand(), 'begun');
  });

  it('takes control back from a person first', async () => {
    const side = fakeBrowserSide(tmp, { mode: 'human' });
    await start(side);
    assert.deepEqual(side.control.changes, ['return']);
  });

  it('reads the debugging port Chromium writes when none is given', async () => {
    const side = fakeBrowserSide(tmp, { cdpPort: 0 });
    fs.writeFileSync(path.join(tmp, 'userData', 'DevToolsActivePort'), '9333\n/devtools/browser/x');
    await start(side);
    assert.equal(door.options.upstream, 9333);
  });

  it('fails and cleans up when the debugging port never appears', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const side = fakeBrowserSide(tmp, { cdpPort: 0 });
    const run = start(side);
    run.catch(() => {});
    for (let i = 0; i < 60; i++) {
      await flush();
      mock.timers.tick(100);
    }
    await assert.rejects(run, /debugging endpoint did not start/);
    assert.deepEqual(fs.readdirSync(path.join(tmp, 'temp')), []);
  });

  it('fails instead of waiting forever when a validation tab never opens', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const side = fakeBrowserSide(tmp);
    const createTab = side.createTab;
    side.createTab = (url) => {
      const id = createTab(url);
      side.tabs.at(-1).ready = new Promise(() => {});
      return id;
    };
    const run = start(side);
    run.catch(() => {});
    await flush();
    mock.timers.tick(15000);
    await assert.rejects(run, /validation tab did not open/);
  });

  it('relays worker messages and cleans up once the run finishes', async () => {
    const side = fakeBrowserSide(tmp);
    await start(side);
    side.worker.emit('message', { type: 'event', event: {} });
    side.worker.emit('message', { type: 'finished', status: 'succeeded' });
    assert.deepEqual(
      side.events.map((e) => e.type),
      ['event', 'finished'],
    );
    assert.ok(side.worker.killed && door.server.closed);
    assert.deepEqual(fs.readdirSync(path.join(tmp, 'temp')), []);
  });

  it('reports an interrupted run when the worker exits early, but not after it finished', async () => {
    const side = fakeBrowserSide(tmp);
    await start(side);
    side.worker.emit('exit');
    side.worker.emit('exit');
    assert.deepEqual(
      side.events.map((e) => e.status),
      ['interrupted'],
    );
  });

  it('passes controls on, and reports a stop the worker never confirms', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const side = fakeBrowserSide(tmp);
    const run = await start(side);
    run.control('pause');
    run.control('stop');
    assert.deepEqual(side.worker.posted.slice(1), [
      { type: 'control', command: 'pause' },
      { type: 'control', command: 'stop' },
    ]);
    mock.timers.tick(5000);
    assert.equal(side.events[0].status, 'interrupted');
    assert.match(side.events[0].error, /Worker stopped/);
    run.control('resume');
    assert.equal(side.worker.posted.length, 3, 'controls after the end are ignored');
  });

  it('cleans up on dispose, once', async () => {
    const side = fakeBrowserSide(tmp);
    const run = await start(side);
    run.dispose();
    run.dispose();
    assert.ok(side.worker.killed && door.server.closed);
  });
});
