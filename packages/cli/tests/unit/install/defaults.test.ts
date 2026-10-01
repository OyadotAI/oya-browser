/**
 * Unit tests for the --yes defaults (src/install/defaults.ts): SQLite and
 * Docker on this machine, and an LLM only when the environment has a key.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { defaultInstall } from '../../../src/install/defaults.ts';

describe('defaultInstall', () => {
  it('runs everything on this machine with SQLite and one Docker browser worker', () => {
    const { answers } = defaultInstall({});
    assert.equal(answers.host, 'docker');
    assert.equal(answers.database, 'sqlite');
    assert.equal(answers.fleet, 'docker-workers');
    assert.equal(answers.workers, 1);
    assert.equal(answers.publicUrl, 'http://localhost:3100');
  });

  it('skips the LLM when no key is in the environment', () => {
    const { answers, secrets } = defaultInstall({});
    assert.equal(answers.llm.provider, 'skip');
    assert.deepEqual(secrets, {});
  });

  it('uses OPENAI_API_KEY with its base URL and model when set', () => {
    const env = { OPENAI_API_KEY: 'sk', OPENAI_BASE_URL: 'https://x.test/v1', CHAT_MODEL: 'm', ANTHROPIC_API_KEY: 'a' };
    const { answers, secrets } = defaultInstall(env);
    assert.deepEqual(answers.llm, { provider: 'openai', baseUrl: 'https://x.test/v1', model: 'm' });
    assert.deepEqual(secrets, { OPENAI_API_KEY: 'sk' });
  });

  it('falls back to ANTHROPIC_API_KEY against the Anthropic endpoint', () => {
    const { answers, secrets } = defaultInstall({ ANTHROPIC_API_KEY: 'a' });
    assert.equal(answers.llm.provider, 'anthropic');
    assert.equal(answers.llm.baseUrl, 'https://api.anthropic.com/v1');
    assert.deepEqual(secrets, { OPENAI_API_KEY: 'a' });
  });
});
