/**
 * Unit tests for the deployment-wide runtime config: environment fallbacks,
 * the masked key, saving to the settings table, taking in the pre-storage
 * config.json once, and the base URL guard against server-side request forgery.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dataPath } from '../../../src/platform/paths.ts';

// The legacy file must be in place before the module loads, since it loads at import.
mkdirSync(dataPath(), { recursive: true });
writeFileSync(dataPath('config.json'), JSON.stringify({ legacy_note: 'kept' }));
// Watched so a test can tell when that load has taken the file in.
const log = mock.method(console, 'log', () => {});
const { runtimeConfig, validateBaseUrl } = await import('../../../src/platform/runtime-config.ts');
const { getConnection } = await import('../../../src/platform/storage/index.ts');

/** The stored value of one setting, or undefined. */
const storedSetting = async (key: string) => (await getConnection().select('settings', { key }))[0]?.value;

/** Whether the import-time load has logged taking in the legacy file. */
const imported = () => log.mock.calls.some((c) => /imported/.test(String(c.arguments[0])));

/** Lets the import-time load finish: once it logs the import, only its storage read is left, and that settles in one turn. */
async function loaded() {
  while (!imported()) await new Promise((resolve) => setImmediate(resolve));
  await new Promise((resolve) => setImmediate(resolve));
  log.mock.restore();
}

/** Environment variables the config falls back to; cleared around each test. */
const ENV = ['OPENAI_API_KEY', 'OPENAI_BASE_URL', 'CHAT_MODEL'];

describe('runtimeConfig', () => {
  const saved = Object.fromEntries(ENV.map((k) => [k, process.env[k]]));
  beforeEach(() => ENV.forEach((k) => delete process.env[k]));
  afterEach(() => ENV.forEach((k) => (saved[k] === undefined ? delete process.env[k] : (process.env[k] = saved[k]))));

  it('takes in the legacy config.json once and sets it aside as .imported', async () => {
    await loaded();
    assert.equal(await storedSetting('legacy_note'), 'kept');
    assert.equal(existsSync(dataPath('config.json')), false);
  });

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

  it('saves the settings to the settings table', async () => {
    await runtimeConfig.set({ chat_model: 'saved-model' });
    assert.equal(runtimeConfig.getChatModel(), 'saved-model');
    assert.equal(await storedSetting('chat_model'), 'saved-model');
  });

  it('logs a save that storage refuses, keeping the setting in memory', async () => {
    const error = mock.method(console, 'error', () => {});
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    await runtimeConfig.set({ chat_model: 'unsaved-model' });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(runtimeConfig.getChatModel(), 'unsaved-model');
    assert.match(error.mock.calls[0].arguments[1], /storage down/);
    upsert.mock.restore();
    error.mock.restore();
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
