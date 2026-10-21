/**
 * Unit tests for src/main/connection/server-api.ts: a call made with a time limit
 * gives up on a server that never answers, and an error keeps the server's
 * status and words.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ServerApi } from '../../../../src/main/connection/server-api.ts';

/** A ServerApi over a fake key, server and socket. */
const api = () =>
  new ServerApi({
    config: { values: { serverUrl: 'wss://oya.test/ws', apiKey: 'k1' } },
    socket: { ready: true, browserId: 'b1' },
  } as any);

describe('ServerApi.send', () => {
  let calls: any[];
  beforeEach(() => {
    calls = [];
    mock.method(globalThis, 'fetch', async (url: any, init: any) => {
      calls.push({ url, init });
      return { ok: true, status: 200, json: async () => ({ ok: true }) };
    });
  });

  it('sends the method and body to the project API with the key', async () => {
    assert.deepEqual(await api().send('PATCH', 'playbooks/a', { name: 'b' }), { ok: true });
    assert.equal(calls[0].url, 'https://oya.test/api/playbooks/a');
    assert.equal(calls[0].init.method, 'PATCH');
    assert.equal(calls[0].init.headers.Authorization, 'Bearer k1');
    assert.equal(calls[0].init.body, '{"name":"b"}');
  });

  it('gives up after timeoutMs, and only when one is given', async () => {
    await api().send('PATCH', 'playbooks/a', {}, 1000);
    assert.ok(calls[0].init.signal instanceof AbortSignal, 'a timed call carries a signal');
    await api().send('PATCH', 'playbooks/a', {});
    assert.equal(calls[1].init.signal, undefined, 'existing callers are unchanged');
  });

  it('throws the server’s own error with its status', async () => {
    (globalThis.fetch as any).mock.mockImplementation(async () => ({
      ok: false,
      status: 409,
      json: async () => ({ error: 'A playbook named b already exists' }),
    }));
    await assert.rejects(api().send('PATCH', 'playbooks/a', { name: 'b' }), {
      message: 'A playbook named b already exists',
      status: 409,
    });
  });
});
