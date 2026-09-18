/**
 * Unit tests for selfApi: lifecycle calls replayed against the public API as
 * the caller, with a fresh idempotency key, and their errors surfaced.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { selfApi } from '../../../src/mcp/self-api.ts';

const SELF = { origin: 'http://127.0.0.1:3100', authorization: 'Bearer k' };

describe('selfApi', () => {
  afterEach(() => mock.restoreAll());

  it('posts the body to /api with the caller’s credential and an idempotency key', async () => {
    const fetch = mock.method(globalThis, 'fetch', async () => Response.json({ id: 'b1' }));
    assert.deepEqual(await selfApi(SELF, '/browsers/start', { name: 'n' }), { id: 'b1' });
    const [url, init] = fetch.mock.calls[0].arguments;
    assert.equal(url, 'http://127.0.0.1:3100/api/browsers/start');
    assert.equal(init.headers.Authorization, 'Bearer k');
    assert.match(init.headers['Idempotency-Key'], /^[0-9a-f-]{36}$/);
    assert.equal(init.body, '{"name":"n"}');
  });

  it('sends an empty object when there is no body', async () => {
    const fetch = mock.method(globalThis, 'fetch', async () => Response.json({}));
    await selfApi(SELF, '/x');
    assert.equal(fetch.mock.calls[0].arguments[1].body, '{}');
  });

  it('throws the API’s error message on failure', async () => {
    mock.method(globalThis, 'fetch', async () => Response.json({ error: 'Browser quota reached' }, { status: 429 }));
    await assert.rejects(selfApi(SELF, '/x'), /Browser quota reached/);
  });

  it('falls back to the status line when the failure has no JSON error', async () => {
    mock.method(globalThis, 'fetch', async () => new Response('oops', { status: 502, statusText: 'Bad Gateway' }));
    await assert.rejects(selfApi(SELF, '/x'), /502 Bad Gateway/);
  });

  it('is unavailable without an origin or credential', async () => {
    await assert.rejects(selfApi({}, '/x'), /browser lifecycle is unavailable/);
  });
});
