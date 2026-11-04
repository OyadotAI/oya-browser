/** Native Gemini diagnostics serialize an explicit output limit and retain caller cancellation. */
import { afterEach, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { gemini } from '../../../../src/platform/llm/gemini.ts';
afterEach(() => mock.restoreAll());
it('propagates diagnostic limits to the native Gemini request', async () => {
  const abort = new AbortController();
  let sent: any;
  mock.method(globalThis, 'fetch', async (_url, init) => {
    sent = init;
    return { ok: true, json: async () => ({ candidates: [{ content: { parts: [{ text: 'OK' }] } }] }) } as any;
  });
  await gemini.send({
    baseUrl: 'https://aiplatform.googleapis.com/v1/publishers/google',
    apiKey: 'synthetic-key',
    model: 'catalog-model',
    messages: [{ role: 'user', content: 'Reply with OK only.' }],
    signal: abort.signal,
    maxTokens: 73,
  });
  assert.deepEqual(JSON.parse(sent.body).generationConfig, { maxOutputTokens: 73 });
  abort.abort();
  assert.equal(sent.signal.aborted, true);
});
