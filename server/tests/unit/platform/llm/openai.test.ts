/**
 * Unit tests for the OpenAI-compatible transport: a request's `effort` reaches
 * Gemini's /openai endpoint as reasoning_effort, and is left out everywhere else,
 * where OpenAI's non-reasoning models refuse the field.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { openai } from '../../../../src/platform/llm/openai.ts';
import { stubFetch, json } from '../../support/http.ts';

/** A completion answering "ok". */
const OK = { choices: [{ message: { role: 'assistant', content: 'ok' } }] };

/** The body sent for a request to `baseUrl` with `effort`. */
async function sentBody(baseUrl: string, effort?: 'low') {
  const calls = stubFetch(() => json(OK));
  await openai.send({ baseUrl, apiKey: 'k', model: 'm', messages: [], ...(effort ? { effort } : {}) });
  return JSON.parse(calls[0].init.body);
}

describe('openai transport effort', () => {
  afterEach(() => mock.restoreAll());

  it('sends effort to Gemini’s OpenAI-compatible endpoint as reasoning_effort', async () => {
    const body = await sentBody('https://generativelanguage.googleapis.com/v1beta/openai', 'low');
    assert.equal(body.reasoning_effort, 'low');
  });

  it('leaves effort out for any other endpoint', async () => {
    assert.equal((await sentBody('https://api.openai.com/v1', 'low')).reasoning_effort, undefined);
  });

  it('sends no reasoning_effort when the request names no effort', async () => {
    const body = await sentBody('https://generativelanguage.googleapis.com/v1beta/openai');
    assert.equal(body.reasoning_effort, undefined);
  });
});
