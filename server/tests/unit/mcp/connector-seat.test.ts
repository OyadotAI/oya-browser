/**
 * Unit tests for a chat connector's one browser: started on first use through
 * the public API, held for every later call, rejoined after a deploy or a
 * dropped socket, replaced once it ends, and never started twice, started by a
 * tool that only reads, or left half-started.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { acquire, releaseBrowser, resetSeats } from '../../../src/mcp/connector-seat.ts';
import { rememberSeat } from '../../../src/mcp/seat-store.ts';
import { destroyMcpServer } from '../../../src/mcp/pool-state.ts';
import {
  CONNECTOR_START_REPLY_MS,
  REJOIN_WAIT_MS,
  START_POLL_MS,
  START_RETRY_AFTER_MS,
  START_WAIT_MS,
} from '../../../src/mcp/constants.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';
import { stubControl } from '../support/browsers.ts';

const KEY = 'k-seat';
let n = 0;
/** A connector no earlier test has used: what a seat drives is remembered in the store. */
const seat = () => ({
  id: `cred-${n++}`,
  apiKey: KEY,
  self: { origin: 'http://127.0.0.1:1', authorization: 'Bearer t' },
});

/** One call the public API received. */
type Call = { path: string; body: any; key: string };

/** The public API: each start answers the next of `answers` (an id, or a Response or Error); returns the calls made. */
function api(answers: (string | Response | Error)[]) {
  const calls: Call[] = [];
  mock.method(globalThis, 'fetch', async (url, init) => {
    const path = new URL(String(url)).pathname;
    calls.push({ path, body: JSON.parse(init.body), key: init.headers['Idempotency-Key'] });
    if (path === '/api/browsers/stop') return Response.json({ results: [{ ok: true }] });
    const next = answers.shift();
    if (next instanceof Error) throw next;
    return next instanceof Response ? next : Response.json({ id: next, provider: 'oya-cloud' }, { status: 201 });
  });
  return calls;
}

/** The starts among `calls`. */
const starts = (calls: Call[]) => calls.filter((c) => c.path === '/api/browsers/start');

/** Lets pending work settle, then moves the fake clock on by `ms`, one poll at a time. */
async function advance(ms: number) {
  for (let t = 0; t < ms; t += START_POLL_MS) {
    await settle();
    mock.timers.tick(START_POLL_MS);
  }
  await settle();
}

/** Lets promises and store reads that are already under way finish. */
async function settle() {
  for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
}

/** The browser an acquisition found, or undefined. */
const idOf = (found) => ('id' in found ? found.id : undefined);

const STILL_STARTING = { error: 'The browser is still starting. Repeat this step shortly.' };

describe('a connector’s browser', () => {
  beforeEach(() => {
    stubControl();
    mock.method(console, 'error', () => {});
  });
  afterEach(() => {
    mock.restoreAll();
    mock.timers.reset();
    ['s-1', 's-2'].forEach(disconnectBrowser);
    resetSeats();
  });

  it('starts one in Oya Cloud on first use and says so', async () => {
    connectBrowser('s-1', KEY, true);
    const calls = api(['s-1']);
    assert.deepEqual(await acquire(seat()), { id: 's-1', note: 'Started a browser for this chat.' });
    assert.deepEqual(
      calls.map((c) => [c.path, c.body]),
      [['/api/browsers/start', { provider: 'oya-cloud' }]],
    );
  });

  it('sends every later call to the same browser without starting another', async () => {
    connectBrowser('s-1', KEY, true);
    const calls = api(['s-1', 's-2']);
    const mine = seat();
    await acquire(mine);
    for (let i = 0; i < 10; i++) assert.deepEqual(await acquire(mine), { id: 's-1' });
    assert.equal(starts(calls).length, 1);
  });

  it('shares one start between first calls that arrive together', async () => {
    connectBrowser('s-1', KEY, true);
    const calls = api(['s-1', 's-2']);
    const mine = seat();
    const both = await Promise.all([acquire(mine), acquire(mine)]);
    assert.deepEqual(both.map(idOf), ['s-1', 's-1']);
    assert.equal(starts(calls).length, 1);
  });

  it('gives two connectors in one project a browser each', async () => {
    connectBrowser('s-1', KEY, true);
    connectBrowser('s-2', KEY, true);
    api(['s-1', 's-2']);
    assert.deepEqual([idOf(await acquire(seat())), idOf(await acquire(seat()))], ['s-1', 's-2']);
  });

  it('starts nothing for a tool that only reads: it says how to open a browser instead', async () => {
    const calls = api(['s-1']);
    assert.deepEqual(await acquire(seat(), false), {
      error: 'No browser is open. Call open_tab with a URL to start one.',
    });
    assert.equal(calls.length, 0);
  });

  it('lets a tool that only reads use a browser that is already running', async () => {
    connectBrowser('s-1', KEY, true);
    api(['s-1']);
    const mine = seat();
    await acquire(mine);
    assert.deepEqual(await acquire(mine, false), { id: 's-1' });
  });

  it('starts a new one once its browser has ended, and says pages are gone', async () => {
    connectBrowser('s-1', KEY, true);
    connectBrowser('s-2', KEY, true);
    api(['s-1', 's-2']);
    const mine = seat();
    await acquire(mine);
    disconnectBrowser('s-1');
    destroyMcpServer('s-1');
    const next = await acquire(mine);
    assert.equal(idOf(next), 's-2');
    assert.match('note' in next ? next.note! : '', /^The previous browser ended, so a new one started\./);
  });

  it('notices a browser that ended without a disconnect event', async () => {
    connectBrowser('s-1', KEY, true);
    connectBrowser('s-2', KEY, true);
    api(['s-1', 's-2']);
    const mine = seat();
    await acquire(mine);
    disconnectBrowser('s-1');
    assert.equal(idOf(await acquire(mine)), 's-2');
  });

  it('finds its browser again after a deploy, from the store, starting nothing', async () => {
    connectBrowser('s-1', KEY, true);
    const mine = seat();
    await rememberSeat(mine.id, 's-1');
    const calls = api(['s-2']);
    assert.deepEqual(await acquire(mine), { id: 's-1' });
    assert.deepEqual(await acquire(mine, false), { id: 's-1' });
    assert.equal(calls.length, 0);
  });

  it('waits for its browser to redial after a dropped socket instead of starting a second', async () => {
    stubControl({ findSession: async () => ({ state: 'ready' }) });
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    connectBrowser('s-1', KEY, true);
    const calls = api(['s-1', 's-2']);
    const mine = seat();
    await acquire(mine);
    disconnectBrowser('s-1');
    releaseBrowser('s-1');
    const during = acquire(mine);
    await advance(START_POLL_MS);
    connectBrowser('s-1', KEY, true);
    await advance(START_POLL_MS);
    assert.deepEqual(await during, { id: 's-1' }, 'nothing ended, so nothing is said');
    assert.equal(starts(calls).length, 1);
  });

  it('starts another once its browser has not redialled in time', async () => {
    stubControl({ findSession: async () => ({ state: 'ready' }) });
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    connectBrowser('s-2', KEY, true);
    const mine = seat();
    await rememberSeat(mine.id, 's-1');
    api(['s-2']);
    const waiting = acquire(mine);
    await advance(REJOIN_WAIT_MS + START_POLL_MS);
    const next = await waiting;
    assert.equal(idOf(next), 's-2');
    assert.match('note' in next ? next.note! : '', /previous browser ended/);
  });

  it('never takes a browser that belongs to another project', async () => {
    connectBrowser('s-1', 'k-someone-else', true);
    connectBrowser('s-2', KEY, true);
    const mine = seat();
    await rememberSeat(mine.id, 's-1');
    api(['s-2']);
    assert.equal(idOf(await acquire(mine)), 's-2');
  });

  it('tells the model why when the start is refused, and logs it', async () => {
    api([Response.json({ error: 'Browser quota reached (5)' }, { status: 429 })]);
    assert.deepEqual(await acquire(seat()), {
      error: 'Could not start a browser: Browser quota reached (5). Tell the person this reason.',
    });
    assert.equal((console.error as any).mock.calls.length, 1);
  });

  it('ends the reason with one full stop, whatever the API’s message ends with', async () => {
    api([Response.json({ error: 'Cloud browsers need OYA_PUBLIC_WS_URL.' }, { status: 503 })]);
    assert.deepEqual(await acquire(seat()), {
      error: 'Could not start a browser: Cloud browsers need OYA_PUBLIC_WS_URL. Tell the person this reason.',
    });
  });

  it('does not try a refused start again at once: repeats would spend the project’s start allowance', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    connectBrowser('s-1', KEY, true);
    const calls = api([Response.json({ error: 'Rate limited' }, { status: 429 }), 's-1']);
    const mine = seat();
    const first = await acquire(mine);
    for (let i = 0; i < 20; i++) assert.deepEqual(await acquire(mine), first);
    assert.equal(starts(calls).length, 1);
    mock.timers.tick(START_RETRY_AFTER_MS);
    assert.equal(idOf(await acquire(mine)), 's-1');
    assert.equal(starts(calls).length, 2);
  });

  it('repeats a start whose reply was lost under the same idempotency key, so it is one start', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    connectBrowser('s-1', KEY, true);
    const calls = api([new Error('The operation was aborted due to timeout'), 's-1', 's-2']);
    const mine = seat();
    assert.match(JSON.stringify(await acquire(mine)), /Could not start a browser/);
    mock.timers.tick(START_RETRY_AFTER_MS);
    assert.equal(idOf(await acquire(mine)), 's-1');
    assert.equal(calls[1].key, calls[0].key);
    disconnectBrowser('s-1');
    connectBrowser('s-2', KEY, true);
    await acquire(mine);
    assert.notEqual(calls[2].key, calls[0].key, 'a settled start does not reuse its key');
  });

  it('answers "still starting" when a boot outlasts the call, and a repeat joins the same start', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    const calls = api(['s-1', 's-2']);
    const mine = seat();
    const first = acquire(mine);
    await advance(CONNECTOR_START_REPLY_MS);
    assert.deepEqual(await first, STILL_STARTING);
    const repeat = acquire(mine, false);
    await settle();
    connectBrowser('s-1', KEY, true);
    await advance(START_POLL_MS);
    assert.deepEqual(await repeat, { id: 's-1', note: 'Started a browser for this chat.' });
    assert.equal(starts(calls).length, 1);
  });

  it('stops a browser that never dials in, so it holds no session', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    const calls = api(['s-1']);
    const first = acquire(seat());
    await advance(START_WAIT_MS + START_POLL_MS);
    assert.deepEqual(await first, STILL_STARTING);
    assert.deepEqual([calls.at(-1)!.path, calls.at(-1)!.body], ['/api/browsers/stop', { ids: ['s-1'] }]);
  });

  it('refuses a started browser that is not a verified cloud browser of this project, and stops it', async () => {
    connectBrowser('s-1', 'k-someone-else', true);
    const calls = api(['s-1']);
    assert.match(JSON.stringify(await acquire(seat())), /did not come up in time/);
    assert.equal(calls.at(-1)!.path, '/api/browsers/stop');
  });
});
