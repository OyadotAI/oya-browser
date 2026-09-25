/**
 * Unit tests for per-key settings: masked reads, validated and sealed writes,
 * the LLM settings a key resolves to, and the environment it hands providers.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir();
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { store } = await import('../../../../src/modules/config/store.ts');
const { CONFIG_VALUE_MAX_CHARS } = await import('../../../../src/modules/config/constants.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { runtimeConfig } = await import('../../../../src/platform/runtime-config.ts');

const KEY = 'config-service-key';
const ENV = ['OPENAI_API_KEY', 'OYA_BROWSER_PROVIDER', 'BROWSERBASE_API_KEY', 'CHAT_MODEL'];
let saved: Record<string, string | undefined>;

describe('key settings', () => {
  beforeEach(() => {
    keyConfig.reset();
    saved = Object.fromEntries(ENV.map((k) => [k, process.env[k]]));
    for (const k of ENV) delete process.env[k];
  });
  afterEach(() => {
    for (const [k, v] of Object.entries(saved)) restoreEnv(k, v);
    keyConfig.reset();
  });

  describe('get', () => {
    it('shows a key that set nothing as empty, running on the host defaults', () => {
      const view = keyConfig.get(KEY);
      assert.equal(view.openai_api_key, '');
      assert.equal(view.has_openai_key, false);
      assert.equal(view.inherited, false);
      assert.deepEqual(view.effective, {
        baseUrl: runtimeConfig.getOpenAIBase(),
        model: runtimeConfig.getChatModel(),
        hasLlmKey: false,
      });
    });

    it('lists every LLM provider with its models, so every client builds the same pickers', () => {
      const catalog = keyConfig.get(KEY).llm_catalog;
      assert.deepEqual(
        catalog.map((p) => p.id),
        ['openai', 'anthropic', 'gemini', 'vertex', 'openrouter'],
      );
      const claude = catalog.find((p) => p.id === 'anthropic');
      assert.equal(claude.model, 'claude-opus-5');
      assert.ok(claude.models.some((m) => m.id === 'claude-opus-5-5'));
      assert.ok(catalog.every((p) => p.label && p.hint && p.keysUrl.startsWith('https://') && p.models.length));
    });

    it('masks a secret to its last four characters', async () => {
      await keyConfig.set(KEY, { openai_api_key: 'sk-abcdef1234' });
      const view = keyConfig.get(KEY);
      assert.equal(view.openai_api_key, '••••1234');
      assert.equal(view.has_openai_key, true);
    });

    it('marks the LLM key as inherited when the deployment supplies it', () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      assert.equal(keyConfig.get(KEY).inherited, true);
    });

    it('lists every provider choice, configured only when its credentials are present', async () => {
      const byId = () => Object.fromEntries(keyConfig.get(KEY).providers.map((p) => [p.id, p.configured]));
      assert.equal(byId().browserbase, false);
      await keyConfig.set(KEY, { browserbase_api_key: 'bb-1' });
      assert.equal(byId().browserbase, true);
      assert.equal(byId().cdp, false);
      assert.deepEqual(
        Object.keys(byId()),
        keyConfig.PROVIDER_CHOICES.map((p) => p.id),
      );
    });

    it('counts a provider credential from the environment as configured', () => {
      process.env.BROWSERBASE_API_KEY = 'bb-env';
      const bb = keyConfig.get(KEY).providers.find((p) => p.id === 'browserbase');
      assert.equal(bb.configured, true);
    });
  });

  describe('set', () => {
    it('stores secrets sealed, never in plaintext', async () => {
      await keyConfig.set(KEY, { openai_api_key: 'sk-plaintext-value' });
      const row = store.get(fingerprint(KEY));
      assert.ok(row.openai_api_key);
      assert.ok(!row.openai_api_key.includes('sk-plaintext-value'));
    });

    it('never writes the masked placeholder back over a real secret', async () => {
      await keyConfig.set(KEY, { openai_api_key: 'sk-real-1234' });
      assert.equal(await keyConfig.set(KEY, { openai_api_key: '••••1234' }), false);
      assert.equal(keyConfig.resolve(KEY).openaiKey, 'sk-real-1234');
    });

    it('clears a field set to null or empty', async () => {
      await keyConfig.set(KEY, { chat_model: 'gpt-x', openai_api_key: 'sk-1' });
      assert.equal(await keyConfig.set(KEY, { chat_model: null, openai_api_key: '' }), true);
      const view = keyConfig.get(KEY);
      assert.equal(view.openai_api_key, '');
      assert.equal(store.get(fingerprint(KEY)).chat_model, undefined);
    });

    it('answers false for an empty update and stores nothing', async () => {
      assert.equal(await keyConfig.set(KEY), false);
      assert.equal(await keyConfig.set(KEY, {}), false);
      assert.equal(store.has(fingerprint(KEY)), false);
    });

    it('refuses an unknown LLM provider rather than storing it', async () => {
      await assert.rejects(keyConfig.set(KEY, { llm_provider: 'nope' }), {
        status: 400,
        message: /llm_provider must be one of: openai, anthropic, gemini, vertex/,
      });
    });

    it('refuses an unknown browser provider and CAPTCHA solver', async () => {
      await assert.rejects(keyConfig.set(KEY, { browser_provider: 'nope' }), { status: 400 });
      await assert.rejects(keyConfig.set(KEY, { captcha_solver: 'nope' }), { status: 400 });
    });

    it('refuses a base URL that is not https', async () => {
      await assert.rejects(keyConfig.set(KEY, { openai_base_url: 'http://8.8.8.8/v1' }), {
        status: 400,
        message: 'openai_base_url must use https',
      });
    });

    it('refuses a setting it does not know, naming it and listing the settings there are', async () => {
      await assert.rejects(keyConfig.set(KEY, { chat_modle: 'x' }), (err: any) => {
        assert.deepEqual([err.status, err.code, err.field], [400, 'invalid_request', 'chat_modle']);
        assert.match(err.message, /^Unknown setting "chat_modle"\. Settings are: llm_provider, /);
        return true;
      });
    });

    it('refuses an object or an array as a value, naming the setting', async () => {
      await assert.rejects(keyConfig.set(KEY, { chat_model: {} }), {
        status: 400,
        field: 'chat_model',
        message: 'chat_model must be a string, not an object',
      });
      await assert.rejects(keyConfig.set(KEY, { chat_model: [1] }), { status: 400 });
    });

    it('refuses a value longer than a setting can be', async () => {
      await assert.rejects(keyConfig.set(KEY, { chat_model: 'x'.repeat(CONFIG_VALUE_MAX_CHARS + 1) }), {
        status: 400,
        field: 'chat_model',
      });
      await keyConfig.set(KEY, { chat_model: 'y'.repeat(CONFIG_VALUE_MAX_CHARS) });
    });

    it('stores numbers as strings', async () => {
      await keyConfig.set(KEY, { desktop_seen_at: 1234 });
      assert.equal(keyConfig.get(KEY).desktop_seen_at, '1234');
    });
  });

  describe('resolve', () => {
    it('uses the deployment’s key, base URL and model for a key without its own credential', async () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      await keyConfig.set(KEY, { chat_model: 'my-model' });
      assert.deepEqual(keyConfig.resolve(KEY), {
        openaiKey: 'sk-host',
        baseUrl: runtimeConfig.getOpenAIBase(),
        model: 'my-model',
      });
    });

    it('uses the provider’s defaults with the key’s own credential', async () => {
      await keyConfig.set(KEY, { llm_provider: 'anthropic', openai_api_key: 'sk-ant' });
      assert.deepEqual(keyConfig.resolve(KEY), {
        own: true,
        openaiKey: 'sk-ant',
        baseUrl: 'https://api.anthropic.com/v1',
        model: 'claude-opus-5',
      });
    });

    it('honours the key’s own base URL only alongside its own credential', async () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      store.set(fingerprint(KEY), { openai_base_url: 'https://llm.example.com/v1' });
      assert.equal(keyConfig.resolve(KEY).baseUrl, runtimeConfig.getOpenAIBase(), 'never ships the host key there');
      await keyConfig.set(KEY, { openai_api_key: 'sk-own' });
      assert.equal(keyConfig.resolve(KEY).baseUrl, 'https://llm.example.com/v1');
    });

    it("never lends the deployment's key to an agent's unclaimed key, only its own", async () => {
      const { keyDigest, noteAgentKey } = await import('../../../../src/modules/auth/keys.ts');
      process.env.OPENAI_API_KEY = 'sk-host';
      noteAgentKey(keyDigest(KEY), true);
      try {
        assert.equal(keyConfig.resolve(KEY).openaiKey, '');
        await keyConfig.set(KEY, { llm_provider: 'anthropic', openai_api_key: 'sk-agent' });
        assert.equal(keyConfig.resolve(KEY).openaiKey, 'sk-agent');
      } finally {
        noteAgentKey(keyDigest(KEY), false);
      }
      await keyConfig.set(KEY, { openai_api_key: '' });
      assert.equal(keyConfig.resolve(KEY).openaiKey, 'sk-host', 'once claimed it runs on the deployment again');
    });

    it('reads a secret that no longer unseals as absent', () => {
      store.set(fingerprint(KEY), { openai_api_key: 'bm90LXNlYWxlZA==' });
      assert.equal(keyConfig.resolve(KEY).own, undefined);
      assert.equal(keyConfig.get(KEY).openai_api_key, '');
    });
  });

  it('layers the key’s credentials over the environment it is given', async () => {
    await keyConfig.set(KEY, { anchor_api_key: 'an-1', captcha_solver: 'capsolver' });
    const env = keyConfig.envFor(KEY, { PATH: '/bin', ANCHOR_API_KEY: 'host' });
    assert.deepEqual(env, { PATH: '/bin', ANCHOR_API_KEY: 'an-1', OYA_CAPTCHA_PROVIDER: 'capsolver' });
  });

  it('picks the key’s browser provider, else the environment’s, else cdp', async () => {
    assert.equal(keyConfig.providerFor(KEY), 'cdp');
    process.env.OYA_BROWSER_PROVIDER = 'steel';
    assert.equal(keyConfig.providerFor(KEY), 'steel');
    await keyConfig.set(KEY, { browser_provider: 'anchor' });
    assert.equal(keyConfig.providerFor(KEY), 'anchor');
  });

  it('keeps each key’s settings apart', async () => {
    await keyConfig.set(KEY, { chat_model: 'mine' });
    assert.notEqual(keyConfig.get('another-key').chat_model, 'mine');
  });
});
