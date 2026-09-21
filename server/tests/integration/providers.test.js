/**
 * Cloud browser vendors (Anchor, Browserbase, Browser Use, Steel) against a stubbed
 * fetch: each creates, authenticates and releases with the vendor's own contract,
 * release is idempotent and retryable, and a failed create cleans up without echoing
 * secrets.
 */
import assert from 'node:assert/strict';
import { acquire } from '../../src/drivers/providers.ts';

const originalFetch = globalThis.fetch;
const cases = [
  [
    'anchor',
    { data: { id: 'test-id', cdp_url: 'wss://example.com/cdp' } },
    '/v1/sessions',
    'DELETE',
    '/test-id',
    undefined,
  ],
  [
    'browserbase',
    { id: 'test-id', connectUrl: 'wss://example.com/cdp' },
    '/v1/sessions',
    'POST',
    '/test-id',
    { status: 'REQUEST_RELEASE' },
  ],
  [
    'browseruse',
    { id: 'test-id', cdpUrl: 'wss://example.com/cdp' },
    '/api/v2/browsers',
    'PATCH',
    '/test-id',
    { action: 'stop' },
  ],
  [
    'steel',
    { id: 'test-id', websocketUrl: 'wss://connect.steel.dev?sessionId=test-id' },
    '/v1/sessions',
    'POST',
    '/test-id/release',
    undefined,
  ],
];
try {
  for (const [provider, payload, path, method, suffix, body] of cases) {
    const calls = [];
    globalThis.fetch = async (url, options) => {
      calls.push({ url: String(url), ...options });
      return Response.json(calls.length === 1 ? payload : {});
    };
    const session = await acquire({ provider, env: { [`${provider.toUpperCase()}_API_KEY`]: 'test-secret+&' } });
    assert.equal(new URL(calls[0].url).pathname, path);
    if (provider === 'steel') assert.equal(new URL(session.wsUrl).searchParams.get('apiKey'), 'test-secret+&');
    await Promise.all([session.release(), session.release()]);
    assert.equal(calls.length, 2, `${provider}: release is idempotent`);
    assert.equal(calls[1].method, method);
    assert.equal(new URL(calls[1].url).pathname, path + suffix);
    assert.deepEqual(calls[1].body ? JSON.parse(calls[1].body) : undefined, body);
    console.log(`✓ ${provider} create / authenticate / release contract`);
  }
  let calls = 0;
  globalThis.fetch = async () => Response.json(++calls === 1 ? { id: 'orphan' } : {});
  await assert.rejects(acquire({ provider: 'steel', env: { STEEL_API_KEY: 'secret' } }), /CDP URL/);
  assert.equal(calls, 2, 'missing CDP URL releases the created session');
  calls = 0;
  globalThis.fetch = async () =>
    ++calls === 1
      ? Response.json({ id: 'id', connectUrl: 'wss://example.com' })
      : Response.json({}, { status: calls === 2 ? 500 : 200 });
  const session = await acquire({ provider: 'browserbase', env: { BROWSERBASE_API_KEY: 'secret' } });
  await assert.rejects(session.release(), /release failed \(500\)/);
  await session.release();
  assert.equal(calls, 3, 'a failed release can be retried');
  globalThis.fetch = async () => new Response('secret credential echoed by vendor', { status: 401 });
  await assert.rejects(
    acquire({ provider: 'steel', env: { STEEL_API_KEY: 'secret' } }),
    (e) => !e.message.includes('secret'),
  );
  console.log('✓ failed creates clean up; release errors surface and can retry; errors omit secrets');
} finally {
  globalThis.fetch = originalFetch;
}
