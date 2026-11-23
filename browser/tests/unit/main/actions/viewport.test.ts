/** Viewport commands preserve ownership, optional density and reset without accepting unsupported modes. */
import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { VIEWPORT_READ_MS } from '../../../../src/main/actions/constants.ts';
import { inLane } from '../../../../src/main/control/agent-tab.ts';
import { setViewport, releaseViewport, releaseAllViewports } from '../../../../src/main/actions/viewport.ts';
/** Native seams record mutations; world reads represent the acknowledged renderer metrics. */
function fixture() {
  const calls: any[] = [];
  let actual = { width: 1000, height: 800, dpr: 2, url: 'http://fixture.test' };
  const wc = Object.assign(new EventEmitter(), {
    isDestroyed: () => false,
    enableDeviceEmulation: (params: any) => {
      calls.push(params);
      actual = {
        ...actual,
        width: params.viewSize.width,
        height: params.viewSize.height,
        dpr: params.deviceScaleFactor || 2,
      };
    },
    disableDeviceEmulation: () => {
      calls.push('reset');
      actual = { ...actual, width: 1000, height: 800, dpr: 2 };
    },
  });
  const driver: any = {
    deps: { worldEval: async () => ({ ...actual }) },
  };
  return { calls, wc, driver, view: { webContents: wc } as any };
}
afterEach(() => releaseAllViewports());
test('375/768/1280 widths and retained density use the same native lease until reset', async () => {
  const f = fixture();
  await inLane('mcp-sizing', async () => {
    for (const width of [375, 768, 1280])
      await setViewport(f.driver, f.view, { width, height: 812, ...(width === 375 ? { deviceScaleFactor: 2 } : {}) });
    assert.equal(f.calls[2].deviceScaleFactor, 2);
    await setViewport(f.driver, f.view, { reset: true });
  });
  assert.equal(f.calls.at(-1), 'reset');
});
test('unsupported and invalid settings leave the prior viewport untouched', async () => {
  const f = fixture();
  for (const params of [
    { width: 375, height: 812, mobile: true },
    { width: 375 },
    { width: 0, height: 812 },
    { width: 4096, height: 4096, deviceScaleFactor: 4 },
    { reset: true, width: 375 },
    { width: 375, height: 812, surprise: true },
  ]) {
    await assert.rejects(inLane('mcp-invalid', () => setViewport(f.driver, f.view, params)));
  }
  assert.equal(f.calls.length, 0);
});
test('another lane cannot clear an existing lease and ending its owner restores defaults', async () => {
  const f = fixture();
  await inLane('mcp-owner', () => setViewport(f.driver, f.view, { width: 375, height: 812 }));
  await assert.rejects(
    inLane('mcp-other', () => setViewport(f.driver, f.view, { reset: true })),
    /another connection/,
  );
  releaseViewport('mcp-owner');
  assert.equal(f.calls.at(-1), 'reset');
});
test('closing one tab releases its lease without touching another lane', async () => {
  const a = fixture(),
    b = fixture();
  await inLane('mcp-a', () => setViewport(a.driver, a.view, { width: 375, height: 812 }));
  await inLane('mcp-b', () => setViewport(b.driver, b.view, { width: 768, height: 812 }));
  a.wc.emit('destroyed');
  releaseViewport('mcp-a');
  assert.equal(b.calls.length, 1);
  releaseViewport('mcp-b');
  assert.equal(b.calls.at(-1), 'reset');
});

test('ending a session while a renderer read is pending prevents a late resize', async () => {
  const f = fixture();
  let finish: (value: unknown) => void = () => {};
  f.driver.deps.worldEval = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  const pending = inLane('mcp-cancel', () => setViewport(f.driver, f.view, { width: 375, height: 812 }));
  releaseViewport('mcp-cancel');
  finish({ width: 1000, height: 800, dpr: 2 });
  await assert.rejects(pending, /session ended/);
  assert.equal(f.calls.length, 0);
});

test('repeated reset removes destruction callbacks instead of accumulating listeners', async () => {
  const f = fixture();
  await inLane('mcp-resets', async () => {
    for (let n = 0; n < 12; n++) {
      await setViewport(f.driver, f.view, { width: 375, height: 812 });
      await setViewport(f.driver, f.view, { reset: true });
      assert.equal(f.wc.listenerCount('destroyed'), 0);
    }
  });
});

/** Fail only the final acknowledgement, after the native emulation call already succeeded. */
function failConfirmation(f: ReturnType<typeof fixture>) {
  const read = f.driver.deps.worldEval;
  let reads = 0;
  f.driver.deps.worldEval = async () => {
    if (++reads === 2) throw Error('renderer disappeared');
    return read();
  };
}
test('failed initial measurement clears the lease and its unconfirmed density', async () => {
  const f = fixture();
  failConfirmation(f);
  await inLane('mcp-initial-failure', async () => {
    await assert.rejects(
      setViewport(f.driver, f.view, { width: 375, height: 812, deviceScaleFactor: 3 }),
      /previous settings restored/,
    );
    assert.equal(f.calls.at(-1), 'reset');
    await setViewport(f.driver, f.view, { width: 768, height: 812 });
    assert.equal(f.calls.at(-1).deviceScaleFactor, 0);
  });
});
test('failed replacement restores the previous size and density before another call', async () => {
  const f = fixture();
  await inLane('mcp-replacement', async () => {
    await setViewport(f.driver, f.view, { width: 320, height: 800, deviceScaleFactor: 2 });
    failConfirmation(f);
    await assert.rejects(
      setViewport(f.driver, f.view, { width: 375, height: 812, deviceScaleFactor: 3 }),
      /previous settings restored/,
    );
    assert.equal(f.calls.at(-1).viewSize.width, 320);
    assert.equal(f.calls.at(-1).deviceScaleFactor, 2);
    await setViewport(f.driver, f.view, { width: 768, height: 812 });
    assert.equal(f.calls.at(-1).deviceScaleFactor, 2);
  });
});
test('rollback failure preserves cleanup ownership and requires explicit reset', async () => {
  const f = fixture();
  failConfirmation(f);
  const clear = f.wc.disableDeviceEmulation;
  f.wc.disableDeviceEmulation = () => {
    throw Error('native clear failed');
  };
  await inLane('mcp-rollback-failure', async () => {
    await assert.rejects(
      setViewport(f.driver, f.view, { width: 375, height: 812, deviceScaleFactor: 3 }),
      /Restoring prior settings also failed/,
    );
    await assert.rejects(setViewport(f.driver, f.view, { width: 768, height: 812 }), /restoration is unconfirmed/);
    f.wc.disableDeviceEmulation = clear;
    await setViewport(f.driver, f.view, { reset: true });
    assert.equal(f.wc.listenerCount('destroyed'), 0);
  });
});

test('a suspended first read reaches a deadline and releases work without a late resize', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  let resume: (value: unknown) => void = () => {};
  let released = false;
  f.driver.deps.worldEval = () =>
    new Promise((resolve) => {
      resume = resolve;
    });
  const pending = inLane('mcp-timeout-before', () =>
    setViewport(f.driver, f.view, { width: 375, height: 812 }),
  ).finally(() => {
    released = true;
  });
  const rejected = assert.rejects(pending, /measurement timed out/);
  t.mock.timers.tick(VIEWPORT_READ_MS);
  await rejected;
  assert.equal(released, true);
  assert.equal(f.calls.length, 0);
  resume({ width: 1000, height: 800, dpr: 2 });
  await Promise.resolve();
  assert.equal(f.calls.length, 0);
});

test('a suspended confirmation rolls back its native change and ignores late acknowledgement', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture(),
    read = f.driver.deps.worldEval;
  let calls = 0,
    resume: (value: unknown) => void = () => {};
  f.driver.deps.worldEval = () =>
    ++calls === 1
      ? read()
      : new Promise((resolve) => {
          resume = resolve;
        });
  const pending = inLane('mcp-timeout-after', () => setViewport(f.driver, f.view, { width: 375, height: 812 }));
  const rejected = assert.rejects(pending, /previous settings restored/);
  while (calls < 2) await Promise.resolve();
  t.mock.timers.tick(VIEWPORT_READ_MS);
  await rejected;
  assert.equal(f.calls.at(-1), 'reset');
  const count = f.calls.length;
  resume({ width: 375, height: 812, dpr: 2 });
  await Promise.resolve();
  assert.equal(f.calls.length, count);
});
