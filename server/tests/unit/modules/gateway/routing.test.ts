/**
 * Unit tests for ProviderPool: registering providers per owner, what each
 * owner can see and pick, queueing for a slot, and handing a released slot
 * to the right waiter.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-routing-');
const { ProviderPool, STRATEGIES, pool } = await import('../../../../src/modules/gateway/routing.ts');
const { stubControl } = await import('../../support/gateway.ts');

/** A cdp provider config. */
const cfg = (name: string, extra: object = {}) => ({ name, wsUrl: `ws://${name}:9222`, ...extra });

describe('ProviderPool', () => {
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
    delete process.env.OYA_ROUTING_STRATEGY;
  });

  it("uses the host's strategy when it is a known one, else priority", () => {
    process.env.OYA_ROUTING_STRATEGY = 'latency';
    assert.equal(new ProviderPool().strategy, 'latency');
    process.env.OYA_ROUTING_STRATEGY = 'bogus';
    assert.equal(new ProviderPool().strategy, 'priority');
  });

  it('is seeded from OYA_PROVIDERS, and survives it being malformed', () => {
    const p = new ProviderPool().loadFromEnv({ OYA_PROVIDERS: JSON.stringify([cfg('shared')]) });
    assert.equal(p.get(null, 'shared').owner, null);
    const error = mock.method(console, 'error', () => {});
    assert.equal(new ProviderPool().loadFromEnv({ OYA_PROVIDERS: '[nope' }).providers.size, 0);
    assert.match(error.mock.calls[0].arguments[0], /OYA_PROVIDERS is not valid JSON/);
    assert.equal(new ProviderPool().loadFromEnv({}).providers.size, 0);
  });

  it('exports a process-wide pool', () => {
    assert.ok(pool instanceof ProviderPool);
    assert.equal(STRATEGIES[0], 'priority');
  });

  it('keeps each owner’s strategy, refusing an unknown one with a 400', () => {
    const p = new ProviderPool();
    p.setStrategy('a', 'round-robin');
    assert.deepEqual([p.strategyFor('a'), p.strategyFor('b')], ['round-robin', 'priority']);
    assert.throws(() => p.setStrategy('a', 'fastest'), { status: 400, message: 'Unknown strategy' });
  });

  it('namespaces names by owner, so two keys can both have a "chrome"', () => {
    const p = new ProviderPool();
    p.register(cfg('chrome', { owner: 'a' }));
    p.register(cfg('chrome', { owner: 'b' }));
    assert.equal(p.providers.size, 2);
    assert.equal(p.key(null, 'x'), '@shared::x');
  });

  it('updates a provider in place, keeping its live counters', () => {
    const p = new ProviderPool();
    const first = p.register(cfg('c', { owner: 'a' }));
    first.totalSessions = 5;
    const again = p.register(cfg('c', { owner: 'a', priority: 1 }));
    assert.equal(again, first);
    assert.deepEqual([again.priority, again.totalSessions], [1, 5]);
  });

  it('refuses to move an active provider to another URL, with a 409', () => {
    const p = new ProviderPool();
    p.register(cfg('c', { owner: 'a' })).active = 1;
    assert.throws(() => p.register({ name: 'c', owner: 'a', wsUrl: 'ws://elsewhere' }), { status: 409 });
    assert.doesNotThrow(() => p.register(cfg('c', { owner: 'a', weight: 2 })));
  });

  it('removes a provider, but not while it has sessions', () => {
    const p = new ProviderPool();
    p.register(cfg('c', { owner: 'a' })).active = 1;
    assert.throws(() => p.remove('a', 'c'), {
      status: 409,
      message: 'End active sessions before removing this provider.',
    });
    p.get('a', 'c').active = 0;
    assert.equal(p.remove('a', 'c'), true);
    assert.equal(p.remove('a', 'c'), false);
  });

  it('shows an owner its own and shared providers, and returns only its own configs', () => {
    const p = new ProviderPool();
    p.register(cfg('mine', { owner: 'a' }));
    p.register(cfg('theirs', { owner: 'b' }));
    p.register(cfg('shared'));
    assert.deepEqual(
      p.list('a').map((x) => x.name),
      ['mine', 'shared'],
    );
    assert.deepEqual(
      p.configs('a').map((x) => x.name),
      ['mine'],
    );
  });

  it('picks only available providers, skipping excluded ones', () => {
    const p = new ProviderPool();
    p.register(cfg('busy', { owner: 'a', maxConcurrent: 1, priority: 1 })).active = 1;
    p.register(cfg('free', { owner: 'a', priority: 5 }));
    p.register(cfg('last', { owner: 'a', priority: 9 }));
    assert.equal(p.pick('a').name, 'free');
    assert.equal(p.pick('a', 'priority', new Set([p.key('a', 'free')])).name, 'last');
    assert.equal(p.pick('b'), null);
  });

  it('answers a zero-length wait at once with null', async () => {
    assert.equal(await new ProviderPool().waitForSlot(0, 'a', undefined, new Set()), null);
  });

  it('hands a released slot to the longest waiter', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const p = new ProviderPool();
    const prov = p.register(cfg('c', { owner: 'a', maxConcurrent: 1 }));
    prov.active = 1;
    const waiting = p.waitForSlot(1000, 'a', undefined, new Set());
    assert.equal(p.queueDepth, 1);
    p.release(prov);
    assert.equal(await waiting, prov);
    assert.equal(prov.active, 0);
  });

  it('picks another provider for a waiter that already failed on the released one', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const p = new ProviderPool();
    const bad = p.register(cfg('bad', { owner: 'a' }));
    const good = p.register(cfg('good', { owner: 'a' }));
    const waiting = p.waitForSlot(1000, 'a', 'priority', new Set([p.key('a', 'bad')]));
    bad.active = 1;
    p.release(bad);
    assert.equal(await waiting, good);
  });

  it('never lets active go below zero', () => {
    const p = new ProviderPool();
    const prov = p.register(cfg('c'));
    p.release(prov);
    assert.equal(prov.active, 0);
  });

  it("reports the owner's strategy, queue, capacity, load and health", () => {
    const p = new ProviderPool();
    p.register(cfg('a1', { owner: 'a', maxConcurrent: 3 })).active = 1;
    p.register(cfg('a2', { owner: 'a', maxConcurrent: 2, enabled: false }));
    const s = p.stats('a');
    assert.deepEqual(
      [s.strategy, s.queueDepth, s.capacity, s.active, s.healthy, s.providers.length],
      ['priority', 0, 5, 1, 1, 2],
    );
  });

  it('acquires a connected session through a provider', async () => {
    const { of } = stubControl();
    const p = new ProviderPool();
    p.register(cfg('c', { owner: 'a' }));
    const got = await p.acquire({ owner: 'a', connect: async () => 'session' });
    assert.deepEqual([got.session, got.provider.name, got.holdId], ['session', 'c', 'hold-1']);
    assert.equal(of('holdProvider').length, 1);
  });
});
