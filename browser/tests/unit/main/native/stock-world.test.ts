/**
 * Unit tests for isolated-world runs through the tab preload: the stock fallback
 * after a missing acknowledgement, and a preload-only run that never falls back.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { runInWorld, useStockWorld } from '../../../../src/main/native/stock-world.ts';
import { STOCK_WORLD_ACK_MS } from '../../../../src/main/native/constants.ts';
import { STOCK_PERSONA } from '../../../../src/shared/stock-persona.ts';

/** The preload's channels, as main listens on them. */
const ipc = {
  handlers: new Map(),
  on(name, fn) {
    this.handlers.set(name, fn);
  },
  handle(name, fn) {
    this.handlers.set(name, fn);
  },
};
useStockWorld(ipc);

/** A tab whose preload receives runs in `sent`, and whose stock API counts its calls. */
function tab() {
  const contents = new EventEmitter();
  contents.sent = [];
  contents.stockCalls = 0;
  contents.mainFrame = { send: (_channel, run) => contents.sent.push(run) };
  contents.executeJavaScriptInIsolatedWorld = async () => (contents.stockCalls++, 'stock');
  return contents;
}

/** The preload answering run `id` of `contents` with `value`. */
function answer(contents, id, value) {
  ipc.handlers.get(STOCK_PERSONA.RESULT)({ sender: contents, senderFrame: contents.mainFrame }, { id, value });
}

describe('runInWorld', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('falls back to the stock API when the preload does not acknowledge in time', async () => {
    const contents = tab();
    const run = runInWorld(contents, 1, '1');
    mock.timers.tick(STOCK_WORLD_ACK_MS);
    assert.equal(await run, 'stock');
  });

  it('never falls back when asked for the preload only, so a slow load is not mistaken for silence', async () => {
    const contents = tab();
    const run = runInWorld(contents, 1, '1', true);
    mock.timers.tick(STOCK_WORLD_ACK_MS * 5);
    assert.equal(contents.stockCalls, 0);
    answer(contents, contents.sent[0].id, 1);
    assert.equal(await run, 1);
  });

  it('gives up a preload-only run when its renderer dies', async () => {
    const contents = tab();
    const run = runInWorld(contents, 1, '1', true);
    contents.emit('render-process-gone');
    await assert.rejects(run, /disposed/);
  });
});
