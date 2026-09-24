/**
 * Unit tests for .env assembly (src/install/build-env.ts): secrets are reused,
 * and each fleet gets its own settings.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildEnv } from '../../../src/install/build-env.ts';
import type { Answers } from '../../../src/install/types.ts';

const BASE: Answers = {
  version: 1,
  host: 'docker',
  database: 'sqlite',
  fleet: 'docker-workers',
  workers: 2,
  llm: { provider: 'openai', baseUrl: 'https://api.openai.com/v1', model: 'm' },
  publicUrl: 'https://oya.example.com',
  optional: { captcha: '', recordingBucket: '', metrics: false },
};

describe('buildEnv', () => {
  it('reuses an existing KEK, keys and operator token', () => {
    const existing = { OYA_PROFILE_SECRET: 'kek', API_KEYS: 'a,b', OYA_OPERATOR_TOKEN: 'op' };
    const { values, apiKey } = buildEnv(BASE, {}, existing);
    assert.deepEqual(
      [values.OYA_PROFILE_SECRET, values.API_KEYS, values.OYA_OPERATOR_TOKEN, apiKey],
      ['kek', 'a,b', 'op', 'a'],
    );
  });

  it('generates fresh secrets and an oya_ key when there is nothing to reuse', () => {
    const { values, apiKey } = buildEnv(BASE, {}, {});
    assert.match(values.OYA_PROFILE_SECRET, /^[A-Za-z0-9_-]{43}$/);
    assert.match(apiKey, /^oya_[A-Za-z0-9_-]{43}$/);
    assert.equal(values.API_KEYS, apiKey);
  });

  it('picks the server storage driver from the database answer; Supabase is its Postgres', () => {
    const storage = (database: string) => buildEnv({ ...BASE, database }, {}, {}).values.OYA_STORAGE;
    assert.deepEqual(['sqlite', 'postgres', 'supabase'].map(storage), ['sqlite', 'postgres', 'postgres']);
  });

  it('sets PORT only outside Docker, from the public URL or 3100', () => {
    assert.equal(buildEnv(BASE, {}, {}).values.PORT, undefined);
    assert.equal(buildEnv({ ...BASE, host: 'local', publicUrl: 'http://h:8080' }, {}, {}).values.PORT, '8080');
    assert.equal(buildEnv({ ...BASE, host: 'local', publicUrl: 'http://h' }, {}, {}).values.PORT, '3100');
  });

  it('writes the LLM unless it was skipped, and passes secrets through', () => {
    const { values } = buildEnv(BASE, { OPENAI_API_KEY: 'sk' }, {});
    assert.deepEqual(
      [values.OPENAI_BASE_URL, values.CHAT_MODEL, values.OPENAI_API_KEY],
      ['https://api.openai.com/v1', 'm', 'sk'],
    );
    const skipped = buildEnv({ ...BASE, llm: { provider: 'skip', baseUrl: '', model: '' } }, {}, {}).values;
    assert.equal(skipped.OPENAI_BASE_URL, undefined);
  });

  it('points Oya Cloud at the public socket and keeps a pre-rename snapshot name', () => {
    const { values } = buildEnv({ ...BASE, fleet: 'oya-cloud' }, {}, { DAYTONA_SNAPSHOT: 'snap' });
    assert.deepEqual(
      [values.OYA_BROWSER_PROVIDER, values.OYA_PUBLIC_WS_URL, values.OYA_CLOUD_SNAPSHOT],
      ['oya-cloud', 'wss://oya.example.com/ws', 'snap'],
    );
  });

  it('wires governed Docker to the compose service names inside Docker', () => {
    const { values } = buildEnv({ ...BASE, fleet: 'oya-selfhosted' }, {}, {});
    assert.equal(values.OYA_MANAGED_CONTROL_URL, 'ws://server:3100/ws');
    assert.equal(values.OYA_MANAGED_PROXY_URL, 'http://server:3128');
    assert.deepEqual(Object.keys(values).slice(-9), [
      'OYA_BROWSER_PROVIDER',
      'OYA_FLEET_RUNTIME',
      'OYA_MANAGED_NETWORK',
      'OYA_MANAGED_IMAGE',
      'OYA_MANAGED_CONTROL_URL',
      'OYA_EGRESS_PORT',
      'OYA_EGRESS_HOST',
      'OYA_MANAGED_PROXY_URL',
      'OYA_MANAGED_REGION',
    ]);
  });

  it('wires the Kubernetes fleet from its answers, or names the fleet when they are missing', () => {
    const k8sFleet = { namespace: 'ns', image: 'img', controlUrl: 'ws://c', proxyUrl: 'http://p' };
    const { values } = buildEnv({ ...BASE, fleet: 'k8s', k8sFleet }, {}, {});
    assert.deepEqual(
      [values.OYA_FLEET_RUNTIME, values.OYA_K8S_NAMESPACE, values.OYA_MANAGED_REGION],
      ['k8s', 'ns', 'ns'],
    );
    assert.equal(buildEnv({ ...BASE, fleet: 'k8s' }, {}, {}).values.OYA_BROWSER_PROVIDER, 'k8s');
  });

  it('names any other fleet as the provider, and adds optional services', () => {
    const optional = { captcha: 'capsolver', recordingBucket: 'rec', metrics: true };
    const { values, extra } = buildEnv({ ...BASE, fleet: 'steel', optional }, {}, { OYA_METRICS_TOKEN: 'mt' });
    assert.deepEqual(
      [values.OYA_BROWSER_PROVIDER, values.OYA_CAPTCHA_PROVIDER, values.OYA_RECORDING_BUCKET, values.OYA_METRICS_TOKEN],
      ['steel', 'capsolver', 'rec', 'mt'],
    );
    assert.match(extra[2], /^# FLEET_TOKEN=/);
  });
});
