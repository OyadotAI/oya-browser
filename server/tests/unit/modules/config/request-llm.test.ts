/**
 * Unit tests for a model picked per request (`new Oya({ model })`): which headers
 * are accepted, what a run resolves to with them, that nothing is saved, and the
 * guards that keep the deployment's credential on its own vendor's priced models.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';
import { FakeResponse } from '../../support/browsers.ts';

ownDataDir();
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { llmFromHeaders, checkRequestLlm, requestLlmMiddleware, withLlm, requestLlm } =
  await import('../../../../src/modules/config/request-llm.ts');
const { store } = await import('../../../../src/modules/config/store.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { resolveFresh } = await import('../../../../src/modules/config/chatgpt.ts');

const KEY = 'request-llm-key';
const ENV = ['OPENAI_API_KEY', 'CHAT_MODEL'];
let envBefore: Record<string, string | undefined>;

/** Headers as the SDK sends them, lower-cased the way Node hands them over. */
const headers = (provider?: string, model?: string, apiKey?: string) => ({
  ...(provider ? { 'x-oya-llm-provider': provider } : {}),
  ...(model ? { 'x-oya-llm-model': model } : {}),
  ...(apiKey ? { 'x-oya-llm-api-key': apiKey } : {}),
});

/** The key's saved config, as the middleware reads it. */
const saved = () => keyConfig.resolveSaved(KEY);

/** The 400 message for headers `h`: parsing them, then, with `savedFor`, checking them against the key's credential. */
function refusal(h: Record<string, string>, savedFor?: typeof saved): string {
  try {
    const llm = llmFromHeaders(h);
    if (savedFor && llm) withLlm(llm, () => checkRequestLlm(savedFor));
  } catch (err) {
    assert.equal(err.status, 400);
    return err.message;
  }
  throw new Error('expected a 400');
}

describe('request model', () => {
  beforeEach(() => {
    keyConfig.reset();
    envBefore = Object.fromEntries(ENV.map((k) => [k, process.env[k]]));
    process.env.OPENAI_API_KEY = 'sk-host';
    delete process.env.CHAT_MODEL;
  });
  afterEach(() => {
    for (const [k, v] of Object.entries(envBefore)) restoreEnv(k, v);
    keyConfig.reset();
  });

  describe('headers', () => {
    it('asks nothing when no header is set', () => {
      assert.equal(llmFromHeaders({}), null);
    });

    it('needs a provider alongside a model or key', () => {
      assert.match(refusal(headers(undefined, 'gpt-4o')), /X-Oya-Llm-Provider is required/);
    });

    it('refuses a provider outside the catalog', () => {
      assert.match(refusal(headers('acme', undefined, 'sk')), /must be one of openai, anthropic/);
    });

    it('refuses chatgpt, whose credential is a sign-in that must never arrive as a header', () => {
      assert.match(refusal(headers('chatgpt', undefined, 'rt-token')), /must be one of/);
    });

    it("refuses a model of another vendor than the key's credential, on the server's own", () => {
      assert.match(refusal(headers('anthropic', 'claude-opus-5'), saved), /This key's model is not anthropic/);
    });

    it("refuses an unpriced model on the server's own credential", () => {
      assert.match(refusal(headers('openai', 'o9-ultra'), saved), /not offered on this server's model/);
    });

    it("refuses another vendor than the key's own saved credential", async () => {
      await keyConfig.set(KEY, { openai_api_key: 'sk-own' });
      assert.match(refusal(headers('gemini', 'gemini-3.8-flash'), saved), /not gemini/);
    });

    it("lets any model run on the key's own saved credential of the same vendor", async () => {
      await keyConfig.set(KEY, { openai_api_key: 'sk-own' });
      assert.doesNotThrow(() => withLlm({ provider: 'openai', model: 'o9-ultra' }, () => checkRequestLlm(saved)));
    });

    it('checks nothing against a key with no credential at all; the chat says how to bring one', () => {
      delete process.env.OPENAI_API_KEY;
      assert.doesNotThrow(() => withLlm({ provider: 'anthropic', model: 'claude-x' }, () => checkRequestLlm(saved)));
    });

    it('takes any model with the caller’s own key', () => {
      assert.deepEqual(llmFromHeaders(headers('anthropic', 'claude-x', 'sk-ant')), {
        provider: 'anthropic',
        model: 'claude-x',
        apiKey: 'sk-ant',
      });
    });

    it('refuses a value past the setting limit', () => {
      assert.match(refusal(headers('openai', undefined, 'k'.repeat(5000))), /too long/);
    });
  });

  describe('resolve', () => {
    it('uses the caller’s key at the catalog endpoint, and saves nothing', () => {
      const llm = withLlm({ provider: 'anthropic', apiKey: 'sk-ant' }, () => keyConfig.resolve(KEY));
      assert.deepEqual(llm, {
        own: true,
        openaiKey: 'sk-ant',
        baseUrl: 'https://api.anthropic.com/v1',
        model: 'claude-opus-5',
      });
      assert.equal(keyConfig.get(KEY).inherited, true, 'the saved config still runs on the host');
    });

    it('never takes the endpoint from the key’s saved base URL for a request key', () => {
      store.set(fingerprint(KEY), { openai_base_url: 'https://llm.example.com/v1' });
      const llm = withLlm({ provider: 'openai', apiKey: 'sk-req' }, () => keyConfig.resolve(KEY));
      assert.equal(llm.baseUrl, 'https://api.openai.com/v1');
    });

    it('swaps only the model on the deployment’s credential', () => {
      const llm = withLlm({ provider: 'openai', model: 'gpt-4o' }, () => keyConfig.resolve(KEY));
      assert.equal(llm.openaiKey, 'sk-host');
      assert.equal(llm.model, 'gpt-4o');
    });

    it('swaps the model on a key’s own saved credential of the same vendor', async () => {
      await keyConfig.set(KEY, { openai_api_key: 'sk-own', chat_model: 'gpt-4.1' });
      const llm = withLlm({ provider: 'openai', model: 'gpt-4o' }, () => keyConfig.resolve(KEY));
      assert.deepEqual([llm.openaiKey, llm.model], ['sk-own', 'gpt-4o']);
    });

    it("lends an agent's unclaimed key no model unless it brings its own key", async () => {
      const { keyDigest, noteAgentKey } = await import('../../../../src/modules/auth/keys.ts');
      noteAgentKey(keyDigest(KEY), true);
      try {
        assert.equal(withLlm({ provider: 'openai', model: 'gpt-4o' }, () => keyConfig.resolve(KEY)).openaiKey, '');
        assert.equal(withLlm({ provider: 'openai', apiKey: 'sk-a' }, () => keyConfig.resolve(KEY)).openaiKey, 'sk-a');
      } finally {
        noteAgentKey(keyDigest(KEY), false);
      }
    });

    it('refuses a bad model choice where the model is called, after auth settled the key', async () => {
      await assert.rejects(
        withLlm({ provider: 'anthropic', model: 'claude-x' }, () => resolveFresh(KEY)),
        { status: 400, message: /not anthropic/ },
      );
    });

    it('treats a request key as a plain API key, never a stored sign-in to refresh', async () => {
      const llm = await withLlm({ provider: 'openai', apiKey: '{"refresh_token":"x"}' }, () => resolveFresh(KEY));
      assert.equal(llm.openaiKey, '{"refresh_token":"x"}');
      assert.equal(llm.baseUrl, 'https://api.openai.com/v1');
    });

    it('carries the choice across awaits, so background work started by the request keeps it', async () => {
      const seen = await withLlm({ provider: 'openai', model: 'gpt-4o' }, async () => {
        await new Promise((resolve) => setImmediate(resolve));
        return keyConfig.resolve(KEY).model;
      });
      assert.equal(seen, 'gpt-4o');
      assert.equal(keyConfig.resolve(KEY).model, 'gpt-4o-mini', 'outside the request the saved model is back');
    });
  });

  describe('middleware', () => {
    it('runs the rest of the request with the model asked for', () => {
      let inside;
      requestLlmMiddleware({ headers: headers('openai', 'gpt-4o') }, new FakeResponse(), () => (inside = requestLlm()));
      assert.deepEqual(inside, { provider: 'openai', model: 'gpt-4o', apiKey: undefined });
    });

    it('answers 400 and stops on a bad header', () => {
      const res = new FakeResponse();
      let ran = false;
      requestLlmMiddleware({ headers: headers('acme', 'm', 'k') }, res, () => (ran = true));
      assert.equal(res.statusCode, 400);
      assert.equal(ran, false);
    });
  });
});
