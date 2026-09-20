/**
 * Unit tests for Acquisition: one acquire() that holds a control-plane slot,
 * connects, fails over to the next provider on error, queues when everything
 * is busy, and hands back a release that works once.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-acquisition-');
const { ProviderPool } = await import('../../../../src/modules/gateway/routing.ts');
const { Acquisition } = await import('../../../../src/modules/gateway/acquisition.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { HttpError } = await import('../../../../src/platform/errors.ts');
const { CAPACITY_RETRY_MS } = await import('../../../../src/modules/gateway/constants.ts');
const { stubControl } = await import('../../support/gateway.ts');
const { advance } = await import('../../support/http.ts');

/** A pool with these providers for owner 'a', in priority order. */
function poolOf(...names: string[]) {
  const pool = new ProviderPool();
  names.forEach((name, i) => pool.register({ name, owner: 'a', wsUrl: `ws://${name}`, priority: i }));
  return pool;
}

let control$: ReturnType<typeof stubControl>;
beforeEach(() => {
  control$ = stubControl();
  mock.method(console, 'error', () => {});
});
afterEach(() => {
  mock.timers.reset();
  mock.restoreAll();
});

describe('Acquisition', () => {
  it('connects through the preferred provider and counts the session', async () => {
    const pool = poolOf('first', 'second');
    const got = await new Acquisition(pool, { owner: 'a', connect: async (p) => `on ${p.name}` }).run();
    assert.equal(got.session, 'on first');
    assert.deepEqual([got.provider.active, got.provider.totalSessions, got.provider.failures], [1, 1, 0]);
    assert.deepEqual(control$.of('holdProvider')[0].args, ['a', 'first', 10]);
  });

  it('frees the slot and the hold once, however often release is called', async () => {
    const pool = poolOf('only');
    const got = await new Acquisition(pool, { owner: 'a', connect: async () => 's' }).run();
    await got.release();
    await got.release();
    assert.equal(got.provider.active, 0);
    assert.deepEqual(
      control$.of('releaseProvider').map((c) => c.args[0]),
      ['hold-1'],
    );
  });

  it('fails over to the next provider, cooling down the one that failed', async () => {
    const pool = poolOf('broken', 'working');
    const connect = async (p) => {
      if (p.name === 'broken') throw new Error('refused');
      return 'ok';
    };
    const got = await new Acquisition(pool, { owner: 'a', connect }).run();
    assert.equal(got.provider.name, 'working');
    const broken = pool.get('a', 'broken');
    assert.deepEqual([broken.active, broken.failures, broken.healthy], [0, 1, false]);
    assert.equal(control$.of('releaseProvider')[0].args[0], 'hold-1');
  });

  it('answers 502 naming the last error once every attempt has failed', async () => {
    const pool = poolOf('a1', 'a2');
    const connect = async (p) => Promise.reject(new Error(`${p.name} down`));
    await assert.rejects(new Acquisition(pool, { owner: 'a', connect, attempts: 2 }).run(), {
      status: 502,
      message: 'All providers failed. Last error: a2 down',
    });
  });

  it('answers 503 at once when there is no provider and no time to queue', async () => {
    await assert.rejects(new Acquisition(poolOf(), { owner: 'a', connect: async () => 's', queueMs: 0 }).run(), {
      status: 503,
      message: 'No browser provider available',
    });
  });

  it('answers 503 after running out of providers to fail over to', async () => {
    const pool = poolOf('only');
    const connect = async () => Promise.reject(new Error('down'));
    await assert.rejects(new Acquisition(pool, { owner: 'a', connect, queueMs: 0 }).run(), { status: 503 });
  });

  it('queues while every provider is busy and takes the first slot freed', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const pool = poolOf('only');
    pool.get('a', 'only').maxConcurrent = 1;
    pool.get('a', 'only').active = 1;
    const pending = new Acquisition(pool, { owner: 'a', connect: async () => 'late' }).run();
    await advance(0);
    pool.release(pool.get('a', 'only'));
    assert.equal((await pending).session, 'late');
  });

  it('retries after a short pause while the control plane has no capacity', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    let full = true;
    mock.method(control() as any, 'holdProvider', async () => {
      if (full) throw new HttpError(429, 'full', { code: 'provider_capacity' });
      return 'hold-x';
    });
    const pending = new Acquisition(poolOf('only'), { owner: 'a', connect: async () => 's' }).run();
    await advance(0);
    full = false;
    await advance(CAPACITY_RETRY_MS);
    assert.equal((await pending).holdId, 'hold-x');
  });

  it('answers 503 when control-plane capacity stays exhausted past the queue time', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    mock.method(control() as any, 'holdProvider', async () => {
      throw new HttpError(429, 'full', { code: 'provider_capacity' });
    });
    const pending = new Acquisition(poolOf('only'), { owner: 'a', connect: async () => 's', queueMs: 250 }).run();
    const settled = assert.rejects(pending, { status: 503, message: 'Provider capacity exhausted' });
    await advance(CAPACITY_RETRY_MS, 4);
    await settled;
  });

  it('passes any other hold error straight through', async () => {
    mock.method(control() as any, 'holdProvider', async () => Promise.reject(new Error('store offline')));
    await assert.rejects(
      new Acquisition(poolOf('only'), { owner: 'a', connect: async () => 's' }).run(),
      /store offline/,
    );
  });
});
