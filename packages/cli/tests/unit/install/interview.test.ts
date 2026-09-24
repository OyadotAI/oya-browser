/**
 * Unit tests for the install interview (src/install/interview.ts) on the
 * replay path: a complete plan asks nothing, and secrets come from the
 * environment.
 */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { interview } from '../../../src/install/interview.ts';
import type { Answers } from '../../../src/install/types.ts';

const PLAN: Answers = {
  version: 1,
  host: 'docker',
  database: 'postgres',
  fleet: 'browserbase',
  workers: 0,
  llm: { provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'm' },
  publicUrl: 'http://localhost:3100/',
  optional: { captcha: '', recordingBucket: '', metrics: false },
};

describe('interview (replay)', () => {
  afterEach(() => mock.restoreAll());

  it('takes every answer from the plan and every secret from the environment', async () => {
    mock.method(process.stdout, 'write', () => true);
    const env = { DATABASE_URL: 'postgres://u:p@db/x', BROWSERBASE_API_KEY: 'bb', OPENAI_API_KEY: 'sk' };
    Object.assign(process.env, env);
    const result = await interview(PLAN, true);
    assert.deepEqual(result.answers, { ...PLAN, k8sFleet: undefined, publicUrl: 'http://localhost:3100' });
    assert.deepEqual(result.secrets, {
      DATABASE_URL: env.DATABASE_URL,
      BROWSERBASE_API_KEY: 'bb',
      BROWSERBASE_PROJECT_ID: '',
      OPENAI_API_KEY: 'sk',
    });
    assert.equal(result.migrateUrl, env.DATABASE_URL);
  });

  it('takes Supabase’s Postgres connection string too, since the data lives there', async () => {
    mock.method(process.stdout, 'write', () => true);
    const env = {
      SUPABASE_URL: 'https://x.supabase.co',
      SUPABASE_SERVICE_KEY: 'sk',
      DATABASE_URL: 'postgres://u:p@db/x',
    };
    Object.assign(process.env, env);
    const result = await interview({ ...PLAN, database: 'supabase' }, true);
    assert.equal(result.secrets.DATABASE_URL, env.DATABASE_URL);
    assert.equal(result.migrateUrl, env.DATABASE_URL);
  });
});
