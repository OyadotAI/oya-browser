/**
 * Unit tests for Proxy: defaults from its config, exponential cooldown after
 * failures, recovery on a good check, and a JSON view without credentials.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Proxy, newProxyId } from '../../../../src/modules/proxies/proxy.ts';
import { assignments } from '../../../../src/modules/proxies/store.ts';
import { COOLDOWN_MS, MAX_COOLDOWN_MS } from '../../../../src/modules/proxies/constants.ts';

describe('Proxy', () => {
  afterEach(() => {
    mock.timers.reset();
    assignments.clear();
  });

  it('makes ids of "px-" and random hex', () => {
    assert.match(newProxyId(), /^px-[0-9a-f]{12}$/);
    assert.notEqual(newProxyId(), newProxyId());
  });

  it('defaults to a shared, healthy residential proxy for one persona, labelled by its id', () => {
    const p = new Proxy({ sealed: 'x' });
    assert.deepEqual(
      [p.owner, p.kind, p.maxPersonas, p.healthy, p.geo, p.label],
      [null, 'residential', 1, true, null, p.id],
    );
  });

  it('keeps a datacenter kind, a positive persona limit and its last known health', () => {
    const p = new Proxy({
      kind: 'datacenter',
      maxPersonas: '3',
      healthy: false,
      exitIp: '1.2.3.4',
      lastCheckedAt: 't',
    });
    assert.deepEqual(
      [p.kind, p.maxPersonas, p.healthy, p.exitIp, p.lastCheckedAt],
      ['datacenter', 3, false, '1.2.3.4', 't'],
    );
    assert.equal(new Proxy({ kind: 'mobile', maxPersonas: -1 }).kind, 'residential');
    assert.equal(new Proxy({ maxPersonas: -1 }).maxPersonas, 1);
  });

  it('cools down after a failure, doubling per consecutive failure up to the cap', () => {
    mock.timers.enable({ apis: ['Date'], now: 1_000_000 });
    const p = new Proxy({});
    p.fail();
    assert.equal(p.cooldownUntil, 1_000_000 + COOLDOWN_MS);
    p.fail();
    assert.equal(p.cooldownUntil, 1_000_000 + COOLDOWN_MS * 2);
    for (let i = 0; i < 10; i++) p.fail();
    assert.equal(p.cooldownUntil, 1_000_000 + MAX_COOLDOWN_MS);
    assert.equal(p.healthy, false);
  });

  it('is unavailable until a check succeeds, even after the cooldown', () => {
    mock.timers.enable({ apis: ['Date'], now: 0 });
    const p = new Proxy({});
    p.fail();
    assert.equal(p.available, false);
    mock.timers.tick(COOLDOWN_MS);
    assert.equal(p.available, false);
    p.succeed('9.9.9.9');
    assert.deepEqual([p.available, p.failures, p.exitIp], [true, 0, '9.9.9.9']);
  });

  it('keeps the known exit IP when a check reports none, and notes the check time', () => {
    mock.timers.enable({ apis: ['Date'], now: Date.parse('2026-01-01T00:00:00.000Z') });
    const p = new Proxy({ exitIp: '1.1.1.1' });
    p.succeed(null);
    assert.equal(p.exitIp, '1.1.1.1');
    assert.equal(p.lastCheckedAt, '2026-01-01T00:00:00.000Z');
  });

  it('counts the personas assigned to it', () => {
    const p = new Proxy({ id: 'px-count' });
    assignments.set('p-1', 'px-count');
    assignments.set('p-2', 'px-count');
    assignments.set('p-3', 'px-other');
    assert.equal(p.assigned, 2);
  });

  it('describes itself without its sealed credentials', () => {
    mock.timers.enable({ apis: ['Date'], now: 0 });
    const p = new Proxy({ id: 'px-json', owner: 'o', label: 'L', geo: 'US', sealed: 'SECRET' });
    p.fail();
    assert.deepEqual(p.toJSON(), {
      id: 'px-json',
      label: 'L',
      kind: 'residential',
      geo: 'US',
      shared: false,
      healthy: false,
      available: false,
      exitIp: null,
      lastCheckedAt: null,
      assigned: 0,
      maxPersonas: 1,
      cooldownMsRemaining: COOLDOWN_MS,
    });
  });
});
