/**
 * Unit tests for the deployment-wide runtime config: environment fallbacks,
 * the masked key, saving to data/config.json without a database, and the base
 * URL guard against server-side request forgery.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runtimeConfig, validateBaseUrl } from '../../../src/platform/runtime-config.ts';
import { dataPath } from '../../../src/platform/paths.ts';

/** Environment variables the config falls back to; cleared around each test. */
const ENV = ['OPENAI_API_KEY', 'OPENAI_BASE_URL', 'CHAT_MODEL'];

describe('runtimeConfig', () => {
  const saved = Object.fromEntries(ENV.map((k) => [k, process.env[k]]));
  beforeEach(() => ENV.forEach((k) => delete process.env[k]));
  afterEach(() => ENV.forEach((k) => (saved[k] === undefined ? delete process.env[k] : (process.env[k] = saved[k]))));

  it('falls back to the environment, then to built-in defaults', () => {
    process.env.CHAT_MODEL = 'env-model';
    assert.equal(runtimeConfig.getChatModel(), 'env-model');
    assert.equal(runtimeConfig.getOpenAIBase(), 'https://api.openai.com/v1');
    assert.equal(runtimeConfig.get().has_openai_key, false);
    assert.equal(runtimeConfig.get().openai_api_key, '');
  });

  it('stores a key but only ever shows its last four characters', async () => {
    await runtimeConfig.set({ openai_api_key: 'sk-abcdef1234' });
    assert.equal(runtimeConfig.get().openai_api_key, '••••1234');
    assert.equal(runtimeConfig.getOpenAIKey(), 'sk-abcdef1234');
  });

  it('ignores a masked key sent back unchanged', async () => {
    await runtimeConfig.set({ openai_api_key: 'sk-original9999' });
    await runtimeConfig.set({ openai_api_key: '••••9999' });
    assert.equal(runtimeConfig.getOpenAIKey(), 'sk-original9999');
  });

  it('saves the settings to config.json when there is no database', async () => {
    await runtimeConfig.set({ chat_model: 'saved-model' });
    assert.equal(runtimeConfig.getChatModel(), 'saved-model');
    assert.equal(JSON.parse(readFileSync(dataPath('config.json'), 'utf8')).chat_model, 'saved-model');
  });

  it('validates the base URL before saving it', async () => {
    await assert.rejects(runtimeConfig.set({ openai_base_url: 'http://api.test' }), { status: 400 });
    await runtimeConfig.set({ openai_base_url: 'https://8.8.8.8/v1/' });
    assert.equal(runtimeConfig.getOpenAIBase(), 'https://8.8.8.8/v1');
  });
});

describe('validateBaseUrl', () => {
  it('clears the setting for an empty value', async () => {
    assert.equal(await validateBaseUrl('  '), '');
  });

  it('refuses something that is not a URL', async () => {
    await assert.rejects(validateBaseUrl('nope'), { status: 400, message: 'openai_base_url must be a valid URL' });
  });

  it('refuses plain http, embedded credentials and fragments', async () => {
    await assert.rejects(validateBaseUrl('http://8.8.8.8'), { message: 'openai_base_url must use https' });
    await assert.rejects(validateBaseUrl('https://u:p@8.8.8.8'), {
      message: 'openai_base_url must not embed credentials',
    });
    await assert.rejects(validateBaseUrl('https://8.8.8.8/#x'), {
      message: 'openai_base_url must not contain a fragment',
    });
  });

  it('refuses an address inside the network', async () => {
    const saved = process.env.OYA_ALLOW_PRIVATE_TARGETS;
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
    try {
      await assert.rejects(validateBaseUrl('https://10.0.0.5/v1'), /private or loopback/);
    } finally {
      if (saved !== undefined) process.env.OYA_ALLOW_PRIVATE_TARGETS = saved;
    }
  });

  it('returns the URL without trailing slashes', async () => {
    assert.equal(await validateBaseUrl(' https://8.8.8.8/v1// '), 'https://8.8.8.8/v1');
  });
});
