/**
 * Unit tests for what every fleet runtime shares: the egress proxy endpoint and
 * the per-session setup that gives a governed browser its own credential and
 * egress token, never the project key.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../../support/data-dir.ts';

ownDataDir('oya-control-fleet-common-');
const { control, hash } = await import('../../../../../src/modules/control/service.ts');
const { egressProxy, prepareSession } = await import('../../../../../src/modules/control/fleet/common.ts');

let saved;
beforeEach(() => {
  saved = { proxy: process.env.OYA_MANAGED_PROXY_URL, url: process.env.OYA_MANAGED_CONTROL_URL };
  process.env.OYA_MANAGED_PROXY_URL = 'http://egress.internal:3128';
  process.env.OYA_MANAGED_CONTROL_URL = 'http://control.internal:3100';
});
afterEach(() => {
  restoreEnv('OYA_MANAGED_PROXY_URL', saved.proxy);
  restoreEnv('OYA_MANAGED_CONTROL_URL', saved.url);
});

describe('egressProxy', () => {
  it('points the browser at the proxy with per-session credentials', () => {
    assert.deepEqual(egressProxy('b1', 'tok'), {
      type: 'http',
      host: 'egress.internal',
      port: 3128,
      username: 'b1',
      password: 'tok',
    });
  });

  it('defaults the port by scheme', () => {
    process.env.OYA_MANAGED_PROXY_URL = 'https://egress.internal';
    assert.deepEqual([egressProxy('b', 't').type, egressProxy('b', 't').port], ['https', 443]);
    process.env.OYA_MANAGED_PROXY_URL = 'http://egress.internal';
    assert.equal(egressProxy('b', 't').port, 80);
  });

  it('refuses a proxy URL that is not HTTP(S) or embeds credentials', () => {
    for (const url of ['socks5://egress.internal', 'http://u:p@egress.internal']) {
      process.env.OYA_MANAGED_PROXY_URL = url;
      assert.throws(() => egressProxy('b', 't'), { status: 422, code: 'invalid_proxy' });
    }
  });
});

describe('prepareSession', () => {
  let n = 0,
    id;
  beforeEach(async () => {
    id = `m-${n++}`;
    await control().reserve('key-a', { id, provider: 'oya-selfhosted', managed: true });
  });

  /** Options for the reserved session on a runtime in `region`. */
  const options = (extra = {}) => ({
    apiKey: 'key-a',
    browserId: id,
    persona: 'p1',
    fallbackName: `oya-managed-${id}`,
    cleanup: { kind: 'docker', container: 'c' },
    runtime: { id: 'docker-local', region: 'local' },
    ...extra,
  });

  it('persists the cleanup descriptor and token hashes before anything is created', async () => {
    const { token } = await prepareSession(options({ policies: [{ region: 'local' }] }));
    const x = await control().store.get('session', id);
    assert.deepEqual(x.cleanup, { kind: 'docker', container: 'c' });
    assert.equal(x.egressHash, hash(token));
    assert.equal(x.enrollmentHash, hash(token));
    assert.equal(x.managed, true);
    assert.deepEqual(x.policies, [{ region: 'local' }]);
  });

  it('gives the container a session-scoped credential instead of the project key', async () => {
    const { token, environment } = await prepareSession(options());
    assert.notEqual(environment.OYA_API_KEY, 'key-a');
    assert.equal(JSON.stringify(environment).includes('key-a'), false);
    const principal = await control().authenticate(environment.OYA_API_KEY);
    assert.deepEqual([principal.role, principal.sessionId], ['browser', id]);
    assert.equal(environment.OYA_ENROLLMENT_TOKEN, token);
    assert.equal(environment.OYA_SERVER_URL, 'http://control.internal:3100');
    assert.equal(environment.OYA_BROWSER_NAME, `oya-managed-${id}`);
    assert.equal(environment.OYA_PROVIDER, 'oya-selfhosted');
    assert.deepEqual(JSON.parse(environment.OYA_GOVERNANCE).proxy.username, id);
  });

  it('refuses a policy that pins another region', async () => {
    await assert.rejects(prepareSession(options({ policies: [{ region: 'eu' }] })), {
      status: 422,
      code: 'region_unavailable',
    });
  });

  it('refuses a value with a newline, which could forge an env-file entry', async () => {
    await assert.rejects(prepareSession(options({ name: 'x\nOYA_API_KEY=stolen' })), {
      status: 400,
      code: 'invalid_environment',
    });
  });
});
