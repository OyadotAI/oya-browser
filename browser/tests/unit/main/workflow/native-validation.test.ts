/** Native validation enforces preflight, exact tab ownership, stop and control gates without a network or debugger. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { normalizeDraft } from '../../../../src/workflow/index.ts';
import { validateNative } from '../../../../src/main/workflow/native-validation.ts';
import type { NativeValidationDeps } from '../../../../src/main/workflow/native-validation.ts';
import type { WorkerMessage } from '../../../../src/main/workflow/runs.ts';

/** One hermetic application boundary; any debugging access is fatal. */
function fixture(steps: unknown[], options = {}) {
  const calls: unknown[][] = [];
  const tabs: any[] = [];
  const events: WorkerMessage[] = [];
  let finish!: (message: WorkerMessage) => void;
  const done = new Promise<WorkerMessage>((resolve) => {
    finish = resolve;
  });
  const state = {
    human: false,
    paint: async () => {},
    hover: async () => true,
    read: async () => ({ count: 1, recorded: true, editable: true, reference: '7', value: 'hello' }),
    locate: async () => ({ ok: true, data: { x: 5, y: 5 } }),
  };
  const driver: any = {
    deps: {
      worldEval: async (...args) => {
        calls.push(['read', args[0]]);
        const code = String(args[1]);
        if (code.includes('const strategies')) return state.read();
        if (code.includes('return slot.pointerReached')) return state.hover();
        if (code.includes('elementFromPoint')) return { x: 5, y: 5 };
        if (code.includes('return {type:')) return { type: 'text' };
        return true;
      },
    },
    locate: async () => state.locate(),
    mouse: { move: async (view) => calls.push(['move', view]) },
    keyboard: {
      clear: async (view) => calls.push(['clear', view]),
      type: async (view, text) => calls.push(['type', view, text]),
    },
  };
  const deps: NativeValidationDeps = {
    draft: normalizeDraft({ steps }),
    options,
    driver,
    event: (message) => {
      events.push(message);
      if (message.type === 'finished') finish(message);
    },
    control: {
      snapshot: () => ({ mode: 'agent' }),
      change: async () => {},
      localClient: (delta) => calls.push(['client', delta]),
      beginLocalCommand: async () => {
        if (state.human) throw new Error('Automation paused for human control');
        calls.push(['begin']);
        return () => {
          calls.push(['end']);
        };
      },
    },
    tabs: () => tabs,
    createTab: (url) => {
      const webContents = Object.assign(new EventEmitter(), {
        isDestroyed: () => false,
        getURL: () => url,
        getZoomFactor: () => 1,
        capturePage: () => state.paint(),
        sendInputEvent: (event) => calls.push(['input', event.type]),
      });
      Object.defineProperty(webContents, 'debugger', {
        get: () => {
          throw new Error('Debugger forbidden');
        },
      });
      const tab = { id: tabs.length + 1, title: 'Run', url, view: { webContents }, ready: Promise.resolve() };
      tabs.push(tab);
      calls.push(['open', tab.id]);
      return tab.id;
    },
    closeTab: (id) => {
      calls.push(['close', id]);
    },
    leftOpen: new Set(),
  };
  return { deps, calls, tabs, events, done, state };
}
/** Let the scheduled native run reach its next async boundary. */
const turn = (): Promise<void> => new Promise((resolve) => setImmediate(resolve));

it('rejects unsupported later steps before control, tab, or native work', async () => {
  const f = fixture([
    { action: 'navigate', url: 'https://example.test' },
    { action: 'upload_file', file: 'secret', candidates: [{ kind: 'css', value: '#file' }] },
  ]);
  await assert.rejects(validateNative(f.deps), /absolute file path/);
  assert.deepEqual(f.calls, []);
});
it('rejects unsupported locators and missing variables without taking human control', async () => {
  const f = fixture([{ action: 'click', candidates: [{ kind: 'css', value: 'button' }] }]);
  f.deps.draft.steps[0].candidates[0].kind = 'unknown';
  f.deps.control.snapshot = () => ({ mine: true, mode: 'human' });
  f.deps.control.change = async () => {
    throw new Error('Must not reclaim control');
  };
  await assert.rejects(validateNative(f.deps), /Invalid locator candidate/);
  const missing = fixture([{ action: 'type', text: '{{secret}}', candidates: [{ kind: 'css', value: '#field' }] }]);
  await assert.rejects(validateNative(missing.deps), /Missing variable/);
  assert.deepEqual(missing.calls, []);
});
it('clears empty replacement text and types only into the exact run tab', async () => {
  const f = fixture([{ action: 'type', text: '', candidates: [{ kind: 'css', value: '#field' }] }]);
  await validateNative(f.deps);
  assert.equal((await f.done).status, 'succeeded');
  assert.deepEqual(
    f.calls.filter((c) => c[0] === 'clear'),
    [['clear', f.tabs[0].view]],
  );
  assert.deepEqual(
    f.calls.filter((c) => c[0] === 'input'),
    [
      ['input', 'mouseDown'],
      ['input', 'mouseUp'],
    ],
  );
  assert.equal(f.calls.filter((c) => c[0] === 'type').length, 0);
  assert.deepEqual(
    f.calls.filter((c) => c[0] === 'client'),
    [
      ['client', 1],
      ['client', -1],
    ],
  );
});
it('reports input failure as outcome unknown and never replays input', async () => {
  const f = fixture([{ action: 'type', text: 'hello', candidates: [{ kind: 'css', value: '#field' }] }]);
  f.deps.driver.keyboard.type = async () => {
    f.calls.push(['failure']);
    throw new Error('Transport ended');
  };
  await validateNative(f.deps);
  assert.equal((await f.done).status, 'outcome-unknown');
  assert.equal(f.calls.filter((c) => c[0] === 'failure').length, 1);
  assert.equal(f.calls.filter((c) => c[0] === 'begin').length, f.calls.filter((c) => c[0] === 'end').length);
});
it('stop during an awaited target read prevents native input dispatch', async () => {
  const f = fixture([{ action: 'type', text: 'hello', candidates: [{ kind: 'css', value: '#field' }] }]);
  let release!: () => void;
  f.state.read = async () => {
    await new Promise<void>((r) => {
      release = r;
    });
    return { count: 1, recorded: true, editable: true, reference: '7', value: 'hello' };
  };
  const run = await validateNative(f.deps);
  await turn();
  run.control('stop');
  release();
  assert.equal((await f.done).status, 'stopped');
  assert.equal(f.calls.filter((c) => c[0] === 'click').length, 0);
});
it('replacement tabs cannot inherit an old run tab capability', async () => {
  const f = fixture([{ action: 'type', text: 'hello', candidates: [{ kind: 'css', value: '#field' }] }]);
  f.state.read = async () => {
    f.tabs[0] = { ...f.tabs[0] };
    return { count: 1, recorded: true, editable: true, reference: '7', value: 'hello' };
  };
  await validateNative(f.deps);
  assert.equal((await f.done).status, 'failed');
  assert.equal(f.calls.filter((c) => c[0] === 'click').length, 0);
});
it('stop while compositor geometry settles prevents all native pointer input', async () => {
  const f = fixture([{ action: 'click', candidates: [{ kind: 'css', value: '#field' }] }]);
  let release!: () => void;
  f.state.paint = () =>
    new Promise<void>((resolve) => {
      release = resolve;
    });
  const run = await validateNative(f.deps);
  await turn();
  assert.equal(typeof release, 'function');
  run.control('stop');
  release();
  assert.equal((await f.done).status, 'outcome-unknown');
  assert.equal(f.calls.filter((call) => ['move', 'input'].includes(String(call[0]))).length, 0);
});
it('never presses a button when native movement has not reached the exact node', async (t) => {
  const f = fixture([{ action: 'click', candidates: [{ kind: 'css', value: '#field' }] }]);
  t.mock.timers.enable({ apis: ['Date'], now: 1000 });
  f.state.hover = async () => {
    t.mock.timers.tick(10000);
    return false;
  };
  await validateNative(f.deps);
  assert.equal((await f.done).status, 'outcome-unknown');
  assert.equal(f.calls.filter((call) => call[0] === 'input').length, 0);
});
it('resume after human takeover refuses admission without reclaiming control', async () => {
  const f = fixture([
    { action: 'checkpoint' },
    { action: 'type', text: 'hello', candidates: [{ kind: 'css', value: '#field' }] },
  ]);
  const run = await validateNative(f.deps);
  await turn();
  f.state.human = true;
  run.control('resume');
  assert.equal((await f.done).status, 'failed');
  assert.equal(f.calls.filter((c) => c[0] === 'click').length, 0);
});
it('stop immediately after start opens no tab and releases its local client', async () => {
  const f = fixture([{ action: 'checkpoint' }]);
  const run = await validateNative(f.deps);
  run.dispose();
  assert.equal((await f.done).status, 'stopped');
  assert.deepEqual(f.calls, [
    ['client', 1],
    ['client', -1],
  ]);
});
it('evidence requests report safe omission without collecting secret pixels', async () => {
  const f = fixture([{ action: 'assert_value', expected: 'hello', candidates: [{ kind: 'css', value: '#field' }] }], {
    evidence: true,
  });
  await validateNative(f.deps);
  assert.equal((await f.done).status, 'succeeded');
  assert.ok(
    f.events.some((e) => e.event?.kind === 'evidence' && String(e.event.message).includes('Screenshot omitted')),
  );
});

it('snapshots draft and options before scheduling so callers cannot append unsupported actions', async () => {
  const f = fixture([{ action: 'type', text: '{{value}}', candidates: [{ kind: 'css', value: '#field' }] }], {
    vars: { value: 'original' },
  });
  const run = await validateNative(f.deps);
  f.deps.draft.steps[0].action = 'upload_file';
  f.deps.draft.steps.push(normalizeDraft({ steps: [{ action: 'upload_file', file: 'secret' }] }).steps[0]);
  f.deps.options.vars!.value = 'changed';
  assert.equal((await f.done).status, 'succeeded');
  assert.equal(
    f.calls
      .filter((c) => c[0] === 'type')
      .map((c) => c[2])
      .join(''),
    'original',
  );
  run.dispose();
});
