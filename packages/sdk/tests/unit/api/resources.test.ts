/**
 * Unit tests for the small namespaces: `oya.playbooks`, `oya.proxies` and
 * `oya.config` (src/api/playbooks.ts, proxies.ts, config.ts).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { client, type Call } from '../support/fake-fetch.ts';
import type { Oya, OyaError } from '../../../dist/index.js';

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

describe('ids in a path', () => {
  /** Every call that puts an id into a path, as `(oya, id) => call`. */
  const byId: Record<string, (oya: Oya, id: string) => Promise<unknown>> = {
    'personas.get': (oya, id) => oya.personas.get(id),
    'personas.update': (oya, id) => oya.personas.update(id, {}),
    'personas.clone': (oya, id) => oya.personas.clone(id),
    'personas.remove': (oya, id) => oya.personas.remove(id),
    'personas.cookies': (oya, id) => oya.personas.cookies(id),
    'personas.credentials': (oya, id) => oya.personas.credentials(id),
    'browser.get': (oya, id) => oya.browser.get(id),
    'control.session': (oya, id) => oya.control.session(id),
    'control.cancel': (oya, id) => oya.control.cancel(id),
    'control.recover': (oya, id) => oya.control.recover(id),
    'control.removeMember': (oya, id) => oya.control.removeMember(id),
    'proxies.remove': (oya, id) => oya.proxies.remove(id),
    'playbooks.remove': (oya, id) => oya.playbooks.remove(id),
  };

  it('refuses an id that is empty, not a string, or would change the path, before any request', async () => {
    for (const [name, call] of Object.entries(byId)) {
      for (const id of ['', '.', '..', '../config', 'a/b', 12, undefined]) {
        const { oya, calls } = any();
        const err = (await call(oya, id as string).catch((e) => e)) as OyaError & { body: { code: string } };
        assert.equal(err?.status, 400, `${name}(${JSON.stringify(id)})`);
        assert.equal(err.body.code, 'invalid_request', name);
        assert.equal(calls.length, 0, `${name}(${JSON.stringify(id)}) sent a request`);
      }
    }
  });

  it('names the id and what it needed', async () => {
    const { oya } = any();
    await assert.rejects(oya.personas.get('../config'), {
      message: 'id must be a single path segment (no "/" and not "." or ".."), not "../config"',
    });
    await assert.rejects(oya.personas.get(''), { message: 'id must be a non-empty string, not ""' });
  });

  it('encodes an ordinary id that needs it, and sends it', async () => {
    const { oya, calls } = any();
    await oya.personas.get('p 1?x');
    assert.equal(calls[0].path, '/api/personas/p%201%3Fx');
  });
});
