/**
 * Unit tests for the mirror run (src/main/mirror/mirror.ts): importing the logins
 * of the person's real browser, and telling them plainly how it went. The
 * capture, the storage seeding and the socket are fakes; timers are mocked.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Mirror } from '../../../../src/main/mirror/mirror.ts';
import { MIRROR_ANSWER_TIMEOUT_MS } from '../../../../src/main/mirror/constants.ts';
import { flush } from '../../support/fakes.cjs';

/** What a capture of Chrome with two profiles returns. */
const CAPTURED = {
  source: 'chrome',
  name: 'Chrome',
  userDataDir: '/chrome',
  device: { navigator: { platform: 'MacIntel' } },
  profiles: [
    { profile: 'Default', name: 'Work', lastUsed: true, cookies: [{ domain: '.a.test' }, { domain: 'b.test' }] },
    { profile: 'Profile 1', name: 'Home', lastUsed: false, cookies: [{ domain: 'a.test' }] },
  ],
};

/** A mirror over a fake context; `capture` answers the capture, `up` is whether the socket is ready. */
function mirrorWith({ capture = async (): Promise<any> => CAPTURED, up = true }: any = {}) {
  const ctx: any = {
    statuses: [],
    sent: [],
    reconnects: 0,
    config: { values: { apiKey: 'k' }, merge: (v: object) => Object.assign(ctx.config.values, v), save: () => {} },
    electron: {},
  };
  ctx.shell = { send: (_channel: string, status: object) => ctx.statuses.push(status) };
  ctx.socket = {
    ready: up,
    isOpen: () => up,
    send: (message: object) => ctx.sent.push(message),
    disconnect: () => ctx.reconnects++,
    connect: () => {},
  };
  const seeded: unknown[][] = [];
  const mirror = new Mirror(ctx, { capture, seed: (...args) => void seeded.push(args) });
  return { ctx, mirror, seeded };
}

describe('Mirror', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    mock.method(console, 'log', () => {});
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('imports the chosen browser and says how many profiles and cookies came over', async () => {
    const capture = mock.fn(async () => CAPTURED);
    const { ctx, mirror, seeded } = mirrorWith({ capture });
    await mirror.reimport('chrome');
    assert.equal(capture.mock.calls[0].arguments[1], 'chrome');
    assert.deepEqual(ctx.statuses[0], { started: true });
    assert.equal(ctx.sent[0].type, 'mirror_persona');
    assert.equal(ctx.sent[0].profiles[0].name, 'Chrome · Work', 'named without an em-dash');
    mirror.onOk({ personaIds: ['p-1', 'p-2'], defaultPersonaId: 'p-1' });
    const { at, ...done } = ctx.statuses.at(-1);
    assert.deepEqual(done, { done: true, source: 'Chrome', profiles: 2, cookies: 3, sites: 2 });
    assert.equal(typeof at, 'number');
    assert.deepEqual(seeded[0].slice(1), ['p-1', '/chrome', 'Default']);
    assert.equal(ctx.config.values.persona, 'p-1');
    assert.equal(ctx.reconnects, 1);
  });

  it('remembers each finished import for the account page, the newest first, the last five only', async () => {
    const { ctx, mirror } = mirrorWith();
    ctx.config.values.imports = [1, 2, 3, 4, 5].map((n) => ({ source: `Old ${n}` }));
    await mirror.reimport('chrome');
    mirror.onOk({ personaIds: ['p-1'], defaultPersonaId: 'p-1' });
    const { imports } = ctx.config.values;
    assert.deepEqual(
      imports.map((record) => record.source),
      ['Chrome', 'Old 1', 'Old 2', 'Old 3', 'Old 4'],
    );
    assert.deepEqual([imports[0].sites, imports[0].cookies, imports[0].at], [2, 3, ctx.statuses.at(-1).at]);
  });

  it('remembers nothing of an import the server refused', async () => {
    const { ctx, mirror } = mirrorWith();
    await mirror.reimport('chrome');
    mirror.onFailed({ error: 'no' });
    assert.equal(ctx.config.values.imports, undefined);
  });

  it('imports on the first sign-in after a pairing that asked for it, and only then', async () => {
    const capture = mock.fn(async () => CAPTURED);
    const { ctx, mirror } = mirrorWith({ capture });
    ctx.config.values.mirroredFrom = 'chrome';
    await mirror.maybeRun();
    assert.equal(capture.mock.callCount(), 0, 'already imported and not asked again');
    mirror.importOnConnect = true;
    await mirror.maybeRun();
    await mirror.maybeRun();
    assert.equal(capture.mock.callCount(), 1);
  });

  it('asks the person to connect first instead of importing into nothing', async () => {
    const capture = mock.fn(async () => CAPTURED);
    const { ctx, mirror } = mirrorWith({ capture, up: false });
    await mirror.reimport('chrome');
    assert.equal(capture.mock.callCount(), 0);
    assert.match(ctx.statuses.at(-1).error, /connect/i);
  });

  it('says why a capture failed, and lets it be tried again', async () => {
    const { ctx, mirror } = mirrorWith({
      capture: async () => Promise.reject(new Error('Chrome never opened its port')),
    });
    await mirror.reimport('chrome');
    assert.deepEqual(ctx.statuses.at(-1), { done: true, error: 'Chrome never opened its port' });
    assert.notEqual(ctx.config.values.mirroredFrom, 'none', 'a failure is not remembered as done');
  });

  it('says so when there is no browser, or no profile, to import', async () => {
    const { ctx, mirror } = mirrorWith({ capture: async () => null });
    await mirror.reimport();
    assert.deepEqual(ctx.statuses.at(-1), { done: true, empty: true });
  });

  it('gives up on a server that never answers, and ignores its answer if it comes late', async () => {
    const { ctx, mirror } = mirrorWith();
    await mirror.reimport('chrome');
    mock.timers.tick(MIRROR_ANSWER_TIMEOUT_MS);
    assert.match(ctx.statuses.at(-1).error, /did not answer/i);
    mirror.onOk({ personaIds: ['p-1'], defaultPersonaId: 'p-1' });
    assert.equal(ctx.reconnects, 0);
  });

  it("reports the server's refusal", async () => {
    const { ctx, mirror } = mirrorWith();
    await mirror.reimport('chrome');
    mirror.onFailed({ error: 'At most 8 profiles per import' });
    assert.deepEqual(ctx.statuses.at(-1), { done: true, error: 'At most 8 profiles per import' });
    await flush();
  });
});
