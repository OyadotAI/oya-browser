/** Unit tests for src/main/connection/server-api.ts: a refusal keeps the server's words, status and code. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { readApiAnswer } from '../../../../src/main/connection/server-api.ts';

it('answers the JSON of a success', async () => {
  assert.deepEqual(await readApiAnswer(Response.json({ ok: 1 })), { ok: 1 });
});

it('throws a refusal with the server’s message, its status and its code', async () => {
  const res = Response.json({ error: 'No model', code: 'llm_unconfigured' }, { status: 422 });
  await assert.rejects(readApiAnswer(res), { message: 'No model', status: 422, code: 'llm_unconfigured' });
});

it('says the status when the refusal has no body', async () => {
  await assert.rejects(readApiAnswer(new Response('', { status: 502 })), {
    message: 'Server returned 502',
    status: 502,
  });
});
