/**
 * Unit tests for page translation: refused where Ask is refused, refused on
 * server-run browsers, billed against the hourly chat cap, and only a
 * same-length array of strings from the model is accepted.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';
import { json, stubFetch } from '../../support/http.ts';

ownDataDir('oya-translate-');
// One reply's tokens (15) reach the cap, so the next call is refused.
process.env.OYA_QUOTA_CHAT_TOKENS_HOUR = '15';
const { translateTexts } = await import('../../../../src/modules/translate/service.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const usage = await import('../../../../src/platform/usage.ts');
const { stubLlm } = await import('../../support/agent.ts');

/** A key no other suite uses. */
const KEY = 'translate-service-key-0123456789ab';
/** The desktop browser the tests translate for. */
const BROWSER = 'b-translate';
/** A request body in Japanese. */
const BODY = { texts: ['こんにちは', '世界'], target: 'en-US' };
/** A model reply answering `content`. */
const reply = (content: string) => ({
  choices: [{ message: { content } }],
  usage: { prompt_tokens: 10, completion_tokens: 5 },
});

describe('translateTexts', () => {
  const saved = process.env.OPENAI_API_KEY;
  afterEach(() => {
    mock.restoreAll();
    restoreEnv('OPENAI_API_KEY', saved);
    usage.reset();
    registry.remove(BROWSER);
  });

  /** Registers the browser under `provider` and gives the deployment a model. */
  const connect = (provider?: string) => {
    process.env.OPENAI_API_KEY = 'sk-host';
    registry.add(BROWSER, { apiKey: KEY, name: 'Test', clientType: 'oya', provider, engine: {} });
  };

  it('answers the model’s array, in order', async () => {
    connect();
    stubLlm([reply('["Hello", "World"]')]);
    assert.deepEqual(await translateTexts(KEY, BROWSER, BODY), { texts: ['Hello', 'World'] });
  });

  it('sends the page text as JSON data in the user message, never in the system prompt', async () => {
    connect();
    const { requests } = stubLlm([reply('["Hello", "World"]')]);
    await translateTexts(KEY, BROWSER, BODY);
    const [system, user] = requests[0].messages;
    assert.ok(!system.content.includes('こんにちは'));
    assert.deepEqual(JSON.parse(user.content), BODY.texts);
  });

  it('accepts an array inside a code fence', async () => {
    connect();
    stubLlm([reply('```json\n["Hello", "World"]\n```')]);
    assert.deepEqual((await translateTexts(KEY, BROWSER, BODY)).texts, ['Hello', 'World']);
  });

  it('refuses an answer of the wrong length as translate_malformed', async () => {
    connect();
    stubLlm([reply('["Hello"]')]);
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { status: 502, code: 'translate_malformed' });
  });

  it('refuses an answer that is not JSON as translate_malformed', async () => {
    connect();
    stubLlm([reply('Hello, World')]);
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { code: 'translate_malformed' });
  });

  it('refuses an answer whose items are not all strings as translate_malformed', async () => {
    connect();
    stubLlm([reply('[null, {"a": 1}]')]);
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { code: 'translate_malformed' });
  });

  it('refuses a browser the server launched', async () => {
    connect('oya-cloud');
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { status: 403, code: 'translate_unavailable' });
  });

  it('refuses with llm_unconfigured when no model is set', async () => {
    connect();
    delete process.env.OPENAI_API_KEY;
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { status: 422 });
  });

  it('counts tokens against the hourly cap, so the next call is refused with llm_quota', async () => {
    connect();
    stubLlm([reply('["Hello", "World"]')]);
    await translateTexts(KEY, BROWSER, BODY);
    assert.equal(usage.current(KEY).chat_input_tokens, 10);
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { status: 429, code: 'llm_quota' });
  });

  it('turns a provider 401 into llm_rejected', async () => {
    connect();
    stubFetch(() => json({ error: { message: 'bad key' } }, 401));
    await assert.rejects(translateTexts(KEY, BROWSER, BODY), { code: 'llm_rejected' });
  });
});
