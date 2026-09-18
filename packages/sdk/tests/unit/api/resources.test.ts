/**
 * Unit tests for the small namespaces: `oya.playbooks`, `oya.proxies` and
 * `oya.config` (src/api/playbooks.ts, proxies.ts, config.ts).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { client, type Call } from '../support/fake-fetch.ts';

/** Every request answers 200 with `body`. */
const any = (body: unknown = {}) => client(new Proxy({}, { get: () => ({ body }) }));

/** The method, path and body of each call. */
const summary = (calls: Call[]) => calls.map((c) => [c.method, c.path, c.body]);

describe('oya.playbooks', () => {
  it('lists, removes and promotes, encoding names', async () => {
    const { oya, calls } = any({ playbooks: [{ name: 'a' }] });
    assert.deepEqual(await oya.playbooks.list(), [{ name: 'a' }]);
    assert.equal(await oya.playbooks.remove('a:draft'), undefined);
    await oya.playbooks.promote('my flow');
    assert.deepEqual(summary(calls).slice(1), [
      ['DELETE', '/api/playbooks/a%3Adraft', undefined],
      ['POST', '/api/playbooks/my%20flow/promote', {}],
    ]);
  });
});

describe('oya.proxies', () => {
  it('lists, creates, removes and checks', async () => {
    const { oya, calls } = any({ proxies: [{ id: 'x' }], results: [{ id: 'x', ok: true }] });
    assert.deepEqual(await oya.proxies.list(), [{ id: 'x' }]);
    await oya.proxies.create({ url: 'http://h:1', maxPersonas: 1 });
    await oya.proxies.remove('x');
    assert.deepEqual(await oya.proxies.check(), [{ id: 'x', ok: true }]);
    assert.deepEqual(summary(calls).slice(1), [
      ['POST', '/api/proxies', { url: 'http://h:1', maxPersonas: 1 }],
      ['DELETE', '/api/proxies/x', undefined],
      ['POST', '/api/proxies/check', {}],
    ]);
  });
});

describe('oya.config and usage', () => {
  it('gets and sets config, and reads usage', async () => {
    const { oya, calls } = any({ llm_provider: 'openai' });
    assert.deepEqual(await oya.config.get(), { llm_provider: 'openai' });
    await oya.config.set({ llm_provider: 'gemini' });
    await oya.usage();
    assert.deepEqual(summary(calls), [
      ['GET', '/api/config', undefined],
      ['POST', '/api/config', { llm_provider: 'gemini' }],
      ['GET', '/api/usage', undefined],
    ]);
  });
});
